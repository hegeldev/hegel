//! Zone sources (`hegel_zone_source_t`): the time zone data the zone draws
//! and queries use, filled from Rust closures through libhegel's callbacks.

use super::hegel_c::{self, hegel_result_t};
use super::{free_on_drop, rc_to_unit, require_ok, static_string, view};
use super::{string_opt, with_context};
use std::ffi::c_void;
use std::ptr;

/// The message libhegel recorded on `ctx`, which need not be this
/// thread's context: a callback reads the one it was handed.
fn context_error(ctx: *mut hegel_c::HegelContext) -> String {
    // SAFETY: ctx is a live context handle; its message is copied out
    // before any later call taking it can invalidate the buffer.
    let p = unsafe { hegel_c::hegel_context_last_error(ctx) };
    string_opt(p).unwrap_or_default()
}

/// A local time type an oracle answers, the Rust side of
/// `hegel_zone_type_t`.
pub(crate) struct ZoneTypeRaw {
    pub(crate) utoff: i32,
    pub(crate) is_dst: bool,
    pub(crate) abbreviation: String,
}

/// The offset callback of an oracle.
pub(crate) type OffsetFn<'a> = dyn FnMut(i64) -> Result<ZoneTypeRaw, String> + 'a;

/// The next-transition callback of an oracle.
pub(crate) type NextTransitionFn<'a> = dyn FnMut(i64) -> Result<Option<i64>, String> + 'a;

/// An offset oracle (`hegel_zone_oracle_t`), borrowed for the length of
/// one `hegel_zone_source_add_oracle` call.
pub(crate) struct OracleCallbacks<'a, 'b> {
    pub(crate) offset_at: &'b mut OffsetFn<'a>,
    pub(crate) next_transition: Option<&'b mut NextTransitionFn<'a>>,
    pub(crate) first_year: i32,
    pub(crate) last_year: i32,
    pub(crate) after: u32,
}

struct OracleState<'a, 'b> {
    callbacks: OracleCallbacks<'a, 'b>,
    abbreviation: String,
}

/// What a callback returns for success; any other value is failure.
const CALLBACK_OK: i32 = 0;
const CALLBACK_FAILED: i32 = 1;

/// Run a callback's body, turning an `Err` or a panic into the message
/// libhegel reports for the failed callback: nothing may unwind across the
/// C ABI.
fn callback_result(
    ctx: *mut hegel_c::HegelContext,
    what: &str,
    body: impl FnOnce() -> Result<(), String>,
) -> i32 {
    let message = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(Ok(())) => return CALLBACK_OK,
        Ok(Err(message)) => message,
        Err(payload) => format!(
            "{what} panicked: {}",
            crate::run_lifecycle::panic_message(&payload)
        ),
    };
    // SAFETY: ctx is the live context libhegel handed the callback, and
    // the message outlives the call. The message is valid UTF-8, so the
    // call succeeds.
    let _ = unsafe { hegel_c::hegel_context_set_error(ctx, view(message.as_bytes())) };
    CALLBACK_FAILED
}

unsafe extern "C" fn oracle_offset_trampoline(
    user_data: *mut c_void,
    ctx: *mut hegel_c::HegelContext,
    instant: i64,
    out_type: *mut hegel_c::hegel_zone_type_t,
) -> i32 {
    // SAFETY: user_data is the `OracleState` `add_oracle` passed, alive and
    // ours alone for the length of that call, the only time libhegel calls
    // the oracle, on the calling thread.
    let state = unsafe { &mut *user_data.cast::<OracleState<'_, '_>>() };
    callback_result(ctx, "zone oracle", || {
        let local_type = (state.callbacks.offset_at)(instant)?;
        state.abbreviation = local_type.abbreviation;
        // SAFETY: out_type is libhegel's out-parameter; the abbreviation
        // stays in `state` until the oracle is next called.
        unsafe {
            *out_type = hegel_c::hegel_zone_type_t {
                utoff: local_type.utoff,
                is_dst: local_type.is_dst,
                abbreviation: view(state.abbreviation.as_bytes()),
            };
        }
        Ok(())
    })
}

unsafe extern "C" fn oracle_next_trampoline(
    user_data: *mut c_void,
    ctx: *mut hegel_c::HegelContext,
    after: i64,
    out_found: *mut bool,
    out_instant: *mut i64,
) -> i32 {
    // SAFETY: as for `oracle_offset_trampoline`.
    let state = unsafe { &mut *user_data.cast::<OracleState<'_, '_>>() };
    callback_result(ctx, "zone oracle", || {
        let next = state.callbacks.next_transition.as_mut().unwrap();
        if let Some(instant) = next(after)? {
            // SAFETY: both are libhegel's out-parameters.
            unsafe {
                *out_found = true;
                *out_instant = instant;
            }
        }
        Ok(())
    })
}

/// A zone source's loader, as [`ZoneSourceHandle::set_loader`] takes it.
pub(crate) type ZoneLoaderFn = dyn Fn(ZoneTarget, &str) -> Result<(), String> + Send + Sync;

unsafe extern "C" fn loader_trampoline(
    user_data: *mut c_void,
    ctx: *mut hegel_c::HegelContext,
    source: *mut hegel_c::HegelZoneSource,
    name: hegel_c::hegel_string_t,
) -> i32 {
    // SAFETY: user_data is the loader `set_loader` handed libhegel, which
    // keeps it alive until it calls `loader_release`, never during a call
    // of the loader, and it is only ever shared.
    let loader = unsafe { &*user_data.cast::<Box<ZoneLoaderFn>>() };
    let name = string_opt(name).unwrap_or_default();
    callback_result(ctx, &format!("zone loader for {name:?}"), || {
        loader(ZoneTarget { ctx, source }, &name)
    })
}

unsafe extern "C" fn loader_release(user_data: *mut c_void) {
    // SAFETY: user_data is the box `set_loader` handed libhegel, which
    // calls this exactly once, when no call of the loader is in flight or
    // will follow.
    let loader = unsafe { Box::from_raw(user_data.cast::<Box<ZoneLoaderFn>>()) };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(loader)));
}

/// A zone source to add data to, through a context: this thread's while
/// building a source, or the one libhegel hands a loader.
#[derive(Clone, Copy)]
pub(crate) struct ZoneTarget {
    ctx: *mut hegel_c::HegelContext,
    source: *mut hegel_c::HegelZoneSource,
}

impl ZoneTarget {
    fn check(&self, rc: hegel_result_t) -> Result<(), String> {
        rc_to_unit(rc).map_err(|_| context_error(self.ctx))
    }

    /// Add the zone `name` from a TZif file's bytes.
    pub(crate) fn add_tzif(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        // SAFETY: the name and bytes outlive the call, which copies them.
        self.check(unsafe {
            hegel_c::hegel_zone_source_add_tzif(
                self.ctx,
                self.source,
                view(name.as_bytes()),
                bytes.as_ptr(),
                bytes.len(),
            )
        })
    }

    /// Add the zone `name` from its data as fields, whose pointers must be
    /// valid for the call.
    pub(crate) fn add_zone(
        &self,
        name: &str,
        data: &hegel_c::hegel_zone_data_t,
    ) -> Result<(), String> {
        // SAFETY: the name and data outlive the call, which copies them.
        self.check(unsafe {
            hegel_c::hegel_zone_source_add_zone(self.ctx, self.source, view(name.as_bytes()), data)
        })
    }

    /// Add the zone `name` by probing an oracle, whose callbacks run on
    /// this thread during the call.
    pub(crate) fn add_oracle(
        &self,
        name: &str,
        callbacks: OracleCallbacks<'_, '_>,
    ) -> Result<(), String> {
        let next_transition: hegel_c::hegel_zone_next_transition_fn_t =
            match callbacks.next_transition {
                Some(_) => Some(oracle_next_trampoline),
                None => None,
            };
        let (first_year, last_year, after) =
            (callbacks.first_year, callbacks.last_year, callbacks.after);
        let mut state = OracleState {
            callbacks,
            abbreviation: String::new(),
        };
        let oracle = hegel_c::hegel_zone_oracle_t {
            offset_at: Some(oracle_offset_trampoline),
            next_transition,
            user_data: (&raw mut state).cast(),
            first_year,
            last_year,
            after,
        };
        // SAFETY: the oracle's state outlives the call, the only time
        // libhegel calls it, and the name is copied.
        self.check(unsafe {
            hegel_c::hegel_zone_source_add_oracle(
                self.ctx,
                self.source,
                view(name.as_bytes()),
                &oracle,
            )
        })
    }

    /// Make `name` a link to `target`.
    pub(crate) fn add_link(&self, name: &str, target: &str) -> Result<(), String> {
        // SAFETY: both names outlive the call, which copies them.
        self.check(unsafe {
            hegel_c::hegel_zone_source_add_link(
                self.ctx,
                self.source,
                view(name.as_bytes()),
                view(target.as_bytes()),
            )
        })
    }

    /// Declare names whose data comes later.
    pub(crate) fn declare(&self, names: &[String]) -> Result<(), String> {
        let views: Vec<hegel_c::hegel_string_t> =
            names.iter().map(|n| view(n.as_bytes())).collect();
        // SAFETY: the names outlive the call, which copies them.
        self.check(unsafe {
            hegel_c::hegel_zone_source_declare(self.ctx, self.source, views.as_ptr(), views.len())
        })
    }
}

/// An owned libhegel zone-source handle (`hegel_zone_source_t`), freed on
/// drop. libhegel keeps the source itself alive while the default or a
/// spec uses it, and may call its loader from any thread until then, so
/// the loader is libhegel's to release, not the handle's.
pub(crate) struct ZoneSourceHandle {
    raw: *mut hegel_c::HegelZoneSource,
}

// SAFETY: libhegel allows a source to be used and filled from any number
// of threads, and its loader is `Send + Sync`.
unsafe impl Send for ZoneSourceHandle {}
unsafe impl Sync for ZoneSourceHandle {}

impl std::fmt::Debug for ZoneSourceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZoneSourceHandle").finish_non_exhaustive()
    }
}

impl ZoneSourceHandle {
    /// An empty custom source.
    pub(crate) fn new() -> Self {
        let mut raw: *mut hegel_c::HegelZoneSource = ptr::null_mut();
        // SAFETY: &mut raw is a valid out-parameter.
        require_ok(with_context(|ctx| unsafe {
            hegel_c::hegel_zone_source_new(ctx, &mut raw)
        }));
        ZoneSourceHandle { raw }
    }

    /// A handle on the bundled database.
    pub(crate) fn bundled() -> Self {
        let mut raw: *mut hegel_c::HegelZoneSource = ptr::null_mut();
        // SAFETY: &mut raw is a valid out-parameter.
        require_ok(with_context(|ctx| unsafe {
            hegel_c::hegel_zone_source_bundled(ctx, &mut raw)
        }));
        ZoneSourceHandle { raw }
    }

    pub(crate) fn as_ptr(&self) -> *const hegel_c::HegelZoneSource {
        self.raw
    }

    /// Add data to this source through this thread's context.
    pub(crate) fn target(&self) -> ZoneTarget {
        with_context(|ctx| ZoneTarget {
            ctx,
            source: self.raw,
        })
    }

    /// Set the loader asked for the data of a name the source lacks,
    /// dropped when libhegel lets go of it: when another loader replaces
    /// it or the source's last holder goes.
    pub(crate) fn set_loader(&self, loader: Box<ZoneLoaderFn>) {
        let user_data: *mut Box<ZoneLoaderFn> = Box::into_raw(Box::new(loader));
        // SAFETY: libhegel owns the loader from here, whether or not the
        // call succeeds, and frees it through `loader_release` once no
        // call of it can follow. Only a custom source gets a loader, which
        // accepts one.
        require_ok(with_context(|ctx| unsafe {
            hegel_c::hegel_zone_source_set_loader(
                ctx,
                self.raw,
                Some(loader_trampoline),
                user_data.cast(),
                Some(loader_release),
            )
        }));
    }

    /// Record the release of the source's database.
    pub(crate) fn set_version(&self, version: &str) {
        // SAFETY: the version outlives the call, which copies it; a Rust
        // string is UTF-8 and only a custom source gets a version.
        require_ok(with_context(|ctx| unsafe {
            hegel_c::hegel_zone_source_set_version(ctx, self.raw, view(version.as_bytes()))
        }));
    }

    /// The release of the source's database, or an empty string.
    pub(crate) fn version(&self) -> String {
        let mut p = hegel_c::hegel_string_t::default();
        // SAFETY: &mut p is a valid out-parameter; the engine sets it to a
        // string valid for the program's lifetime.
        require_ok(with_context(|ctx| unsafe {
            hegel_c::hegel_zone_source_version(ctx, self.raw, &mut p)
        }));
        static_string(p)
    }

    /// Make this source the process's default.
    pub(crate) fn set_default(&self) {
        // SAFETY: libhegel keeps the source alive as the default.
        require_ok(with_context(|ctx| unsafe {
            hegel_c::hegel_set_default_zone_source(ctx, self.raw)
        }));
    }
}

impl Drop for ZoneSourceHandle {
    fn drop(&mut self) {
        // SAFETY: `raw` came from hegel_zone_source_new or _bundled and is
        // freed exactly once.
        free_on_drop(|ctx| unsafe { hegel_c::hegel_zone_source_free(ctx, self.raw) });
    }
}

/// The handle libhegel takes for a source, NULL meaning the default.
pub(super) fn source_ptr(source: Option<&ZoneSourceHandle>) -> *const hegel_c::HegelZoneSource {
    source.map_or(ptr::null(), ZoneSourceHandle::as_ptr)
}

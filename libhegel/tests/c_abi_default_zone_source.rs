//! The process-wide default zone source, in a binary of its own since
//! setting it affects every test in the process: queries without a source
//! and specs built after it is set read it, a spec's own source overrides
//! it, and a spec built earlier keeps the source it read until it is told
//! to read the default again.

mod common;

use common::{last_error, ok};
use hegel_c::hegel_result_t::*;
use hegel_c::{
    HegelContext, HegelTimezoneSpec, HegelZoneSource, hegel_context_free, hegel_context_new,
    hegel_set_default_zone_source, hegel_string_t, hegel_timezone_info, hegel_timezone_info_t,
    hegel_timezone_spec_free, hegel_timezone_spec_new, hegel_timezone_spec_set_names,
    hegel_timezone_spec_set_source, hegel_timezone_spec_validate, hegel_zone_source_add_tzif,
    hegel_zone_source_bundled, hegel_zone_source_free, hegel_zone_source_new,
    hegel_zone_source_set_loader,
};
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

fn s(text: &str) -> hegel_string_t {
    hegel_string_t {
        data: text.as_ptr().cast(),
        len: text.len(),
    }
}

unsafe extern "C" fn load_nothing(
    _user_data: *mut c_void,
    _ctx: *mut HegelContext,
    _source: *mut HegelZoneSource,
    _name: hegel_string_t,
) -> i32 {
    0
}

unsafe extern "C" fn count_release(user_data: *mut c_void) {
    let released = unsafe { &*user_data.cast::<AtomicUsize>() };
    released.fetch_add(1, Ordering::SeqCst);
}

unsafe fn known(ctx: *mut HegelContext, name: &str) -> bool {
    let mut out = hegel_timezone_info_t {
        known: false,
        primary_name: hegel_string_t::default(),
        is_link: false,
        rank: 0,
        tags: 0,
    };
    ok(unsafe { hegel_timezone_info(ctx, s(name), &mut out) });
    out.known
}

unsafe fn allows(ctx: *mut HegelContext, spec: *mut HegelTimezoneSpec, name: &str) -> bool {
    let names = [s(name)];
    ok(unsafe { hegel_timezone_spec_set_names(ctx, spec, names.as_ptr(), 1) });
    let rc = unsafe { hegel_timezone_spec_validate(ctx, spec) };
    assert!(rc == HEGEL_OK || rc == HEGEL_E_INVALID_ARG, "{rc:?}");
    rc == HEGEL_OK
}

unsafe fn new_spec(ctx: *mut HegelContext) -> *mut HegelTimezoneSpec {
    let mut spec = ptr::null_mut();
    ok(unsafe { hegel_timezone_spec_new(ctx, &mut spec) });
    spec
}

#[test]
fn the_default_source_is_read_where_no_source_is_given() {
    let ctx = hegel_context_new();
    unsafe {
        let earlier = new_spec(ctx);
        let reset = new_spec(ctx);
        let mut source = ptr::null_mut();
        ok(hegel_zone_source_new(ctx, &mut source));
        let bytes = jiff_tzdb::get("Europe/London").unwrap().1;
        ok(hegel_zone_source_add_tzif(
            ctx,
            source,
            s("Europe/Atlantis"),
            bytes.as_ptr(),
            bytes.len(),
        ));
        assert!(!known(ctx, "Europe/Atlantis"));
        ok(hegel_set_default_zone_source(ctx, source));
        ok(hegel_zone_source_free(ctx, source));
        assert!(known(ctx, "Europe/Atlantis"));
        assert!(!known(ctx, "Europe/London"));
        let later = new_spec(ctx);
        assert!(allows(ctx, later, "Europe/Atlantis"));
        assert!(!allows(ctx, later, "Europe/London"));
        assert!(!allows(ctx, earlier, "Europe/Atlantis"));
        assert!(allows(ctx, earlier, "Europe/London"));
        let mut bundled = ptr::null_mut();
        ok(hegel_zone_source_bundled(ctx, &mut bundled));
        let overridden = new_spec(ctx);
        ok(hegel_timezone_spec_set_source(ctx, overridden, bundled));
        assert!(allows(ctx, overridden, "Europe/London"));
        ok(hegel_timezone_spec_set_source(ctx, earlier, ptr::null()));
        assert_eq!(
            hegel_timezone_spec_validate(ctx, earlier),
            HEGEL_E_INVALID_ARG
        );
        assert!(
            last_error(ctx)
                .contains("hegel_timezone_spec_validate: unknown time zone \"Europe/London\""),
            "{}",
            last_error(ctx)
        );
        assert!(allows(ctx, earlier, "Europe/Atlantis"));
        ok(hegel_timezone_spec_set_source(ctx, reset, ptr::null()));
        assert!(allows(ctx, reset, "Europe/Atlantis"));
        ok(hegel_set_default_zone_source(ctx, ptr::null()));
        assert!(known(ctx, "Europe/London"));
        assert!(allows(ctx, later, "Europe/Atlantis"));
        for spec in [earlier, reset, later, overridden] {
            ok(hegel_timezone_spec_free(ctx, spec));
        }
        let released = AtomicUsize::new(0);
        let mut loaded = ptr::null_mut();
        ok(hegel_zone_source_new(ctx, &mut loaded));
        ok(hegel_zone_source_set_loader(
            ctx,
            loaded,
            Some(load_nothing),
            (&released as *const AtomicUsize).cast_mut().cast(),
            Some(count_release),
        ));
        ok(hegel_set_default_zone_source(ctx, loaded));
        ok(hegel_zone_source_free(ctx, loaded));
        assert!(!known(ctx, "Europe/London"));
        assert_eq!(released.load(Ordering::SeqCst), 0);
        ok(hegel_set_default_zone_source(ctx, ptr::null()));
        assert_eq!(released.load(Ordering::SeqCst), 1);
        assert!(known(ctx, "Europe/London"));
        ok(hegel_zone_source_free(ctx, bundled));
        ok(hegel_context_free(ctx));
    }
}

//! The process-wide default zone source, in a binary of its own since
//! setting it affects every test in the process: queries without a source
//! and specs built after it is set read it, a spec's own source overrides
//! it, and a spec built earlier keeps the source it read.

mod common;

use common::ok;
use hegel_c::hegel_result_t::*;
use hegel_c::{
    HegelContext, HegelTimezoneSpec, hegel_context_free, hegel_context_new,
    hegel_set_default_zone_source, hegel_string_t, hegel_timezone_info, hegel_timezone_info_t,
    hegel_timezone_spec_free, hegel_timezone_spec_new, hegel_timezone_spec_set_names,
    hegel_timezone_spec_set_source, hegel_zone_source_add_tzif, hegel_zone_source_bundled,
    hegel_zone_source_free, hegel_zone_source_new,
};
use std::ptr;

fn s(text: &str) -> hegel_string_t {
    hegel_string_t {
        data: text.as_ptr().cast(),
        len: text.len(),
    }
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
    let rc = unsafe { hegel_timezone_spec_set_names(ctx, spec, names.as_ptr(), 1) };
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
        assert_eq!(
            hegel_timezone_spec_set_source(ctx, earlier, ptr::null()),
            HEGEL_E_INVALID_ARG
        );
        assert!(allows(ctx, earlier, "Europe/London"));
        ok(hegel_timezone_spec_set_source(ctx, reset, ptr::null()));
        assert!(allows(ctx, reset, "Europe/Atlantis"));
        ok(hegel_set_default_zone_source(ctx, ptr::null()));
        assert!(known(ctx, "Europe/London"));
        assert!(allows(ctx, later, "Europe/Atlantis"));
        for spec in [earlier, reset, later, overridden] {
            ok(hegel_timezone_spec_free(ctx, spec));
        }
        ok(hegel_zone_source_free(ctx, bundled));
        ok(hegel_context_free(ctx));
    }
}

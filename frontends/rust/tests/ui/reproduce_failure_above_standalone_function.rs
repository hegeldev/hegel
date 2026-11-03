// `#[hegel::reproduce_failure]` must appear below
// `#[hegel::standalone_function]`, not above it.

#[hegel::reproduce_failure("AAEC")]
#[hegel::standalone_function]
fn check(tc: hegel::TestCase) {
    let _ = tc;
}

fn main() {}

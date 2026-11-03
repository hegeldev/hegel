//! Fixture binary: `fixture_main_failing` with a `#[hegel::reproduce_failure]`
//! below `#[hegel::main]`, replaying the blob in `HEGEL_TEST_REPRO_BLOB`.

use hegel::TestCase;
use hegel::generators as gs;

#[hegel::main]
#[hegel::reproduce_failure(std::env::var("HEGEL_TEST_REPRO_BLOB").unwrap())]
fn main(tc: TestCase) {
    let x: i32 = tc.draw(gs::integers::<i32>().min_value(0).max_value(50));
    assert!(x < 0, "got nonneg {}", x);
}

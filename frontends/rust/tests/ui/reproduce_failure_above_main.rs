// `#[hegel::reproduce_failure]` must appear below `#[hegel::main]`, not
// above it.

#[hegel::reproduce_failure("AAEC")]
#[hegel::main]
fn run(tc: hegel::TestCase) {
    let _ = tc;
}

fn main() {}

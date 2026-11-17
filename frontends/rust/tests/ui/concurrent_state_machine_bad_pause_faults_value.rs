// The value of `pause_faults` must be a boolean literal.

struct Machine;

#[hegel::concurrent_state_machine]
impl Machine {
    #[rule]
    fn act(&self, _: hegel::TestCase) {}

    #[invariant(always_run, pause_faults = 1)]
    fn check(&self, _: hegel::TestCase) {}
}

fn main() {}

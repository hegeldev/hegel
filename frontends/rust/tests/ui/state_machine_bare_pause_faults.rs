// `pause_faults` takes a boolean value: `pause_faults = true`, not the bare
// flag form `always_run` uses.

struct Machine;

#[hegel::state_machine]
impl Machine {
    #[rule]
    fn act(&mut self, _: hegel::TestCase) {}

    #[invariant(pause_faults)]
    fn check(&self, _: hegel::TestCase) {}
}

fn main() {}

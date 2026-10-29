// `groups = [...]` must name at least one group.

struct Machine;

#[hegel::concurrent_state_machine]
impl Machine {
    #[rule(groups = [])]
    fn act(&self, _: hegel::TestCase) {}
}

fn main() {}

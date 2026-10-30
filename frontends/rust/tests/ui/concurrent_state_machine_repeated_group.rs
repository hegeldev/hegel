// `groups = [...]` must not list a group twice.

struct Machine;

#[hegel::concurrent_state_machine]
impl Machine {
    #[rule(groups = ["rw", "audit", "rw"])]
    fn act(&self, _: hegel::TestCase) {}
}

fn main() {}

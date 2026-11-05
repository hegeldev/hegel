// A parenthesized #[rule(...)] on a concurrent state machine accepts only
// `groups = [...]`: a single group is a one-element list.

struct Machine;

#[hegel::concurrent_state_machine]
impl Machine {
    #[rule(group = "rw")]
    fn act(&self, _: hegel::TestCase) {}
}

fn main() {}

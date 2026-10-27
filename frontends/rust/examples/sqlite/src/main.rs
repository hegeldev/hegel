//! This is a "workload" version of the test (a standalone binary that runs only a single test case
//! against a fresh SQLite database). Exits nonzero if any failure is detected.

use hegel::TestCase;
use sqlite_bank::{WorkloadConfig, bank_machine};

#[hegel::main]
fn main(tc: TestCase) {
    let config = WorkloadConfig::draw(&tc);
    bank_machine(&tc, config).run_concurrent(tc);
}

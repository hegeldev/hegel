use hegel::HealthCheck;
use hegel::TestCase;
use sqlite_bank::{WorkloadConfig, bank_machine};

#[hegel::test(test_cases = 10, suppress_health_check = [HealthCheck::TooSlow])]
fn bank_workload(tc: TestCase) {
    let config = WorkloadConfig::from_env_or_draw(&tc);
    bank_machine(&tc, config).run_concurrent(tc);
}

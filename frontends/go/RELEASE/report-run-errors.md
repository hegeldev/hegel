RELEASE_TYPE: patch

This patch fixes `hegel.Test` failing a test without saying why when the run ends without a verdict on the property, such as a failed health check, a flaky test or an engine error. The test now fails with the run's error message, for example `FailedHealthCheck: FilterTooMuch` ([#116](https://github.com/hegeldev/hegel/issues/116)).

/**
 * Errors a run throws when the engine aborts it without a verdict on the
 * property. A failing property throws the test's own error instead.
 *
 * @packageDocumentation
 */

import { HealthCheck, HEALTH_CHECK_TO_NATIVE } from "./settings.js";
import { RunErrorKind, type NativeRunErrorKind } from "./engine.js";

/**
 * The engine aborted the run rather than reporting a pass or a test failure:
 * a failed health check ({@link FailedHealthCheckError}), nondeterminism under
 * `NondeterminismStrictness.Error` ({@link FlakyError}), too few valid test
 * cases ({@link UnsatisfiableError}), or another engine error. The message
 * explains which.
 */
export class RunError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RunError";
  }
}

/**
 * A health check failed: the run was likely to test poorly or slowly (too many
 * rejected test cases, say). The message explains how to suppress the check
 * with `suppressHealthCheck` if the behaviour is intentional.
 */
export class FailedHealthCheckError extends RunError {
  /** The check that failed, or `null` if the engine did not say. */
  readonly healthCheck: HealthCheck | null;
  constructor(message: string, healthCheck: HealthCheck | null) {
    super(message);
    this.name = "FailedHealthCheckError";
    this.healthCheck = healthCheck;
  }
}

/**
 * The test behaved nondeterministically and the run was aborted, under
 * `NondeterminismStrictness.Error`. The default strictness instead confirms
 * such failures by replay and reports them with a caveat.
 */
export class FlakyError extends RunError {
  constructor(message: string) {
    super(message);
    this.name = "FlakyError";
  }
}

/**
 * The engine could not find enough test cases satisfying the test's
 * assumptions and filters.
 */
export class UnsatisfiableError extends RunError {
  constructor(message: string) {
    super(message);
    this.name = "UnsatisfiableError";
  }
}

/**
 * None of the failure blobs a test was told to replay with
 * `Hegel.reproduceFailure` failed: the failure may have been fixed, or, for a
 * nondeterministic one, not recurred within the replay budget.
 */
export class DidNotReproduceError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DidNotReproduceError";
  }
}

function healthCheckFromNative(bit: number): HealthCheck | null {
  for (const [check, value] of Object.entries(HEALTH_CHECK_TO_NATIVE)) {
    if (value === bit) return check as HealthCheck;
  }
  return null;
}

/** The error to throw for a run that ended in `RunStatus.ERROR`. */
export function runError(message: string | null, kind: NativeRunErrorKind): RunError {
  const text = message ?? "The run failed with an unknown error";
  switch (kind.kind) {
    case RunErrorKind.HEALTH_CHECK:
      return new FailedHealthCheckError(text, healthCheckFromNative(kind.healthCheck));
    case RunErrorKind.FLAKY:
    case RunErrorKind.NONDETERMINISTIC:
      return new FlakyError(text);
    case RunErrorKind.UNSATISFIABLE:
      return new UnsatisfiableError(text);
    default:
      return new RunError(text);
  }
}

package dev.hegel;

/**
 * A usage error detected while a test runs, such as an argument the engine rejects when a value is
 * drawn: the run fails with it at once instead of shrinking it as a counterexample.
 */
class InvalidArgument extends IllegalArgumentException {
    InvalidArgument(String message) {
        super(message);
    }
}

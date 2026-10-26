package dev.hegel;

/** A usage error in an explicit {@link Example}: it does not fit the test body it is run with. */
final class InvalidExample extends InvalidArgument {
    InvalidExample(String message) {
        super(message);
    }
}

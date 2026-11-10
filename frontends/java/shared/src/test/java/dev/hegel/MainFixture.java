package dev.hegel;

import static dev.hegel.Generators.integers;

/**
 * A program written with {@link Hegel#main}, run by {@link MainProcessTest} in a child JVM. The
 * first argument picks what the test case does: {@code pass} or {@code fail}. The rest is the
 * program's command line. Each test case prints {@code fixture: test case} to stdout.
 */
public final class MainFixture {
    private MainFixture() {}

    /**
     * Entry point.
     *
     * @param args {@code pass} or {@code fail}, then the program's command line
     */
    public static void main(String[] args) {
        boolean fail = args[0].equals("fail");
        String[] rest = java.util.Arrays.copyOfRange(args, 1, args.length);
        Hegel.main(rest, tc -> {
            System.out.println("fixture: test case");
            int n = tc.draw(integers(), "n");
            if (fail) {
                throw new AssertionError("failed with n=" + n);
            }
        });
    }
}

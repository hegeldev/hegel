package dev.hegel;

import dev.hegel.lowlevel.Abi;
import dev.hegel.lowlevel.Libhegel;
import dev.hegel.lowlevel.LibhegelException;
import java.io.PrintStream;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import java.util.function.Consumer;
import java.util.function.IntConsumer;

/**
 * The command line of a program written with {@link Hegel#main}. {@link #run} does everything
 * except exit the process, so tests can call it directly.
 *
 * <p>Options other than {@code --profile} and {@code --failure-blob} are passed to the engine by
 * name, so every engine setting works as an option.
 */
final class Main {
    private static final int EXIT_PASSED = 0;
    private static final int EXIT_FAILED = 1;
    private static final int EXIT_USAGE = 2;

    /** {@link Runner#STALE_BLOB} for a blob given by {@code --failure-blob}. */
    private static final String STALE_BLOB = "the failure blob did not reproduce a failure. The failure may"
            + " have been fixed, or, for a nondeterministic blob, it may not have recurred within the"
            + " replay budget. Re-run to retry, or stop passing --failure-blob once the failure is"
            + " fixed.";

    /** Ends the process. A field so tests can call {@link Hegel#main} in their own JVM. */
    static IntConsumer exit = System::exit;

    private Main() {}

    /** A command line that cannot run. */
    private static final class UsageError extends RuntimeException {
        private static final long serialVersionUID = 1L;

        UsageError(String message) {
            super(message);
        }
    }

    /** An option that sets a setting. {@code value} is {@code null} if the option has none. */
    private record SettingOption(String flag, String key, String value) {}

    /** The parsed command line. */
    private static final class Arguments {
        boolean help;
        String profile;
        String failureBlob;
        final List<SettingOption> settings = new ArrayList<>();
    }

    /**
     * Run {@code body} once under the command line {@code argv}.
     *
     * @param program the name to show in the usage text and to derive the database key from
     * @param out receives the usage text when it is asked for
     * @param err receives the report and any errors
     * @return the status the process should exit with
     */
    static int run(
            Libhegel lib,
            String[] argv,
            Consumer<TestCase> body,
            Settings settings,
            String program,
            PrintStream out,
            PrintStream err) {
        try {
            Arguments args = parse(argv);
            if (args.help) {
                out.print(usage(program));
                return EXIT_PASSED;
            }
            if (args.profile != null) {
                selectProfile(lib, args.profile);
            }
            Settings effective = settings.name == null ? settings.name(program + "#main") : settings;
            if (args.failureBlob != null) {
                effective = effective.reproduceFailure(args.failureBlob);
            }
            RunReport report =
                    Runner.run(lib, effective, body, new PrintingReporter(err, true), s -> configure(lib, s, args));
            report.error().ifPresent(error -> err.println("error: " + error));
            return report.passed() ? EXIT_PASSED : EXIT_FAILED;
        } catch (UsageError e) {
            err.println("error: " + e.getMessage());
            err.println();
            err.print(usage(program));
            return EXIT_USAGE;
        } catch (LibhegelException | IllegalArgumentException e) {
            boolean stale = Runner.STALE_BLOB.equals(e.getMessage());
            err.println("error: " + (stale ? STALE_BLOB : e.getMessage()));
            return EXIT_FAILED;
        }
    }

    private static Arguments parse(String[] argv) {
        Arguments args = new Arguments();
        for (int i = 0; i < argv.length; i++) {
            String arg = argv[i];
            if (arg.equals("-h") || arg.equals("--help")) {
                args.help = true;
                return args;
            }
            if (!arg.startsWith("--") || arg.equals("--")) {
                throw new UsageError("unknown argument: " + arg);
            }
            String option = arg.substring(2);
            int equals = option.indexOf('=');
            String name = equals < 0 ? option : option.substring(0, equals);
            String value = null;
            if (equals >= 0) {
                value = option.substring(equals + 1);
            } else if (i + 1 < argv.length && !argv[i + 1].startsWith("--")) {
                value = argv[++i];
            }
            String flag = "--" + name;
            if (name.contains("_") || name.equals("test-cases")) {
                throw new UsageError("unknown argument: " + flag);
            }
            if (name.equals("profile")) {
                args.profile = once(flag, args.profile, value);
            } else if (name.equals("failure-blob")) {
                args.failureBlob = once(flag, args.failureBlob, value);
            } else {
                args.settings.add(new SettingOption(flag, name.replace('-', '_'), value));
            }
        }
        return args;
    }

    /** The value of an option that takes a value and can be given once. */
    private static String once(String flag, String existing, String value) {
        if (value == null || value.isEmpty()) {
            throw new UsageError(flag + " requires a value");
        }
        if (existing != null) {
            throw new UsageError(flag + " can be given only once");
        }
        return value;
    }

    /** Make {@code name} the default profile, checking that it resolves. */
    private static void selectProfile(Libhegel lib, String name) {
        int rc = lib.setDefaultProfile(name);
        if (rc == Abi.OK) {
            long[] handle = new long[1];
            rc = lib.settingsNew(handle);
            if (rc == Abi.OK) {
                lib.settingsFree(handle[0]);
                return;
            }
        }
        throw new UsageError("--profile: " + lastError(lib));
    }

    /**
     * Apply the options to the settings handle {@code s}. Each run of the program is one test case,
     * so this sets the number of test cases to 1 and suppresses the two health checks that measure
     * runs of many test cases.
     */
    private static void configure(Libhegel lib, long s, Arguments args) {
        for (SettingOption option : args.settings) {
            apply(lib, s, option);
        }
        lib.settingsTestCases(s, 1);
        lib.settingsSuppressHealthCheck(
                s, lib.settingsGetSuppressHealthCheck(s) | Abi.HC_TOO_SLOW | Abi.HC_TEST_CASES_TOO_LARGE);
    }

    private static void apply(Libhegel lib, long s, SettingOption option) {
        String value = Objects.toString(option.value, "");
        boolean accepted = lib.settingsFromString(s, option.key, value) == Abi.OK;
        String error = accepted ? "" : lastError(lib).replaceFirst("^hegel_settings_set_from_string: ", "");
        if (error.startsWith("unknown setting")) {
            throw new UsageError("unknown argument: " + option.flag);
        }
        if (option.value == null) {
            throw new UsageError(option.flag + " requires a value");
        }
        if (option.key.equals("database_key") && value.isEmpty()) {
            throw new UsageError("--database-key expects a non-empty key");
        }
        if (!accepted) {
            // The engine's message starts with the key; replace it with the option's name.
            String reason =
                    error.startsWith("`" + option.key + "` ") ? error.substring(option.key.length() + 3) : error;
            throw new UsageError(option.flag + " " + reason);
        }
    }

    private static String lastError(Libhegel lib) {
        return Objects.toString(lib.lastErrorMessage(), "");
    }

    static String usage(String program) {
        return "Usage: java " + program + " [options]\n" + """

                Hegel property-based testing binary. Runs exactly one test case.

                Options:
                  --profile <NAME>                       Set the default settings profile, like
                                                         the HEGEL_DEFAULT_PROFILE environment
                                                         variable; other flags apply on top
                  --failure-blob <BLOB>                  Replay the failure that BLOB encodes
                  --seed <N|none>                        Seed for randomisation ('none' for unset)
                  --derandomize <BOOL>                   Use a deterministic derived seed
                  --database <PATH|disabled|default>     Database path for failing-example storage
                  --database-key <KEY>                   Key the failing examples are stored under
                  --verbosity <LEVEL>                    quiet | normal | verbose | debug
                  --phases <NAMES>                       Comma-separated phases to run: explicit,
                                                         reuse, generate, target, shrink
                  --suppress-health-check <NAMES>        Comma-separated health check names
                                                         (filter_too_much, too_slow,
                                                         test_cases_too_large,
                                                         large_initial_test_case), or 'all'
                  --report-multiple-failures <BOOL>      Report every distinct failure found
                  --stop-on-first-failure <BOOL>         End generation once a failure is found
                  --show-statistics <BOOL>               Print the run's statistics
                  --print-blob <BOOL>                    Print the failure's reproduce blob
                  --backend <default|urandom>            Randomness source (urandom: the Antithesis
                                                         fuzzer, else /dev/urandom)
                  --nondeterminism-strictness <LEVEL>    quiet | warn | error
                  --unbounded-choices <BOOL>             Remove the limit on choices per test case
                  -h, --help                             Show this help and exit

                Every flag also takes the form --flag=VALUE. A BOOL is true, false, 1, 0,
                yes or no. Values use the vocabulary of the matching hegel.toml key.
                """;
    }
}

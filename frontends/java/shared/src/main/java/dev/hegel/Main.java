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
 * {@link #run} does everything except exit the process, so tests can call it directly.
 */
final class Main {
    private static final int EXIT_PASSED = 0;
    private static final int EXIT_FAILED = 1;
    private static final int EXIT_USAGE = 2;

    private static final String STALE_BLOB = "the failure blob did not reproduce a failure. The failure may"
            + " have been fixed, or, for a nondeterministic blob, it may not have recurred within the"
            + " replay budget. Re-run to retry, or stop passing --failure-blob once the failure is"
            + " fixed.";

    /**
     * Exits the process. Tests replace it with a function that records the status, because JaCoCo
     * does not mark a call to {@code System.exit} as covered.
     */
    static IntConsumer exit = System::exit;

    private Main() {}

    private static final class UsageError extends RuntimeException {
        private static final long serialVersionUID = 1L;

        UsageError(String message) {
            super(message);
        }
    }

    /** An option that sets a setting. {@code value} is {@code null} if the option has none. */
    private record SettingOption(String flag, String key, String value) {}

    private record Arguments(boolean help, String profile, String failureBlob, List<SettingOption> settings) {
        static final Arguments HELP = new Arguments(true, null, null, List.of());
    }

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
            if (args.help()) {
                out.print(usage(program));
                return EXIT_PASSED;
            }
            if (args.profile() != null) {
                selectProfile(lib, args.profile());
            }
            Settings effective = settings.name == null ? settings.name(program + "#main") : settings;
            if (args.failureBlob() != null) {
                effective = effective.reproduceFailure(args.failureBlob());
            }
            RunReport report = Runner.run(
                    lib, effective, body, PrintingReporter.forMain(err), s -> configure(lib, s, args.settings()));
            report.error().ifPresent(error -> err.println("error: " + error));
            return report.passed() ? EXIT_PASSED : EXIT_FAILED;
        } catch (UsageError e) {
            err.println("error: " + e.getMessage());
            err.println();
            err.print(usage(program));
            return EXIT_USAGE;
        } catch (BlobNotReproducedException e) {
            err.println("error: "
                    + (e.stale() ? STALE_BLOB : "the failure blob replay ended in an error: " + e.replayError()));
            return EXIT_FAILED;
        } catch (LibhegelException | IllegalArgumentException e) {
            err.println("error: " + e.getMessage());
            return EXIT_FAILED;
        }
    }

    private static Arguments parse(String[] argv) {
        String profile = null;
        String failureBlob = null;
        List<SettingOption> settings = new ArrayList<>();
        for (int i = 0; i < argv.length; i++) {
            String arg = argv[i];
            if (arg.equals("-h") || arg.equals("--help")) {
                return Arguments.HELP;
            }
            if (!arg.startsWith("--") || arg.equals("--")) {
                throw new UsageError("unknown argument: " + arg);
            }
            String[] nameAndValue = arg.substring(2).split("=", 2);
            String name = nameAndValue[0];
            String flag = "--" + name;
            String value = null;
            if (nameAndValue.length == 2) {
                value = nameAndValue[1];
            } else if (i + 1 < argv.length && !argv[i + 1].startsWith("--")) {
                value = argv[++i];
            }
            switch (name) {
                case "profile" -> profile = once(flag, profile, value);
                case "failure-blob" -> failureBlob = once(flag, failureBlob, value);
                case "test-cases" -> throw new UsageError("unknown argument: " + flag);
                default -> {
                    if (name.contains("_")) {
                        throw new UsageError("unknown argument: " + flag);
                    }
                    settings.add(new SettingOption(flag, name.replace('-', '_'), value));
                }
            }
        }
        return new Arguments(false, profile, failureBlob, List.copyOf(settings));
    }

    private static String once(String flag, String existing, String value) {
        if (value == null || value.isEmpty()) {
            throw new UsageError(flag + " requires a value");
        }
        if (existing != null) {
            throw new UsageError(flag + " can be given only once");
        }
        return value;
    }

    /**
     * If {@code name} does not resolve, resolving again without it tells a bad profile apart from
     * invalid {@code HEGEL_*} variables or {@code hegel.toml}, which are not a usage error.
     */
    private static void selectProfile(Libhegel lib, String name) {
        if (lib.setDefaultProfile(name) != Abi.OK) {
            throw new UsageError("--profile: " + lastError(lib));
        }
        long[] handle = new long[1];
        if (lib.settingsNew(handle) == Abi.OK) {
            lib.settingsFree(handle[0]);
            return;
        }
        String error = lastError(lib);
        lib.setDefaultProfile("");
        lib.settingsFree(Runner.newSettings(lib));
        throw new UsageError("--profile: " + error);
    }

    /**
     * Each run of the program is one test case, so the two health checks that measure runs of many
     * test cases are suppressed.
     */
    private static void configure(Libhegel lib, long s, List<SettingOption> options) {
        for (SettingOption option : options) {
            apply(lib, s, option);
        }
        lib.settingsTestCases(s, 1);
        lib.settingsSuppressHealthCheck(
                s, lib.settingsGetSuppressHealthCheck(s) | Abi.HC_TOO_SLOW | Abi.HC_TEST_CASES_TOO_LARGE);
    }

    /**
     * An option without a value is still sent to the engine, as the empty string, so that an unknown
     * option is reported as unknown rather than as missing its value.
     */
    private static void apply(Libhegel lib, long s, SettingOption option) {
        String error = setFromString(lib, s, option.key(), Objects.requireNonNullElse(option.value(), ""));
        if (error != null && error.startsWith("unknown setting")) {
            throw new UsageError("unknown argument: " + option.flag());
        }
        if (option.value() == null) {
            throw new UsageError(option.flag() + " requires a value");
        }
        if (error != null) {
            throw new UsageError(option.flag() + " " + withoutKey(error, option.key()));
        }
        if (option.key().equals("database_key") && option.value().isEmpty()) {
            throw new UsageError("--database-key expects a non-empty key");
        }
    }

    private static String setFromString(Libhegel lib, long s, String key, String value) {
        if (lib.settingsFromString(s, key, value) == Abi.OK) {
            return null;
        }
        return lastError(lib).replaceFirst("^hegel_settings_set_from_string: ", "");
    }

    private static String withoutKey(String message, String key) {
        String prefix = "`" + key + "` ";
        return message.startsWith(prefix) ? message.substring(prefix.length()) : message;
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

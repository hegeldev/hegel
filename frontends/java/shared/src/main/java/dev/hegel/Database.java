package dev.hegel;

/**
 * The example-database setting: unset, the default location, disabled, or a directory path.
 *
 * <p>This is a closed four-state value — construct one with {@link #unset()}, {@link
 * #defaultLocation()}, {@link #disabled()}, or {@link #path(String)} and hand it to {@link
 * Settings#database(Database)}. Because the states are mutually exclusive values of a single
 * setting, you cannot express a contradiction (such as "disabled, but at this path").
 *
 * <ul>
 *   <li>{@link #unset()} — keep the database of the settings profile (the default location, {@code
 *       .hegel/examples}, except under the {@code ci} and {@code workload} profiles, which disable
 *       it).
 *   <li>{@link #defaultLocation()} — use the default location, {@code .hegel/examples}, even where
 *       the profile disables the database.
 *   <li>{@link #disabled()} — no persistence and no replay.
 *   <li>{@link #path(String)} — use the given directory as the database root.
 * </ul>
 */
public final class Database {
    /**
     * Sentinel string for {@link HegelTest#database()} that disables the database entirely — the
     * annotation equivalent of {@link #disabled()} (annotation attributes can't carry a {@code
     * Database} value). Distinct enough that it can never collide with a real directory path.
     */
    public static final String DISABLED = "__hegel_database_disabled__";

    /**
     * Sentinel string for {@link HegelTest#database()} that selects the default database location —
     * the annotation equivalent of {@link #defaultLocation()}.
     */
    public static final String DEFAULT_LOCATION = "__hegel_database_default__";

    enum Kind {
        UNSET,
        DEFAULT_LOCATION,
        DISABLED,
        PATH
    }

    final Kind kind;
    final String path; // non-null only when kind == PATH

    private Database(Kind kind, String path) {
        this.kind = kind;
        this.path = path;
    }

    /**
     * Keep the database of the settings profile ({@code .hegel/examples}, or disabled under the
     * {@code ci} and {@code workload} profiles).
     *
     * @return the unset database setting
     */
    public static Database unset() {
        return new Database(Kind.UNSET, null);
    }

    /**
     * Use the default database location, {@code .hegel/examples}, even under a profile that disables
     * the database.
     *
     * @return the default-location database setting
     */
    public static Database defaultLocation() {
        return new Database(Kind.DEFAULT_LOCATION, null);
    }

    /**
     * Disable the example database entirely — no persistence, no replay.
     *
     * @return the disabled database setting
     */
    public static Database disabled() {
        return new Database(Kind.DISABLED, null);
    }

    /**
     * Use the directory at {@code path} as the example database root.
     *
     * @param path the database directory
     * @return a database setting pointing at {@code path}
     */
    public static Database path(String path) {
        return new Database(Kind.PATH, path);
    }
}

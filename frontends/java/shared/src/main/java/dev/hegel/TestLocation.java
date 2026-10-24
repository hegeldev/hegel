package dev.hegel;

/**
 * Where a property test lives: its source file, the line its definition begins on, the class
 * enclosing it and its name. Inside <a href="https://antithesis.com/">Antithesis</a> the engine
 * reports each run's verdict as an assertion identified as {@code <className>::<function> passes
 * properties} and located at {@code file:beginLine}; elsewhere the location is unused.
 *
 * <p>{@link Hegel#test} and {@link Hegel#run} locate the test at the call site, and {@link
 * HegelTest} at the annotated method, unless {@link Settings#testLocation(TestLocation)} gives one.
 *
 * @param file the source file, relative to its source root (for example {@code
 *     com/example/CartTest.java})
 * @param beginLine the line the test's definition begins on, or {@code 0} if unknown
 * @param className the fully-qualified name of the class enclosing the test
 * @param function the test's name
 */
public record TestLocation(String file, int beginLine, String className, String function) {
    /**
     * The location of code in method {@code function} of class {@code className}, at line {@code
     * line} of source file {@code fileName} ({@code null} when unknown: the conventional {@code
     * <TopLevelClass>.java}). The file is placed under the class's package directory.
     */
    static TestLocation of(String className, String fileName, int line, String function) {
        int dot = className.lastIndexOf('.');
        String file = fileName;
        if (file == null) {
            String simple = className.substring(dot + 1);
            int nested = simple.indexOf('$');
            file = (nested < 0 ? simple : simple.substring(0, nested)) + ".java";
        }
        String directory = className.substring(0, dot + 1).replace('.', '/');
        return new TestLocation(directory + file, line, className, function);
    }
}

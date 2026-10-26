package dev.hegel;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.EnumSet;
import java.util.List;
import java.util.stream.Stream;
import org.junit.jupiter.api.extension.Extension;
import org.junit.jupiter.api.extension.ExtensionContext;
import org.junit.jupiter.api.extension.InvocationInterceptor;
import org.junit.jupiter.api.extension.ParameterContext;
import org.junit.jupiter.api.extension.ParameterResolver;
import org.junit.jupiter.api.extension.ReflectiveInvocationContext;
import org.junit.jupiter.api.extension.TestTemplateInvocationContext;
import org.junit.jupiter.api.extension.TestTemplateInvocationContextProvider;
import org.junit.platform.commons.support.ReflectionSupport;

/**
 * JUnit 5 extension backing {@link HegelTest}. Reports the property as a single test entry and
 * drives the engine loop, invoking the user method once per generated input.
 *
 * @hidden
 */
public final class HegelTestExtension implements TestTemplateInvocationContextProvider {

    @Override
    public boolean supportsTestTemplate(ExtensionContext context) {
        return isHegelTest(context.getTestMethod().orElse(null));
    }

    static boolean isHegelTest(Method method) {
        return method != null && method.isAnnotationPresent(HegelTest.class);
    }

    @Override
    public Stream<TestTemplateInvocationContext> provideTestTemplateInvocationContexts(ExtensionContext context) {
        return Stream.of(new HegelInvocationContext());
    }

    private static final class HegelInvocationContext implements TestTemplateInvocationContext {
        @Override
        public String getDisplayName(int invocationIndex) {
            return "hegel";
        }

        @Override
        public List<Extension> getAdditionalExtensions() {
            return List.of(new TestCaseResolver(), new PropertyInterceptor());
        }
    }

    /**
     * Resolves the {@link TestCase} parameter so JUnit accepts the method shape; the value is unused.
     */
    private static final class TestCaseResolver implements ParameterResolver {
        @Override
        public boolean supportsParameter(ParameterContext pc, ExtensionContext ec) {
            return isTestCaseParam(pc.getParameter().getType());
        }

        @Override
        public Object resolveParameter(ParameterContext pc, ExtensionContext ec) {
            return null;
        }
    }

    private static final class PropertyInterceptor implements InvocationInterceptor {
        @Override
        public void interceptTestTemplateMethod(
                Invocation<Void> invocation,
                ReflectiveInvocationContext<Method> invocationContext,
                ExtensionContext extensionContext)
                throws Throwable {
            invocation.skip();
            Method method = invocationContext.getExecutable();
            HegelTest ann = method.getAnnotation(HegelTest.class);
            Object target = invocationContext.getTarget().orElse(null);
            Class<?> testClass = extensionContext.getRequiredTestClass();
            Settings settings = settingsFrom(ann, method, testClass).examples(examplesFrom(ann, testClass, target));
            Hegel.test(tc -> ReflectionSupport.invokeMethod(method, target, tc), settings);
        }
    }

    /**
     * The settings {@code ann} gives {@code method} run as a test of {@code testClass}, the class
     * whose instance runs it, which for an inherited test is a subclass of the method's own. Each
     * class running an inherited test keys its database entry by its own name.
     */
    static Settings settingsFrom(HegelTest ann, Method method, Class<?> testClass) {
        String className = testClass.getName();
        String name = ann.name().isEmpty() ? className + "." + method.getName() : ann.name();
        Settings s = ann.profile().isEmpty() ? new Settings() : Settings.fromProfile(ann.profile());
        s = s.verbosity(ann.verbosity())
                .backend(ann.backend())
                .nondeterminismStrictness(ann.nondeterminismStrictness())
                .name(name)
                .testLocation(MethodLocation.of(method, testClass))
                .annotated();
        // The annotation cannot express "unset" for a primitive: 0 test cases (its default) leaves
        // the engine's profile / HEGEL_TEST_CASES value in place.
        if (ann.testCases() != 0) {
            s = s.testCases(ann.testCases());
        }
        if (ann.printBlob() != OptBoolean.DEFAULT) {
            s = s.printBlob(ann.printBlob() == OptBoolean.TRUE);
        }
        if (ann.reportMultipleFailures() != OptBoolean.DEFAULT) {
            s = s.reportMultipleFailures(ann.reportMultipleFailures() == OptBoolean.TRUE);
        }
        if (ann.stopOnFirstFailure() != OptBoolean.DEFAULT) {
            s = s.stopOnFirstFailure(ann.stopOnFirstFailure() == OptBoolean.TRUE);
        }
        if (ann.showStatistics() != OptBoolean.DEFAULT) {
            s = s.showStatistics(ann.showStatistics() == OptBoolean.TRUE);
        }
        if (ann.unboundedChoices() != OptBoolean.DEFAULT) {
            s = s.unboundedChoices(ann.unboundedChoices() == OptBoolean.TRUE);
        }
        if (ann.suppressHealthCheck().length > 0) {
            s = s.suppressHealthCheck(ann.suppressHealthCheck());
        }
        if (ann.seed() != HegelTest.NO_SEED) {
            s = s.seed(ann.seed());
        }
        if (ann.reproduceFailure().length > 0) {
            s = s.reproduceFailure(ann.reproduceFailure());
        }
        if (ann.derandomize() != OptBoolean.DEFAULT) {
            s = s.derandomize(ann.derandomize() == OptBoolean.TRUE);
        }
        if (!isDefaultPhases(ann.phases())) {
            s = s.phases(ann.phases());
        }
        // database() is a single tri-state String (members can't be value objects): "" leaves the
        // engine default, DISABLED turns it off, anything else is a directory path.
        if (ann.database().equals(Database.DISABLED)) {
            s = s.database(Database.disabled());
        } else if (!ann.database().isEmpty()) {
            s = s.database(Database.path(ann.database()));
        }
        return s;
    }

    /**
     * The examples the methods {@code ann} names supply, invoked on {@code target} (ignored by a
     * static method).
     */
    static Example[] examplesFrom(HegelTest ann, Class<?> testClass, Object target) {
        List<Example> examples = new ArrayList<>();
        for (String name : ann.examples()) {
            Method source = ReflectionSupport.findMethod(testClass, name)
                    .orElseThrow(() -> new IllegalArgumentException(
                            "@HegelTest(examples): " + testClass.getName() + " has no method " + name + "()"));
            Object supplied = ReflectionSupport.invokeMethod(source, target);
            if (supplied instanceof Example example) {
                examples.add(example);
            } else if (supplied instanceof Iterable<?> iterable) {
                for (Object item : iterable) {
                    examples.add((Example) item);
                }
            } else {
                throw new IllegalArgumentException("@HegelTest(examples): " + testClass.getName() + "." + name
                        + "() must return an Example or an Iterable of them");
            }
        }
        return examples.toArray(new Example[0]);
    }

    /**
     * Whether {@code phases} is the annotation default (every phase), in which case the engine
     * default is left untouched. An explicitly empty list is <em>not</em> the default — it disables
     * all phases.
     */
    static boolean isDefaultPhases(Phase[] phases) {
        return phases.length > 0 && EnumSet.copyOf(Arrays.asList(phases)).size() == Phase.values().length;
    }

    static boolean isTestCaseParam(Class<?> type) {
        return type == TestCase.class;
    }
}

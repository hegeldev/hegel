package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.io.ByteArrayInputStream;
import java.lang.reflect.Method;
import org.junit.jupiter.api.Test;

class MethodLocationTest {
    static final class Fixture {
        static final long BIG = 1234567890123L;
        static final double HALF = 0.123456789;

        static int firstLine() {
            return new Throwable().getStackTrace()[0].getLineNumber();
        }

        static int firstLine(int offset) {
            int line = new Throwable().getStackTrace()[0].getLineNumber();
            return line + offset;
        }
    }

    @Test
    void locatesAMethodAtTheFirstLineOfItsBody() throws Exception {
        Method plain = Fixture.class.getDeclaredMethod("firstLine");
        Method overload = Fixture.class.getDeclaredMethod("firstLine", int.class);
        assertEquals(
                new TestLocation(
                        "dev/hegel/MethodLocationTest.java",
                        Fixture.firstLine(),
                        "dev.hegel.MethodLocationTest$Fixture",
                        "firstLine"),
                MethodLocation.of(plain, plain.getDeclaringClass()));
        assertEquals(Fixture.firstLine(0), MethodLocation.of(overload, overload.getDeclaringClass()).beginLine());
    }

    @Test
    void takesTheFileFromTheClassFile() throws Exception {
        Method method = SecondaryFixture.class.getDeclaredMethod("firstLine");
        assertEquals(
                new TestLocation(
                        "dev/hegel/MethodLocationTest.java",
                        SecondaryFixture.firstLine(),
                        "dev.hegel.SecondaryFixture",
                        "firstLine"),
                MethodLocation.of(method, method.getDeclaringClass()));
    }

    @Test
    void aClassWithoutAClassFileIsLocatedAtLineZero() throws Exception {
        Runnable lambda = () -> {};
        Method run = lambda.getClass().getMethod("run");
        TestLocation location = MethodLocation.of(run, run.getDeclaringClass());
        assertEquals("dev/hegel/MethodLocationTest.java", location.file());
        assertEquals(0, location.beginLine());
    }

    @Test
    void aMethodWithoutLineNumbersIsLocatedAtLineZero() throws Exception {
        Method plain = Fixture.class.getDeclaredMethod("firstLine");
        TestLocation location = MethodLocation.of(
                "dev.hegel.MethodLocationTest$Fixture",
                "absent",
                "()V",
                plain.getDeclaringClass().getResourceAsStream("MethodLocationTest$Fixture.class"));
        assertEquals(
                new TestLocation(
                        "dev/hegel/MethodLocationTest.java", 0, "dev.hegel.MethodLocationTest$Fixture", "absent"),
                location);
    }

    @Test
    void anUnreadableClassFileIsLocatedAtLineZero() {
        TestLocation expected = new TestLocation("a/B.java", 0, "a.B", "f");
        assertEquals(expected, MethodLocation.of("a.B", "f", "()V", new ByteArrayInputStream(new byte[3])));
        byte[] unknownConstant = {(byte) 0xCA, (byte) 0xFE, (byte) 0xBA, (byte) 0xBE, 0, 0, 0, 61, 0, 2, 99};
        assertEquals(expected, MethodLocation.of("a.B", "f", "()V", new ByteArrayInputStream(unknownConstant)));
    }
}

final class SecondaryFixture {
    private SecondaryFixture() {}

    static int firstLine() {
        return new Throwable().getStackTrace()[0].getLineNumber();
    }
}

package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Test;

/**
 * {@link Hegel#main} as a program. Exiting and setting the default profile affects the whole
 * process, so these tests run {@link MainFixture} in a child JVM on this test's classpath.
 */
class MainProcessTest {
    private record Result(int status, String output) {}

    private static Result program(String... args) throws IOException, InterruptedException {
        List<String> command = new ArrayList<>();
        command.add(Path.of(System.getProperty("java.home"), "bin", "java").toString());
        command.add("--enable-native-access=ALL-UNNAMED");
        command.add("-cp");
        command.add(System.getProperty("java.class.path"));
        command.add(MainFixture.class.getName());
        command.addAll(List.of(args));
        ProcessBuilder pb = new ProcessBuilder(command).redirectErrorStream(true);
        pb.environment().keySet().removeIf(k -> k.startsWith("HEGEL_") && !k.equals("HEGEL_LIBHEGEL_PATH"));
        pb.environment().put("HEGEL_DATABASE", "disabled");
        Process child = pb.start();
        String output = new String(child.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
        assertTrue(child.waitFor(2, TimeUnit.MINUTES), "program did not finish: " + output);
        return new Result(child.exitValue(), output);
    }

    @Test
    void theProcessExitsWithTheStatus() throws Exception {
        Result pass = program("pass");
        assertEquals(0, pass.status, pass.output);
        assertEquals(1, pass.output.split("fixture: test case", -1).length - 1, pass.output);

        Result fail = program("fail", "--phases", "generate");
        assertEquals(1, fail.status, fail.output);
        assertTrue(fail.output.contains("rerun with: --failure-blob "), fail.output);

        Result usage = program("pass", "--no-such-setting", "1");
        assertEquals(2, usage.status, usage.output);
    }

    @Test
    void profileSelectsAProfileTheEngineKnows() throws Exception {
        Result shipped = program("pass", "--profile=workload");
        assertEquals(0, shipped.status, shipped.output);

        Result unknown = program("pass", "--profile", "no-such-profile");
        assertEquals(2, unknown.status, unknown.output);
        assertTrue(unknown.output.contains("error: --profile: "), unknown.output);
    }
}

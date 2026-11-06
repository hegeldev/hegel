package dev.hegel.consumer;

import static dev.hegel.Generators.integers;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import dev.hegel.Generators.Temporal;
import dev.hegel.HegelTest;
import dev.hegel.TestCase;
import dev.hegel.generators.temporal.DateGenerator;
import java.time.LocalDate;

class ConsumerTest {
    @HegelTest
    void additionCommutes(TestCase tc) {
        int x = tc.draw(integers().min(0).max(100));
        int y = tc.draw(integers().min(0).max(100));
        assertEquals(x + y, y + x);
    }

    @HegelTest
    void temporalGeneratorsAreExported(TestCase tc) {
        DateGenerator dates = Temporal.dates().min(LocalDate.of(2000, 1, 1));
        assertTrue(tc.draw(dates).getYear() >= 2000);
    }
}

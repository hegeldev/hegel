package dev.hegel;

import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;

import dev.hegel.lowlevel.Libhegel;
import org.junit.jupiter.api.Test;

class EngineTest {
    @Test
    void setResetAndLazyResolve() {
        try {
            FakeLibhegel fake = new FakeLibhegel();
            Engine.setForTesting(fake);
            assertSame(fake, Engine.get());
            Libhegel real = Engine.real();
            assertNotSame(fake, real);
            assertSame(real, Engine.real());

            Engine.reset();
            Libhegel reloaded = Engine.get();
            assertNotSame(real, reloaded);
            assertSame(reloaded, Engine.get());
            assertSame(reloaded, Engine.real());
        } finally {
            Engine.reset();
        }
    }
}

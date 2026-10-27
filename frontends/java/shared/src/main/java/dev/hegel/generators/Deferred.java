package dev.hegel.generators;

import dev.hegel.Generators;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.concurrent.atomic.AtomicReference;

/**
 * A forward reference to a generator whose definition is not yet known, enabling self-recursive and
 * mutually recursive generators.
 *
 * <p>A {@code Deferred<T>} is itself a {@link PrintableGenerator}: pass it into other generators
 * while building a recursive structure, then call {@link #set} once to supply the real
 * implementation, which is printable too. Drawing before {@code set} has been called fails. It
 * prints what the generator it delegates to prints.
 *
 * <pre>{@code
 * // A binary tree of integers: a leaf, or a branch of two subtrees.
 * Deferred<Tree> tree = Generators.deferred();
 * PrintableGenerator<Tree> leaf = integers().map(Leaf::new);
 * PrintableGenerator<Tree> branch = tuples(tree, tree).map(t -> new Branch(t.value1(), t.value2()));
 * tree.set(oneOf(leaf, branch));
 * Tree t = tc.draw(tree);
 * }</pre>
 *
 * <p>Obtain one from {@link Generators#deferred()}.
 *
 * @param <T> the type of value produced
 */
public final class Deferred<T> implements PrintableGenerator<T> {
    private final AtomicReference<PrintableGenerator<T>> delegate = new AtomicReference<>();

    public Deferred() {}

    /**
     * Supply the implementation this reference delegates to. May be called only once.
     *
     * @param generator the printable generator to delegate to
     * @throws IllegalStateException if this deferred generator has already been set
     */
    public void set(PrintableGenerator<T> generator) {
        if (!delegate.compareAndSet(null, generator)) {
            throw new IllegalStateException("deferred generator has already been set");
        }
    }

    /** @hidden */
    @Override
    public long label() {
        return Label.DEFERRED;
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return tc.drawSilent(delegate());
    }

    /** @hidden */
    @Override
    public T doDrawAndPrint(TestCase tc, Printer printer) {
        return tc.drawAndPrint(delegate(), printer);
    }

    private PrintableGenerator<T> delegate() {
        PrintableGenerator<T> g = delegate.get();
        if (g == null) {
            throw new IllegalStateException("deferred generator has not been set");
        }
        return g;
    }
}

package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Generators;
import dev.hegel.Label;
import dev.hegel.PrettyPrinter;
import dev.hegel.PrintableGenerator;
import dev.hegel.TestCase;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;

/**
 * A forward reference to a generator whose definition is not yet known, enabling self-recursive and
 * mutually recursive generators.
 *
 * <p>A {@code Deferred<T>} is itself a {@link Generator}: pass it into other generators while
 * building a recursive structure, then call {@link #set} once to supply the real implementation.
 * Drawing before {@code set} has been called fails. It prints what the generator it delegates to
 * prints, and once set it is labelled as that generator is (a reference that reaches itself while
 * its label is computed is labelled {@link Label#DEFERRED} there, so the label is finite).
 *
 * <pre>{@code
 * // A binary tree of integers: a leaf, or a branch of two subtrees.
 * Deferred<Tree> tree = Generators.deferred();
 * Generator<Tree> leaf = integers().map(Leaf::new);
 * Generator<Tree> branch = tuples(tree, tree).map(t -> new Branch(t.value1(), t.value2()));
 * tree.set(oneOf(leaf, branch));
 * Tree t = tc.draw(tree);
 * }</pre>
 *
 * <p>Obtain one from {@link Generators#deferred()}.
 *
 * @param <T> the type of value produced
 */
public final class Deferred<T> implements PrintableGenerator<T> {
    private static final ThreadLocal<List<Deferred<?>>> LABELLING = ThreadLocal.withInitial(ArrayList::new);

    private final AtomicReference<Generator<T>> delegate = new AtomicReference<>();
    private volatile Long label;

    public Deferred() {}

    /**
     * Supply the implementation this reference delegates to. May be called only once.
     *
     * @param generator the generator to delegate to
     * @throws IllegalStateException if this deferred generator has already been set
     */
    public void set(Generator<T> generator) {
        if (!delegate.compareAndSet(null, generator)) {
            throw new IllegalStateException("deferred generator has already been set");
        }
    }

    /** @hidden */
    @Override
    public long label() {
        Long known = label;
        if (known != null) {
            return known;
        }
        Generator<T> g = delegate.get();
        if (g == null) {
            return Label.DEFERRED;
        }
        List<Deferred<?>> labelling = LABELLING.get();
        if (labelling.contains(this)) {
            return Label.DEFERRED;
        }
        labelling.add(this);
        try {
            long computed = g.label();
            label = computed;
            return computed;
        } finally {
            labelling.remove(labelling.size() - 1);
        }
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return delegate().doDraw(tc);
    }

    /** @hidden */
    @Override
    public T doDrawAndPrint(TestCase tc, PrettyPrinter printer) {
        Generator<T> g = delegate();
        if (g instanceof PrintableGenerator<T> printable) {
            return printable.doDrawAndPrint(tc, printer);
        }
        throw TestCase.unprintable(g);
    }

    private Generator<T> delegate() {
        Generator<T> g = delegate.get();
        if (g == null) {
            throw new IllegalStateException("deferred generator has not been set");
        }
        return g;
    }
}

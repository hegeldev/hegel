package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
import dev.hegel.PrintableGenerator;
import dev.hegel.Printer;
import dev.hegel.TestCase;
import java.util.function.Function;

/**
 * Generates recursively defined data, such as trees or JSON documents, through the engine's
 * recursion protocol: the engine decides where the value branches and where it bottoms out in
 * leaves, steering each value toward a size drawn from across the leaf budget.
 *
 * <p>Values are leaves from the base generator, or branches built by the extend function from
 * generators of further sub-values. Each subtree is drawn in a span of its own with the same
 * label, so the shrinker can replace a tree with one of its subtrees. Narrow the shape with {@link
 * #maxDepth(int)} (default {@value #DEFAULT_MAX_DEPTH}) and {@link #maxLeaves(int)} (default
 * {@value #DEFAULT_MAX_LEAVES}).
 *
 * <p>It prints what its leaves and branches print: the base generator for a leaf, and the
 * generator {@code extend} returns for a branch, whose sub-values print in place. An attempt that
 * is discarded and retried prints nothing.
 *
 * @param <T> the value type
 */
public final class RecursiveGenerator<T> implements PrintableGenerator<T> {
    /** The default maximum nesting depth of branches. */
    public static final int DEFAULT_MAX_DEPTH = 32;

    /** The default maximum number of leaves in one value. */
    public static final int DEFAULT_MAX_LEAVES = 100;

    private final Generator<T> base;
    private final Function<Generator<T>, ? extends Generator<T>> extend;
    private final int maxDepth;
    private final int maxLeaves;
    private final CachedLabel label = new CachedLabel();

    public RecursiveGenerator(
            Generator<T> base, Function<Generator<T>, ? extends Generator<T>> extend, int maxDepth, int maxLeaves) {
        if (maxDepth < 0) {
            throw new IllegalArgumentException("recursive: maxDepth must be >= 0, got " + maxDepth);
        }
        if (maxLeaves < 1) {
            throw new IllegalArgumentException("recursive: maxLeaves must be >= 1, got " + maxLeaves);
        }
        this.base = base;
        this.extend = extend;
        this.maxDepth = maxDepth;
        this.maxLeaves = maxLeaves;
    }

    /**
     * Branches nest at most this deep: sub-values at this depth are always leaves, so 0 generates
     * only leaves.
     *
     * @param maxDepth the maximum nesting depth
     * @return a copy with the maximum depth set
     */
    public RecursiveGenerator<T> maxDepth(int maxDepth) {
        return new RecursiveGenerator<>(base, extend, maxDepth, maxLeaves);
    }

    /**
     * The most leaves one value may contain. An attempt that draws more is discarded and retried
     * steering toward a smaller value; when several attempts in a row fail to fit, the test case is
     * rejected as if by {@link TestCase#assume}.
     *
     * @param maxLeaves the maximum number of leaves
     * @return a copy with the leaf budget set
     */
    public RecursiveGenerator<T> maxLeaves(int maxLeaves) {
        return new RecursiveGenerator<>(base, extend, maxDepth, maxLeaves);
    }

    /** Unwinds an attempt that outgrew the leaf budget back to the attempt loop. */
    private static final class OverBudget extends RuntimeException {
        OverBudget() {
            super(null, null, false, false);
        }
    }

    /** @hidden */
    @Override
    public long label() {
        return label.get(() -> Label.combine(Label.RECURSIVE, base.label()));
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        return draw(tc, null);
    }

    /** @hidden */
    @Override
    public T doDrawAndPrint(TestCase tc, Printer printer) {
        return draw(tc, printer);
    }

    private T draw(TestCase tc, Printer printer) {
        Printer p = Components.out(printer);
        tc.startSpan(label());
        try {
            long recursion = tc.newRecursion(maxDepth, maxLeaves);
            try {
                while (true) {
                    Printer.Speculation speculation = p.speculate();
                    try {
                        T value;
                        try {
                            value = drawSubtree(tc, recursion, 0, printer);
                        } catch (OverBudget e) {
                            tc.recursionRetry(recursion);
                            continue;
                        }
                        if (tc.recursionFinish(recursion)) {
                            speculation.commit();
                            return value;
                        }
                    } finally {
                        speculation.close();
                    }
                }
            } finally {
                tc.recursionFree(recursion);
            }
        } finally {
            tc.stopSpan(false);
        }
    }

    private T drawSubtree(TestCase tc, long recursion, long depth, Printer printer) {
        if (tc.recursionBranch(recursion, depth)) {
            PrintableGenerator<T> subtrees = new PrintableGenerator<>() {
                @Override
                public T doDraw(TestCase inner) {
                    return drawSubtreeInSpan(inner, recursion, depth + 1, null);
                }

                @Override
                public T doDrawAndPrint(TestCase inner, Printer innerPrinter) {
                    return drawSubtreeInSpan(inner, recursion, depth + 1, innerPrinter);
                }
            };
            return Components.draw(tc, extend.apply(subtrees), printer);
        }
        if (!tc.recursionLeaf(recursion)) {
            throw new OverBudget();
        }
        return Components.draw(tc, base, printer);
    }

    private T drawSubtreeInSpan(TestCase tc, long recursion, long depth, Printer printer) {
        tc.startSpan(label());
        try {
            return drawSubtree(tc, recursion, depth, printer);
        } finally {
            tc.stopSpan(false);
        }
    }
}

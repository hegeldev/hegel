package dev.hegel.generators;

import dev.hegel.Generator;
import dev.hegel.Label;
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
 * @param <T> the value type
 */
public final class RecursiveGenerator<T> implements Generator<T> {
    /** The default maximum nesting depth of branches. */
    public static final int DEFAULT_MAX_DEPTH = 32;

    /** The default maximum number of leaves in one value. */
    public static final int DEFAULT_MAX_LEAVES = 100;

    private final Generator<T> base;
    private final Function<Generator<T>, ? extends Generator<T>> extend;
    private final int maxDepth;
    private final int maxLeaves;

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

    /** Unwinds an attempt that outgrew the leaf budget back to {@link #doDraw}. */
    private static final class OverBudget extends RuntimeException {
        OverBudget() {
            super(null, null, false, false);
        }
    }

    /** @hidden */
    @Override
    public T doDraw(TestCase tc) {
        tc.startSpan(Label.RECURSIVE);
        try {
            long recursion = tc.newRecursion(maxDepth, maxLeaves);
            try {
                while (true) {
                    T value;
                    try {
                        value = drawSubtree(tc, recursion, 0);
                    } catch (OverBudget e) {
                        tc.recursionRetry(recursion);
                        continue;
                    }
                    if (tc.recursionFinish(recursion)) {
                        return value;
                    }
                }
            } finally {
                tc.recursionFree(recursion);
            }
        } finally {
            tc.stopSpan(false);
        }
    }

    private T drawSubtree(TestCase tc, long recursion, long depth) {
        if (tc.recursionBranch(recursion, depth)) {
            Generator<T> subtrees = inner -> {
                inner.startSpan(Label.RECURSIVE);
                try {
                    return drawSubtree(inner, recursion, depth + 1);
                } finally {
                    inner.stopSpan(false);
                }
            };
            return extend.apply(subtrees).doDraw(tc);
        }
        if (!tc.recursionLeaf(recursion)) {
            throw new OverBudget();
        }
        return base.doDraw(tc);
    }
}

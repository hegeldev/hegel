import { describe, test, expect } from "vitest";
import * as hegel from "@hegeldev/hegel";
import * as gs from "@hegeldev/hegel/generators";
import type { Generator } from "../src/generators/core.js";

const deterministic = { derandomize: true, database: hegel.Database.disabled };

type Tree = number | Tree[];

function leaves(tree: Tree): number {
  return Array.isArray(tree) ? tree.reduce<number>((n, child) => n + leaves(child), 0) : 1;
}

function depth(tree: Tree): number {
  return Array.isArray(tree) ? 1 + Math.max(0, ...tree.map(depth)) : 0;
}

const trees = (options?: gs.RecursiveOptions): Generator<Tree> =>
  gs.recursive<Tree>(
    gs.integers({ minValue: 0, maxValue: 10 }),
    (children) => gs.arrays(children, { minSize: 1, maxSize: 4 }),
    options,
  );

/** The value the last test case drew, once `failing` has made the test fail. */
function minimal<T>(generator: Generator<T>, failing: (value: T) => boolean): T {
  let last: T | undefined;
  expect(() =>
    hegel.test(
      (tc) => {
        const value = tc.draw(generator);
        if (failing(value)) {
          last = value;
          throw new Error("found");
        }
      },
      { testCases: 1000, ...deterministic },
    ),
  ).toThrow("found");
  return last as T;
}

describe("gs.recursive()", () => {
  test("generates trees within the leaf and depth limits", () => {
    let branched = 0;
    hegel.test(
      (tc) => {
        const tree = tc.draw(trees({ maxDepth: 3, maxLeaves: 8 }));
        expect(leaves(tree)).toBeLessThanOrEqual(8);
        expect(depth(tree)).toBeLessThanOrEqual(3);
        if (Array.isArray(tree)) branched++;
      },
      { testCases: 200, ...deterministic },
    );
    expect(branched).toBeGreaterThan(0);
  });

  test("maxDepth 0 generates only leaves", () =>
    hegel.test(
      (tc) => {
        expect(typeof tc.draw(trees({ maxDepth: 0 }))).toBe("number");
      },
      { testCases: 30, ...deterministic },
    ));

  test("a branch function of one child is repriced, not stuck at single leaves", () => {
    const chains = gs.recursive<Tree>(gs.just(0), (children) => children.map((c) => [c]));
    let longest = 0;
    hegel.test(
      (tc) => {
        longest = Math.max(longest, depth(tc.draw(chains)));
      },
      { testCases: 200, ...deterministic },
    );
    expect(longest).toBeGreaterThan(3);
  });

  test("values shrink toward a single leaf", () => {
    expect(minimal(trees(), () => true)).toBe(0);
    expect(minimal(trees(), (tree) => leaves(tree) >= 2)).toEqual([0, 0]);
  });

  test("a recursive generator nested in another's sub-values leaves its budget alone", () => {
    const inner = (outer: Generator<Tree>) =>
      gs.recursive<Tree>(outer, (children) => gs.arrays(children, { minSize: 2, maxSize: 3 }));
    const nested = gs.recursive<Tree>(gs.just(1), inner, { maxLeaves: 3 });
    hegel.test(
      (tc) => {
        expect(leaves(tc.draw(nested))).toBeGreaterThanOrEqual(1);
      },
      { testCases: 200, ...deterministic },
    );
  });

  test("is drawn inside a composite and in a filter", () =>
    hegel.test(
      (tc) => {
        const pair = tc.draw(
          gs.composite((tc) => [tc.draw(trees({ maxLeaves: 4 })), tc.draw(trees())]),
        );
        expect(pair).toHaveLength(2);
        const tree = tc.draw(trees({ maxLeaves: 4 }).filter((tree) => leaves(tree) <= 4));
        expect(leaves(tree)).toBeLessThanOrEqual(4);
      },
      { testCases: 50, ...deterministic },
    ));

  test("rejects invalid limits", () => {
    expect(() => trees({ maxDepth: -1 })).toThrow("maxDepth must be an integer >= 0, got -1");
    expect(() => trees({ maxLeaves: 0 })).toThrow("maxLeaves must be an integer >= 1, got 0");
    expect(() => trees({ maxLeaves: 1.5 })).toThrow("maxLeaves must be an integer >= 1");
  });

  test("labels a recursive generator by its leaves, and its sub-values the same", () => {
    let children: Generator<Tree> | undefined;
    const tree = gs.recursive<Tree>(gs.integers(), (c) => {
      children = c;
      return gs.arrays(c);
    });
    expect(tree.label.name).toBe("hegel.recursive");
    expect(tree.label.parts).toEqual([gs.integers().label]);
    hegel.test(
      (tc) => {
        tc.draw(tree);
      },
      { testCases: 50, ...deterministic },
    );
    expect(children!.label).toBe(tree.label);
  });
});

describe("gs.deferred()", () => {
  type Node = null | [Node, Node];

  test("defines a generator in terms of itself", () => {
    const tree: Generator<Node> = gs.deferred(() =>
      gs.oneOf<Node>(gs.none(), gs.tuples(tree, tree)),
    );
    let pairs = 0;
    hegel.test(
      (tc) => {
        const node = tc.draw(tree);
        if (node !== null) pairs++;
      },
      { testCases: 100, ...deterministic },
    );
    expect(pairs).toBeGreaterThan(0);
    expect(tree.label.name).toBe("hegel.one_of");
    expect(tree.label).toBe(tree.label);
  });

  test("a definition that always recurses rejects its test cases", () => {
    const endless: Generator<unknown> = gs.deferred(() => gs.tuples(endless));
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(endless);
        },
        { testCases: 10, ...deterministic },
      ),
    ).toThrow(hegel.RunError);
  });

  test("follows a definition that is itself deferred", () => {
    const resolved = gs.deferred(() => gs.integers({ minValue: 0, maxValue: 9 }));
    const viaResolved = gs.deferred(() => resolved);
    const viaUnresolved = gs.deferred(() =>
      gs.deferred(() => gs.integers({ minValue: 10, maxValue: 19 })),
    );
    hegel.test(
      (tc) => {
        expect(tc.draw(resolved)).toBeLessThan(10);
        expect(tc.draw(viaResolved)).toBeLessThan(10);
        expect(tc.draw(viaUnresolved)).toBeGreaterThanOrEqual(10);
      },
      { testCases: 10, ...deterministic },
    );
  });

  test("a definition that resolves only to itself is an error", () => {
    const loop: Generator<number> = gs.deferred(() => loop);
    expect(loop.label.name).toBe("hegel.deferred");
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(loop);
        },
        { testCases: 1, ...deterministic },
      ),
    ).toThrow("Cannot define a deferred generator as itself");
    const a: Generator<number> = gs.deferred(() => b);
    const b: Generator<number> = gs.deferred(() => a);
    expect(() =>
      hegel.test(
        (tc) => {
          tc.draw(a);
        },
        { testCases: 1, ...deterministic },
      ),
    ).toThrow("Cannot define a deferred generator as itself");
  });
});

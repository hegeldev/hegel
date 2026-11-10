import * as gs from "../../src/generators/index.js";
import type { TestCase } from "../../src/testCase.js";

class Evens extends gs.Generator<number> {
  doDraw(tc: TestCase): number {
    return tc.drawSilent(gs.integers()) & ~1;
  }
}

function silentHelper(): gs.Generator<number> {
  return gs.integers();
}

function printableHelper(): gs.PrintableGenerator<number> {
  return gs.integers();
}

type Tree = null | [Tree, Tree];
const tree: gs.PrintableGenerator<Tree> = gs.deferred(() =>
  gs.oneOf<Tree>(gs.just(null), gs.tuples(tree, tree)),
);

export function unprintable(tc: TestCase): void {
  // @ts-expect-error a hand-written generator without a printer cannot be drawn
  tc.draw(new Evens());
  // @ts-expect-error a helper typed as a plain Generator cannot be drawn
  tc.draw(silentHelper());
  // @ts-expect-error an array of unprintable elements cannot print
  tc.draw(gs.arrays(new Evens()));
  // @ts-expect-error a tuple with an unprintable component cannot print
  tc.draw(gs.tuples(gs.integers(), new Evens()));
  // @ts-expect-error a record with an unprintable field cannot print
  tc.draw(gs.record({ a: new Evens() }));
  // @ts-expect-error a flatMap to an unprintable generator cannot print
  tc.draw(gs.integers().flatMap(() => new Evens()));
  // @ts-expect-error a filter keeps its source's printability
  tc.draw(new Evens().filter(() => true));
  // @ts-expect-error printAsCall needs a printable source
  new Evens().map((n) => n).printAsCall("f");
}

export function printable(tc: TestCase): void {
  const pair: [number, string] = tc.draw(gs.tuples(gs.integers(), gs.text()));
  const record: { a: number; b: boolean } = tc.draw(
    gs.record({ a: gs.integers(), b: gs.booleans() }),
  );
  const many: Map<string, Set<number>> = tc.draw(gs.maps(gs.text(), gs.sets(gs.integers())));
  const maybe: number | null = tc.draw(gs.optional(gs.oneOf(gs.integers(), gs.just(1))));
  const json: unknown = tc.draw(
    gs.recursive<unknown>(gs.integers(), (children) => gs.arrays(children)),
  );
  tc.draw(printableHelper());
  tc.draw(tree);
  tc.draw(new Evens().printAsValue());
  tc.draw(new Evens().printWith((n, p) => p.text(String(n))));
  tc.draw(gs.integers().map((n) => String(n)));
  tc.draw(
    gs
      .integers()
      .map((n) => String(n))
      .printAsCall("String"),
  );
  tc.draw(gs.integers().filter((n) => n > 0));
  tc.draw(gs.booleans().flatMap((b) => (b ? gs.integers() : gs.just(0))));
  tc.draw(gs.composite((inner) => inner.draw(gs.integers())));
  tc.drawSilent(new Evens());
  tc.drawSilent(gs.arrays(new Evens()));
  void [pair, record, many, maybe, json];
}

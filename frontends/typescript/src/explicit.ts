/**
 * Explicit examples: a test body run with values given in advance, before
 * the engine generates any.
 *
 * @packageDocumentation
 */

import { TextPrinter } from "./pretty.js";
import type { DataSource } from "./testCase.js";

function unsupported(operation: string): never {
  throw new Error(`${operation} is not supported in explicit examples`);
}

/**
 * The {@link DataSource} of an explicit example: draws are served from the
 * example's values by `TestCase.draw` itself, so anything that would draw
 * from the engine is an error. Its notes and printed draws are kept as plain
 * lines for the report.
 */
export class ExplicitDataSource implements DataSource {
  readonly family = {};
  private readonly output = new TextPrinter();

  generate(): unknown {
    return unsupported("Drawing from a generator's engine primitives");
  }
  label(): bigint {
    return unsupported("Labelling a span");
  }
  startSpan(): void {
    unsupported("Opening a span");
  }
  stopSpan(): void {
    unsupported("Closing a span");
  }
  newCollection(): number {
    return unsupported("A collection");
  }
  collectionMore(): boolean {
    return unsupported("A collection");
  }
  collectionReject(): void {
    unsupported("A collection");
  }
  markComplete(): void {}
  newPool(): number {
    return unsupported("A stateful.Pool");
  }
  poolAdd(): bigint {
    return unsupported("A stateful.Pool");
  }
  poolGenerate(): bigint {
    return unsupported("A stateful.Pool");
  }
  newStateMachine(): number {
    return unsupported("A state machine");
  }
  stateMachineNextRound(): boolean {
    return unsupported("A state machine");
  }
  stateMachineNextRule(): number | null {
    return unsupported("A state machine");
  }
  stateMachineRuleRejected(): void {
    unsupported("A state machine");
  }
  stateMachineShouldCheckInvariant(): boolean {
    return unsupported("A state machine");
  }
  note(message: string): void {
    this.output.note(message);
  }
  target(): void {}
  event(): void {}
  eventValue(): void {}
  clone(): DataSource {
    return this;
  }
  block(): DataSource {
    return this;
  }
  setWorker(): void {}
  printer(): TextPrinter {
    return this.output;
  }
  /** The example's printed draws and notes. */
  lines(): string[] {
    return this.output.lines();
  }
}

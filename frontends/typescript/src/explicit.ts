/**
 * Explicit examples: a test body run with values given in advance, before
 * the engine generates any.
 *
 * @packageDocumentation
 */

import type { EngineDocument, Printer } from "./pretty.js";
import { ExplicitExampleError, type DataSource } from "./testCase.js";

function unsupported(operation: string): never {
  throw new ExplicitExampleError(`${operation} is not supported in explicit examples`);
}

/**
 * The {@link DataSource} of an explicit example: draws are served from the
 * example's values by `TestCase.draw` itself, so anything that would draw
 * from the engine is a usage error. Its notes and printed draws go into an
 * engine document, laid out as a generated test case's are.
 */
export class ExplicitDataSource implements DataSource {
  constructor(
    private readonly document: EngineDocument,
    private readonly indent = 0,
    readonly family: object = {},
  ) {}

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
  poolRestore(): void {
    unsupported("A stateful.Pool");
  }
  newRecursion(): number {
    return unsupported("A recursive generator");
  }
  recursionBranch(): boolean {
    return unsupported("A recursive generator");
  }
  recursionLeaf(): boolean {
    return unsupported("A recursive generator");
  }
  recursionRetry(): void {
    unsupported("A recursive generator");
  }
  recursionFinish(): boolean {
    return unsupported("A recursive generator");
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
  drawGrammar(): string {
    return unsupported("A grammar");
  }
  note(message: string): void {
    this.document.note(this.indent, message);
  }
  target(): void {}
  event(): void {}
  eventValue(): void {}
  clone(): DataSource {
    return this;
  }
  block(indent: number): DataSource {
    return new ExplicitDataSource(this.document, this.indent + indent, this.family);
  }
  setWorker(): void {}
  printer(): Printer {
    return this.document.view(this.indent);
  }
}

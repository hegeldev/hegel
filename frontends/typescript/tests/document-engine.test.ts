import { expect, it } from "vitest";
import { Document } from "../src/pretty.js";

it("needs an engine to lay a document out", () => {
  expect(() => new Document()).toThrow(
    "No Hegel engine is loaded: import @hegeldev/hegel before creating a Document",
  );
});

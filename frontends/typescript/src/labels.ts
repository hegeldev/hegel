/**
 * Span labels: the engine-derived identities of the generators that open
 * shrinking spans.
 *
 * A generator describes its label as a {@link LabelSpec}: a name of the form
 * `hegel.<kind>` plus the specs of the generators it is built from, so that
 * `arrays(integers())` and `arrays(text())` get different labels while every
 * `arrays(integers())` gets the same one. The label itself is computed by the
 * engine (`hegel_label_from_name` / `hegel_label_combine`) the first time a
 * spec is used with an engine, and cached per engine.
 *
 * @packageDocumentation
 */

import type { ContextHandle, Engine } from "./engine.js";

/** A generator's label, before the engine hashes it. */
export interface LabelSpec {
  readonly name: string;
  readonly parts: readonly LabelSpec[];
}

/** The spec for a generator named `name`, built from `parts`. */
export function labelSpec(name: string, ...parts: LabelSpec[]): LabelSpec {
  return { name, parts };
}

/** The names of the labels the standard generators use. */
export const LabelNames = {
  LISTS: "hegel.lists",
  SETS: "hegel.sets",
  DICTS: "hegel.dicts",
  DICT_ENTRY: "hegel.dicts.entry",
  TUPLES: "hegel.tuples",
  ONE_OF: "hegel.one_of",
  OPTIONAL: "hegel.optional",
  FIXED_DICTS: "hegel.fixed_dicts",
  MAP: "hegel.map",
  FLAT_MAP: "hegel.flat_map",
  FILTER: "hegel.filter",
  COMPOSITE: "hegel.composite",
  GENERATOR: "hegel.generator",
  STATEFUL_RULE: "hegel.stateful.rule",
} as const;

/** The spec of one round of a stateful test: the rule selection plus the rule's draws. */
export const STATEFUL_RULE = labelSpec(LabelNames.STATEFUL_RULE);

const cache = new WeakMap<Engine, WeakMap<LabelSpec, bigint>>();

/** The engine's label for `spec`. */
export function resolveLabel(lib: Engine, ctx: ContextHandle, spec: LabelSpec): bigint {
  let labels = cache.get(lib);
  if (labels === undefined) {
    labels = new WeakMap();
    cache.set(lib, labels);
  }
  let label = labels.get(spec);
  if (label === undefined) {
    const own = lib.labelFromName(ctx, spec.name);
    label =
      spec.parts.length === 0
        ? own
        : lib.labelCombine(ctx, [own, ...spec.parts.map((part) => resolveLabel(lib, ctx, part))]);
    labels.set(spec, label);
  }
  return label;
}

const schemaSpecs = new WeakMap<object, LabelSpec>();
const entrySpecs = new WeakMap<object, LabelSpec>();

const SCHEMA_NAMES: Record<string, string> = {
  list: LabelNames.LISTS,
  dict: LabelNames.DICTS,
  tuple: LabelNames.TUPLES,
  one_of: LabelNames.ONE_OF,
};

/** The spec of the generator a schema describes. */
export function schemaLabelSpec(schema: Record<string, unknown>): LabelSpec {
  let spec = schemaSpecs.get(schema);
  if (spec === undefined) {
    const type = String(schema["type"]);
    const name = SCHEMA_NAMES[type] ?? `hegel.${type}`;
    const children: Record<string, unknown>[] =
      type === "list"
        ? [schema["elements"] as Record<string, unknown>]
        : type === "dict"
          ? [schema["keys"] as Record<string, unknown>, schema["values"] as Record<string, unknown>]
          : type === "tuple"
            ? (schema["elements"] as Record<string, unknown>[])
            : type === "one_of"
              ? (schema["generators"] as Record<string, unknown>[])
              : [];
    spec = labelSpec(name, ...children.map(schemaLabelSpec));
    schemaSpecs.set(schema, spec);
  }
  return spec;
}

/** The spec of one entry of a dict schema: its key and value drawn together. */
export function dictEntryLabelSpec(schema: Record<string, unknown>): LabelSpec {
  let spec = entrySpecs.get(schema);
  if (spec === undefined) {
    spec = labelSpec(LabelNames.DICT_ENTRY, ...schemaLabelSpec(schema).parts);
    entrySpecs.set(schema, spec);
  }
  return spec;
}

/**
 * Structural equality keys for `unique` and `uniqueBy` arrays.
 *
 * @packageDocumentation
 */

/**
 * Gives each value a string key, equal for two values exactly when they are
 * structurally equal: primitives as by `Set` (`0` and `-0` alike, `NaN` equal
 * to itself), arrays, plain objects, `Map`s, `Set`s, `Date`s and typed arrays
 * by their contents (the order of an object's properties, a map's entries
 * and a set's members does not matter), and any other object or function by
 * identity. Identities are numbered per instance, so keys from one instance
 * compare only with each other.
 *
 * @internal
 */
export class StructuralKeys {
  private readonly identities = new Map<unknown, number>();
  private readonly open = new Set<object>();

  key(value: unknown): string {
    switch (typeof value) {
      case "string":
        return `s${JSON.stringify(value)}`;
      case "number":
        return Object.is(value, -0) ? "n0" : `n${value}`;
      case "bigint":
        return `b${value}`;
      case "boolean":
        return `${value}`;
      case "undefined":
        return "u";
      case "object":
        return value === null ? "null" : this.objectKey(value);
      default:
        return this.identity(value);
    }
  }

  private identity(value: unknown): string {
    let id = this.identities.get(value);
    if (id === undefined) {
      id = this.identities.size;
      this.identities.set(value, id);
    }
    return `#${id}`;
  }

  private objectKey(value: object): string {
    if (this.open.has(value)) return this.identity(value);
    this.open.add(value);
    try {
      return this.contentsKey(value) ?? this.identity(value);
    } finally {
      this.open.delete(value);
    }
  }

  private contentsKey(value: object): string | null {
    if (Array.isArray(value)) return `[${value.map((item) => this.key(item)).join(",")}]`;
    if (value instanceof Map) {
      const entries = [...value].map(([k, v]) => `${this.key(k)}:${this.key(v)}`);
      return `Map{${entries.sort().join(",")}}`;
    }
    if (value instanceof Set) {
      return `Set{${[...value]
        .map((item) => this.key(item))
        .sort()
        .join(",")}}`;
    }
    if (value instanceof Date) return `Date(${value.getTime()})`;
    if (ArrayBuffer.isView(value) && !(value instanceof DataView)) {
      const items = Array.from(value as unknown as ArrayLike<number | bigint>, String);
      return `${value.constructor.name}(${items.join(",")})`;
    }
    const prototype: unknown = Object.getPrototypeOf(value);
    if (prototype !== Object.prototype && prototype !== null) return null;
    const entries = Object.keys(value).map(
      (k) => `${JSON.stringify(k)}:${this.key((value as Record<string, unknown>)[k])}`,
    );
    return `{${entries.sort().join(",")}}`;
  }
}

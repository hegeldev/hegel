/** A local time type of a {@link tzif} file. */
export interface TzifType {
  utoff: number;
  isDst: boolean;
  abbreviation: string;
}

/**
 * The bytes of a version 2 TZif file (RFC 9636) with `types`, `transitions`
 * (Unix seconds and an index into `types`) and the POSIX TZ string `footer`.
 */
export function tzif(
  types: readonly TzifType[],
  transitions: readonly (readonly [number, number])[],
  footer: string,
): Uint8Array {
  const encoder = new TextEncoder();
  const chars: number[] = [];
  const indices = types.map((type) => {
    const index = chars.length;
    chars.push(...encoder.encode(type.abbreviation), 0);
    return index;
  });
  const bytes: number[] = [];
  const u32 = (value: number) => {
    const view = new DataView(new ArrayBuffer(4));
    view.setInt32(0, value);
    bytes.push(...new Uint8Array(view.buffer));
  };
  const i64 = (value: number) => {
    const view = new DataView(new ArrayBuffer(8));
    view.setBigInt64(0, BigInt(value));
    bytes.push(...new Uint8Array(view.buffer));
  };
  for (const wide of [false, true]) {
    bytes.push(...encoder.encode("TZif2"), ...new Array<number>(15).fill(0));
    for (const count of [0, 0, 0, transitions.length, types.length, chars.length]) u32(count);
    for (const [instant] of transitions) (wide ? i64 : u32)(instant);
    for (const [, type] of transitions) bytes.push(type);
    types.forEach((type, index) => {
      u32(type.utoff);
      bytes.push(type.isDst ? 1 : 0, indices[index]);
    });
    bytes.push(...chars);
  }
  bytes.push(...encoder.encode(`\n${footer}\n`));
  return Uint8Array.from(bytes);
}

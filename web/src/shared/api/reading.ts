export type Read<T> = (value: unknown) => { got: T } | null;

class NotWhatWeExpected extends Error {}

export function shape<T>(build: (given: Record<string, unknown>) => T): Read<T> {
  return (value) => {
    const given = fields(value);
    if (!given) return null;
    try {
      return { got: build(given) };
    } catch (trouble) {
      if (trouble instanceof NotWhatWeExpected) return null;
      throw trouble;
    }
  };
}

export function need<T>(read: Read<T>, value: unknown): T {
  const got = read(value);
  if (!got) throw new NotWhatWeExpected();
  return got.got;
}

function fields(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return null;
  return Object.fromEntries(Object.entries(value));
}

export const anything: Read<unknown> = (value) => ({ got: value });

export const text: Read<string> = (value) => (typeof value === "string" ? { got: value } : null);

export const whole: Read<number> = (value) =>
  typeof value === "number" && Number.isFinite(value) ? { got: value } : null;

export const yesNo: Read<boolean> = (value) => (typeof value === "boolean" ? { got: value } : null);

export function maybe<T>(read: Read<T>): Read<T | null> {
  return (value) => (value === null || value === undefined ? { got: null } : read(value));
}

export function list<T>(read: Read<T>): Read<T[]> {
  return (value) => {
    if (!Array.isArray(value)) return null;
    const got: T[] = [];
    for (const one of value) {
      const read_one = read(one);
      if (!read_one) return null;
      got.push(read_one.got);
    }
    return { got };
  };
}

export function oneOf<T extends string>(allowed: readonly T[]): Read<T> {
  return (value) => {
    const found = allowed.find((one) => one === value);
    return found === undefined ? null : { got: found };
  };
}

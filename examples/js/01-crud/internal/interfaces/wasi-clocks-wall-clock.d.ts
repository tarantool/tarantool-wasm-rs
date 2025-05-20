declare module 'wasi:clocks/wall-clock@0.2.0' {
  export function now(): Datetime;
  export function resolution(): Datetime;
  export interface Datetime {
    seconds: bigint,
    nanoseconds: number,
  }
}

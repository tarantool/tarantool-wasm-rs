export type Value = null|boolean|number|bigint|string|Uint8Array|Extension|UUID|
    Decimal|Datetime|Interval|ErrorValue|Value[]|{[key: string]: Value}|
    Map<Value, Value>;
export class Extension {
  constructor(type: number, data: Uint8Array);
  readonly type: number;
  readonly data: Uint8Array;
}
export class UUID {
  constructor(value: string|Uint8Array);
  readonly bytes: Uint8Array;
  toString(): string;
}
export class Decimal {
  constructor(value: string);
  readonly negative: boolean;
  readonly digits: string;
  readonly scale: number;
  toString(): string;
}
export class Datetime {
  constructor(
      seconds: bigint|number,
      options?: {nanoseconds?: number; tzoffset?: number; tzindex?: number});
  readonly seconds: bigint;
  readonly nanoseconds: number;
  readonly tzoffset: number;
  readonly tzindex: number;
}
export const IntervalAdjust:
    {readonly excess: 0; readonly limit: 1; readonly snap: 2};
export interface IntervalFields {
  years?: number|bigint;
  months?: number|bigint;
  weeks?: number|bigint;
  days?: number|bigint;
  hours?: number|bigint;
  minutes?: number|bigint;
  seconds?: number|bigint;
  nanoseconds?: number|bigint;
  adjust?: 0|1|2|0n|1n|2n;
}
export class Interval {
  constructor(fields?: IntervalFields);
  readonly years: number;
  readonly months: number;
  readonly weeks: number;
  readonly days: number;
  readonly hours: number;
  readonly minutes: number;
  readonly seconds: number;
  readonly nanoseconds: number;
  readonly adjust: 0|1|2;
}
export interface ErrorFrameFields {
  code: number;
  message: string;
  errorType?: string;
  file?: string;
  line?: number;
  errno?: number;
  fields?: {[key: string]: Value};
  extra?: Map<number|bigint, Value>;
}
export class ErrorFrame {
  constructor(frame: ErrorFrameFields);
  readonly code: number;
  readonly message: string;
  readonly errorType: string;
  readonly file: string;
  readonly line: number;
  readonly errno: number;
  readonly fields: {[key: string]: Value};
  readonly extra: Map<number|bigint, Value>;
}
export class ErrorValue {
  constructor(stack: ErrorFrame[], options?: {
    extra?: Map<number|bigint, Value>,
  });
  readonly stack: ErrorFrame[];
  readonly extra: Map<number|bigint, Value>;
}
export function pack(value: Value): Uint8Array;
export function unpack(
    data: Uint8Array, options?: {rawExtensions?: boolean}): Value;

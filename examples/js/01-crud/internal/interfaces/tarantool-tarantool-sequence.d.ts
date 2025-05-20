/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/sequence@0.1.0' {
  export function next(seq: Sequence): bigint;
  export function current(seq: Sequence): bigint;
  export function set(seq: Sequence, value: bigint): void;
  export function reset(seq: Sequence): void;
  export type Sequence = import('tarantool:tarantool/types@0.1.0').Sequence;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
}

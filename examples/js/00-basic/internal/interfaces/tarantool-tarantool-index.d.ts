/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/index@0.1.0' {
  export function len(index: Index): bigint;
  export function bsize(index: Index): bigint;
  export function random(index: Index, rnd: number): BoxTuple;
  export function get(index: Index, key: Uint8Array): BoxTuple;
  export function min(index: Index, key: Uint8Array): BoxTuple;
  export function max(index: Index, key: Uint8Array): BoxTuple;
  export function count(index: Index, iterType: IteratorType, key: Uint8Array): bigint;
  export type Index = import('tarantool:tarantool/types@0.1.0').Index;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
  export type BoxTuple = import('tarantool:tarantool/types@0.1.0').BoxTuple;
  export type IteratorType = import('tarantool:tarantool/types@0.1.0').IteratorType;
}

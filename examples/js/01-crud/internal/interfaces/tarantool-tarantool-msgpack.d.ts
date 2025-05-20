/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/msgpack@0.1.0' {
  export function encode(data: Uint8Array): Uint8Array;
  export function decode(data: Uint8Array): Uint8Array;
  export function decodeFromRawPtr(ptr: bigint): Uint8Array;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
}

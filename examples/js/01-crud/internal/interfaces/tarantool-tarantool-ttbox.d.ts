/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/ttbox@0.1.0' {
  export function schemaVersion(): bigint;
  export function spaceByName(name: string): Space;
  export function indexByName(space: Space, name: string): Index;
  export function insert(space: Space, tup: Uint8Array): BoxTuple;
  export function update(index: Index, key: Uint8Array, ops: Uint8Array): BoxTuple;
  export function replace(space: Space, tup: Uint8Array): BoxTuple;
  export function upsert(index: Index, tup: Uint8Array, ops: Uint8Array): void;
  export { _delete as delete };
  declare function _delete(index: Index, key: Uint8Array): BoxTuple;
  export function truncate(space: Space): void;
  export type Space = import('tarantool:tarantool/types@0.1.0').Space;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
  export type Index = import('tarantool:tarantool/types@0.1.0').Index;
  export type BoxTuple = import('tarantool:tarantool/types@0.1.0').BoxTuple;
}

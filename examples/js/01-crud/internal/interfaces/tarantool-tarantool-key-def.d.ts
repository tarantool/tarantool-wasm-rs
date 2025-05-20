/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/key-def@0.1.0' {
  export { _new as new };
  declare function _new(parts: Array<KeyPart>): KeyDef;
  export function dup(keyDef: KeyDef): KeyDef;
  export function merge(left: KeyDef, right: KeyDef): KeyDef;
  export { _delete as delete };
  declare function _delete(keyDef: KeyDef): void;
  export function dumpParts(keyDef: KeyDef): Array<KeyPart>;
  export function validateKey(keyDef: KeyDef, key: Uint8Array): [boolean, number];
  export function validateFullKey(keyDef: KeyDef, key: Uint8Array): [boolean, number];
  export function partCount(keyDef: KeyDef): number;
  export function extractKey(keyDef: KeyDef, tuple: BoxTuple): BoxTuple;
  export function validateTuple(keyDef: KeyDef, tuple: BoxTuple): boolean;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
  export type BoxTuple = import('tarantool:tarantool/types@0.1.0').BoxTuple;
  export type KeyDef = import('tarantool:tarantool/types@0.1.0').KeyDef;
  export type KeyPart = import('tarantool:tarantool/types@0.1.0').KeyPart;
}

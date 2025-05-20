/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/tuple@0.1.0' {
  export { _new as new };
  declare function _new(data: Uint8Array): BoxTuple;
  export function ref(t: BoxTuple): number;
  export function unref(t: BoxTuple): void;
  export function fieldCount(t: BoxTuple): number;
  export function bsize(t: BoxTuple): bigint;
  export function toBuf(t: BoxTuple): Uint8Array;
  export function format(t: BoxTuple): TupleFormat;
  export function field(t: BoxTuple, idx: number): TupleField;
  export function fieldByPath(t: BoxTuple, path: string, indexBase: number): TupleField;
  export function update(t: BoxTuple, expr: Uint8Array): BoxTuple;
  export function upsert(t: BoxTuple, expr: Uint8Array): BoxTuple;
  export function validate(t: BoxTuple, format: TupleFormat): boolean;
  export function compare(keyDef: KeyDef, left: BoxTuple, right: BoxTuple): number;
  export function compareWithKey(tuple: BoxTuple, key: Uint8Array, keyDef: KeyDef): number;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
  export type BoxTuple = import('tarantool:tarantool/types@0.1.0').BoxTuple;
  export type TupleField = import('tarantool:tarantool/types@0.1.0').TupleField;
  export type TupleFormat = import('tarantool:tarantool/types@0.1.0').TupleFormat;
  export type KeyDef = import('tarantool:tarantool/types@0.1.0').KeyDef;
  
  export class TupleIterator {
    /**
     * This type does not have a public constructor.
     */
    private constructor();
    static newTupleIterator(tuple: BoxTuple): TupleIterator;
    position(): number;
    rewind(): void;
    seek(pos: number): TupleField;
    next(): TupleField;
  }
}

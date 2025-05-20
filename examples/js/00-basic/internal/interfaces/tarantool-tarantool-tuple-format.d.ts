/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/tuple-format@0.1.0' {
  export { _default as default };
  declare function _default(): TupleFormat;
  export { _new as new };
  declare function _new(keyDefs: BigUint64Array): TupleFormat;
  export function ref(tf: TupleFormat): void;
  export function unref(tf: TupleFormat): void;
  export type KeyDef = import('tarantool:tarantool/types@0.1.0').KeyDef;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
  export type TupleFormat = import('tarantool:tarantool/types@0.1.0').TupleFormat;
}

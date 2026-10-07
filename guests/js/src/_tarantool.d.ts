import type * as T from './wit-types.js';
export type * from './wit-types.js';
export namespace metrics {
  export function uptime(): number;
}
export namespace boxTuple {
  function new_(data: T.MsgpackValue): T.BoxTuple;
  export function retain(t: T.BoxTuple): void;
  export function release(t: T.BoxTuple): void;
  export function fieldCount(t: T.BoxTuple): number;
  export function byteSize(t: T.BoxTuple): bigint;
  export function toBytes(t: T.BoxTuple): T.MsgpackValue;
  export function format(t: T.BoxTuple): T.TupleFormat;
  export function field(t: T.BoxTuple, idx: number): T.TupleField|undefined;
  export function fieldByPath(t: T.BoxTuple, path: string): T.TupleField
      |undefined;
  export function update(t: T.BoxTuple, expr: T.MsgpackValue): T.BoxTuple;
  export function upsert(t: T.BoxTuple, expr: T.MsgpackValue): T.BoxTuple;
  export function validate(t: T.BoxTuple, format: T.TupleFormat): void;
  export function compare(
      keyDef: T.KeyDef, left: T.BoxTuple, right: T.BoxTuple): number;
  export function compareWithKey(
      tuple: T.BoxTuple, key: T.MsgpackValue, keyDef: T.KeyDef): number;
  export class Iterator {
    static new(tuple: T.BoxTuple): Iterator;
    position(): number;
    rewind(): void;
    seek(pos: number): T.TupleField|undefined;
    next(): T.TupleField|undefined;
    [Symbol.dispose](): void;
  }
  export {new_ as new};
}
export namespace database {
  export function schemaVersion(): number;
  export function spaceByName(name: string): T.Space|undefined;
  export function indexByName(space: T.Space, name: string): T.Index|undefined;
  export function insert(space: T.Space, tup: T.MsgpackValue): T.BoxTuple;
  export function update(
      index: T.Index, key: T.MsgpackValue, ops: T.MsgpackValue): T.BoxTuple
      |undefined;
  export function replace(space: T.Space, tup: T.MsgpackValue): T.BoxTuple;
  export function upsert(
      index: T.Index, tup: T.MsgpackValue, ops: T.MsgpackValue): void;
  function delete_(index: T.Index, key: T.MsgpackValue): T.BoxTuple|undefined;
  export {delete_ as delete};
  export function truncate(space: T.Space): void;
}
export namespace error {
  function new_(code: T.ErrorCode, message: string): T.BoxError;
  export function newWithLocation(
      code: T.ErrorCode, message: string, file: string|undefined,
      line: number|undefined): T.BoxError;
  export function set(err: T.BoxError): void;
  export function last(): T.BoxError|undefined;
  export function clear(): void;
  export function toString(err: T.BoxError): string;
  export {new_ as new};
}
export namespace index {
  export function tupleCount(index: T.Index): bigint;
  export function memorySize(index: T.Index): bigint;
  export function random(index: T.Index, rnd: number): T.BoxTuple|undefined;
  export function get(index: T.Index, key: T.MsgpackValue): T.BoxTuple
      |undefined;
  export function min(index: T.Index, key: T.MsgpackValue): T.BoxTuple
      |undefined;
  export function max(index: T.Index, key: T.MsgpackValue): T.BoxTuple
      |undefined;
  export function count(
      index: T.Index, mode: T.IteratorType, key: T.MsgpackValue): bigint;
  export class Iterator {
    static new(index: T.Index, mode: T.IteratorType, key: T.MsgpackValue):
        Iterator;
    next(): T.BoxTuple|undefined;
    [Symbol.dispose](): void;
  }
}
export namespace keyDef {
  function new_(parts: (T.KeyPart)[]): T.KeyDef;
  export function duplicate(keyDef: T.KeyDef): T.KeyDef;
  export function merge(left: T.KeyDef, right: T.KeyDef): T.KeyDef;
  export function release(keyDef: T.KeyDef): void;
  export function dumpParts(keyDef: T.KeyDef): (T.KeyPart)[];
  export function validateKey(keyDef: T.KeyDef, key: T.MsgpackValue): void;
  export function validateFullKey(keyDef: T.KeyDef, key: T.MsgpackValue): void;
  export function partCount(keyDef: T.KeyDef): number;
  export function extractKey(keyDef: T.KeyDef, tuple: T.BoxTuple):
      T.MsgpackValue;
  export function validateTuple(keyDef: T.KeyDef, tuple: T.BoxTuple): void;
  export {new_ as new};
}
export namespace log {
  export function write(
      level: T.LogLevel, message: string,
      context: T.LogContext|undefined): void;
}
export namespace msgpack {
  export function fromJson(json: string): T.MsgpackValue;
  export function toJson(value: T.MsgpackValue): string;
}
export namespace sequence {
  export function next(seq: T.Sequence): bigint;
  export function current(seq: T.Sequence): bigint;
  export function set(seq: T.Sequence, value: bigint): void;
  export function reset(seq: T.Sequence): void;
}
export namespace session {
  export function current(): T.Session;
  export function broadcast(key: string, value: string): void;
  export function iprotoSend(
      session: T.Session, header: T.MsgpackValue,
      body: T.MsgpackValue|undefined): void;
}
export namespace transaction {
  export function id(): bigint|undefined;
  export function isolation(): T.TransactionIsolation|undefined;
  export function begin(): void;
  export function commit(): void;
  export function rollback(): void;
  export function setIsolation(isolation: T.TransactionIsolation): void;
  export function makeSynchronous(): void;
}
export namespace tupleFormat {
  function default_(): T.TupleFormat;
  function new_(keyDefs: (T.KeyDef)[]): T.TupleFormat;
  export function retain(tf: T.TupleFormat): void;
  export function release(tf: T.TupleFormat): void;
  export {new_ as new};
  export {default_ as default};
}

import type * as Raw from './_tarantool.js';
import type {Value} from './codec.js';
import type {IteratorType, TransactionIsolation, LogLevel, FieldType, Collation} from './types.js';
export * from './codec.js';
export * from './types.js';
export interface BoxErrorRecord {
  code: number;
  message: string;
  errorType?: string;
  file?: string|null;
  line?: number|null;
}
export class TarantoolError extends Error {
  constructor(value: BoxErrorRecord, options?: ErrorOptions);
  readonly code: number;
  readonly errorType: string;
  readonly file: string|null;
  readonly line: number|null;
}
export class LuaCallbackError extends Error {
  constructor(message: string, options?: ErrorOptions);
}
export class ClosedResourceError extends Error {}
export interface OwnedResource extends Disposable {
  readonly closed: boolean;
  close(): void;
}
export function usingResource<R extends OwnedResource, T>(
    resource: R,
    fn: (resource: R) => T extends PromiseLike<unknown>? never : T): T;
export type Bindings = Pick<
    typeof Raw,
    'database'|'boxTuple'|'tupleFormat'|'keyDef'|'index'|'transaction'|
    'sequence'|'session'|'error'|'log'|'metrics'|'msgpack'>;
export interface KeyPart {
  fieldNo: number;
  fieldType: FieldType;
  collation?: Collation|null;
  path?: string|null;
  flags?: {isNullable?: boolean; excludeNull?: boolean};
}
export interface ScanOptions {
  mode?: IteratorType;
  limit?: number|bigint;
  offset?: number|bigint;
}
export type Operations = [string, number | string, ...Value[]][];
export class IndexIterator implements IterableIterator<Value[]>, OwnedResource {
  private constructor();
  readonly closed: boolean;
  close(): void;
  [Symbol.dispose](): void;
  next(): IteratorResult<Value[]>;
  return(): IteratorResult<Value[]>;
  throw(error: unknown): IteratorResult<Value[]>;
  [Symbol.iterator](): this;
}
export class Space {
  private constructor();
  readonly id: number;
  index(nameOrId?: string|number): Index;
  insert(value: Value[]): Value[];
  replace(value: Value[]): Value[];
  truncate(): void;
  get(key: Value): Value[]|null;
  update(key: Value, operations: Operations): Value[]|null;
  upsert(value: Value[], operations: Operations): void;
  delete(key: Value): Value[]|null;
  select(key?: Value, options?: ScanOptions): Value[][];
  iter(key?: Value, options?: Pick<ScanOptions, 'mode'>): IndexIterator;
}
export class Index {
  private constructor();
  readonly space: Space;
  readonly id: number;
  readonly tupleCount: bigint;
  readonly memorySize: bigint;
  get(key: Value): Value[]|null;
  min(key?: Value): Value[]|null;
  max(key?: Value): Value[]|null;
  random(seed: number): Value[]|null;
  count(key?: Value, options?: {mode?: IteratorType}): bigint;
  select(key?: Value, options?: ScanOptions): Value[][];
  iter(key?: Value, options?: Pick<ScanOptions, 'mode'>): IndexIterator;
  update(key: Value, operations: Operations): Value[]|null;
  upsert(value: Value[], operations: Operations): void;
  delete(key: Value): Value[]|null;
}
export class Tuple implements Iterable<Value>, OwnedResource {
  private constructor();
  readonly closed: boolean;
  close(): void;
  [Symbol.dispose](): void;
  copy(): Tuple;
  readonly length: number;
  readonly byteSize: bigint;
  readonly format: TupleFormat;
  toBytes(): Uint8Array;
  toArray(): Value[];
  field(indexOrPath: number|string): Value;
  [Symbol.iterator](): IterableIterator<Value>;
  iterFields(): TupleCursor;
  update(operations: Operations): Tuple;
  upsert(operations: Operations): Tuple;
  validate(format: TupleFormat): void;
  compare(other: Tuple, definition: KeyDef): number;
  compareWithKey(key: Value, definition: KeyDef): number;
}
export class TupleCursor implements IterableIterator<Value>, OwnedResource {
  private constructor();
  readonly closed: boolean;
  close(): void;
  [Symbol.dispose](): void;
  next(): IteratorResult<Value>;
  read(): IteratorResult<Value>;
  return(): IteratorResult<Value>;
  throw(error: unknown): IteratorResult<Value>;
  [Symbol.iterator](): this;
  readonly position: number;
  rewind(): void;
  seek(position: number): Value;
}
export class TupleFormat implements OwnedResource {
  private constructor();
  readonly closed: boolean;
  close(): void;
  [Symbol.dispose](): void;
  copy(): TupleFormat;
}
export class KeyDef implements OwnedResource {
  private constructor();
  readonly closed: boolean;
  close(): void;
  [Symbol.dispose](): void;
  copy(): KeyDef;
  merge(other: KeyDef): KeyDef;
  readonly partCount: number;
  readonly parts: Raw.KeyPart[];
  validateKey(key: Value, options?: {full?: boolean}): void;
  extractKey(tuple: Tuple): Value[];
  validateTuple(tuple: Tuple): void;
}
export interface TransactionOptions {
  isolation?: TransactionIsolation;
  synchronous?: boolean;
}
export interface Transactions {
  readonly id: bigint|null;
  readonly isolation: TransactionIsolation|null;
  begin(options?: TransactionOptions): void;
  commit(): void;
  rollback(): void;
  setIsolation(value: TransactionIsolation): void;
  makeSynchronous(): void;
  run<T>(fn: () => T extends PromiseLike<unknown>? never : T,
                             options?: TransactionOptions): T;
}
export interface Sequence {
  readonly id: number;
  next(): bigint;
  readonly current: bigint;
  set(value: number|bigint): void;
  reset(): void;
}
export interface Session {
  readonly id: bigint;
  send(
      header: Map<Value, Value>|{[key: string]: Value},
      body?: Map<Value, Value>|{[key: string]: Value}|null): void;
}
export interface Sessions {
  current(): Session;
  get(id: number|bigint): Session;
  broadcastLua(key: string, expression: string): void;
}
export interface Errors {
  new(code: number, message: string,
      location?: {file?: string; line?: number}): TarantoolError;
  set(error: TarantoolError): void;
  last(): TarantoolError|null;
  clear(): void;
  format(error: TarantoolError): string;
}
export interface Log {
  write(level: LogLevel, message: unknown, options?: {
    context?: {file: string; line: number},
  }): void;
}
export interface Metrics {
  uptime(): number;
}
export interface MessagePack {
  pack(value: Value): Uint8Array;
  unpack(data: Uint8Array, options?: {rawExtensions?: boolean}): Value;
  fromJson(value: string): Uint8Array;
  toJson(data: Uint8Array): string;
}
export class Tarantool {
  constructor(options?: {bindings?: Bindings});
  readonly schemaVersion: number;
  space(nameOrId: string|number): Space;
  sequence(id: number): Sequence;
  tuple(value: Value[]): Tuple;
  keyDef(parts: KeyPart[]): KeyDef;
  tupleFormat(definitions?: KeyDef[]): TupleFormat;
  readonly txn: Transactions;
  readonly session: Sessions;
  readonly error: Errors;
  readonly log: Log;
  readonly metrics: Metrics;
  readonly msgpack: MessagePack;
  transaction<T>(fn: () => T extends PromiseLike<unknown>? never : T,
                                     options?: TransactionOptions): T;
}
export const box: Tarantool;

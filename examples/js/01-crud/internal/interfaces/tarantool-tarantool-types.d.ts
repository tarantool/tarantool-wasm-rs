declare module 'tarantool:tarantool/types@0.1.0' {
  export { Iterator };
}
export interface BoxError {
  message: string,
  /**
   * Human-readable message.
   */
  type: string,
  /**
   * Error type: e.g., "ClientError", "SocketError".
   */
  code: number,
  /**
   * IPROTO or internal error code.
   */
  payload?: Array<[string, string]>,
  /**
   * Optional additional fields.
   */
  file?: string,
  /**
   * File where error originated (if available).
   */
  line?: number,
}
export interface Space {
  id: number,
}
export interface Index {
  id: number,
  /**
   * Index ID (within space).
   */
  spaceId: number,
  /**
   * ID of the parent space.
   */
  indexBase: number,
}
export interface LogContext {
  filename: string,
  /**
   * The name of the source file where the log is generated.
   */
  line: number,
}
/**
 * # Variants
 * 
 * ## `"s-fatal"`
 * 
 * ## `"s-syserror"`
 * 
 * Critical failure, Tarantool exits
 * ## `"s-error"`
 * 
 * System error (errno-based)
 * ## `"s-crit"`
 * 
 * Recoverable error
 * ## `"s-warn"`
 * 
 * Severe error
 * ## `"s-info"`
 * 
 * Warning
 * ## `"s-verbose"`
 * 
 * Informational message
 * ## `"s-debug"`
 * 
 * Verbose message
 */
export type LogLevel = 's-fatal' | 's-syserror' | 's-error' | 's-crit' | 's-warn' | 's-info' | 's-verbose' | 's-debug';
export interface Session {
  id: bigint,
}
/**
 * # Variants
 * 
 * ## `"txn-isolation-default"`
 * 
 * ## `"txn-isolation-best-effort"`
 * 
 * take isolation level from global default_isolation_level.
 * ## `"txn-isolation-read-committed"`
 * 
 * determine isolation level automatically (default).
 * ## `"txn-isolation-read-confirmed"`
 * 
 * allow to read committed, but not confirmed changes.
 * ## `"txn-isolation-linearizable"`
 * 
 * allow to read only confirmed changes.
 */
export type TxnIsolationLevel = 'txn-isolation-default' | 'txn-isolation-best-effort' | 'txn-isolation-read-committed' | 'txn-isolation-read-confirmed' | 'txn-isolation-linearizable';
/**
 * # Variants
 * 
 * ## `"iter-eq"`
 * 
 * ## `"iter-req"`
 * 
 * Equal
 * ## `"iter-all"`
 * 
 * Reverse EQ
 * ## `"iter-lt"`
 * 
 * Full scan
 * ## `"iter-le"`
 * 
 * ## `"iter-ge"`
 * 
 * Less than / less equal
 * ## `"iter-gt"`
 * 
 * ## `"iter-bits-all-set"`
 * 
 * Greater equal / greater than
 * ## `"iter-bits-any-set"`
 * 
 * Bitmask: all bits set
 * ## `"iter-bits-all-not-set"`
 * 
 * Bitmask: any bit set
 * ## `"iter-overlaps"`
 * 
 * Bitmask: all not set
 * ## `"iter-neighbor"`
 * 
 * Range overlaps
 * ## `"iter-np"`
 * 
 * Spatial index
 * ## `"iter-pp"`
 */
export type IteratorType = 'iter-eq' | 'iter-req' | 'iter-all' | 'iter-lt' | 'iter-le' | 'iter-ge' | 'iter-gt' | 'iter-bits-all-set' | 'iter-bits-any-set' | 'iter-bits-all-not-set' | 'iter-overlaps' | 'iter-neighbor' | 'iter-np' | 'iter-pp';
export interface Sequence {
  id: number,
}
export interface KeyPartFlags {
  isNullable?: boolean,
  /**
   * Allows NULL
   */
  excludeNull?: boolean,
}
export interface KeyPart {
  fieldNo: number,
  /**
   * Index of field in tuple
   */
  fieldType: string,
  /**
   * E.g. "unsigned", "string"
   */
  collation?: string,
  /**
   * Optional collation name
   */
  path?: string,
  /**
   * JSON path (for multikeys)
   */
  flags: KeyPartFlags,
}
export type BoxTuple = bigint;
export type KeyDef = bigint;
export type TupleFormat = bigint;
export type TupleField = bigint;

export class Iterator {
  static newIterator(index: Index, iteratorType: IteratorType, key: Uint8Array): Iterator;
  next(): BoxTuple;
}

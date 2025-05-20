declare module 'tarantool:tarantool/types@0.1.0' {
  export interface BoxError {
    message: string,
    type: string,
    code: number,
    payload?: Array<[string, string]>,
    file?: string,
    line?: number,
  }
  export interface Space {
    id: number,
  }
  export interface Index {
    id: number,
    spaceId: number,
    indexBase: number,
  }
  export interface LogContext {
    filename: string,
    line: number,
  }
  /**
   * # Variants
   * 
   * ## `"s-fatal"`
   * 
   * ## `"s-syserror"`
   * 
   * ## `"s-error"`
   * 
   * ## `"s-crit"`
   * 
   * ## `"s-warn"`
   * 
   * ## `"s-info"`
   * 
   * ## `"s-verbose"`
   * 
   * ## `"s-debug"`
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
   * ## `"txn-isolation-read-committed"`
   * 
   * ## `"txn-isolation-read-confirmed"`
   * 
   * ## `"txn-isolation-linearizable"`
   */
  export type TxnIsolationLevel = 'txn-isolation-default' | 'txn-isolation-best-effort' | 'txn-isolation-read-committed' | 'txn-isolation-read-confirmed' | 'txn-isolation-linearizable';
  /**
   * # Variants
   * 
   * ## `"iter-eq"`
   * 
   * ## `"iter-req"`
   * 
   * ## `"iter-all"`
   * 
   * ## `"iter-lt"`
   * 
   * ## `"iter-le"`
   * 
   * ## `"iter-ge"`
   * 
   * ## `"iter-gt"`
   * 
   * ## `"iter-bits-all-set"`
   * 
   * ## `"iter-bits-any-set"`
   * 
   * ## `"iter-bits-all-not-set"`
   * 
   * ## `"iter-overlaps"`
   * 
   * ## `"iter-neighbor"`
   * 
   * ## `"iter-np"`
   * 
   * ## `"iter-pp"`
   */
  export type IteratorType = 'iter-eq' | 'iter-req' | 'iter-all' | 'iter-lt' | 'iter-le' | 'iter-ge' | 'iter-gt' | 'iter-bits-all-set' | 'iter-bits-any-set' | 'iter-bits-all-not-set' | 'iter-overlaps' | 'iter-neighbor' | 'iter-np' | 'iter-pp';
  export interface Sequence {
    id: number,
  }
  export interface KeyPartFlags {
    isNullable?: boolean,
    excludeNull?: boolean,
  }
  export interface KeyPart {
    fieldNo: number,
    fieldType: string,
    collation?: string,
    path?: string,
    flags: KeyPartFlags,
  }
  export type BoxTuple = bigint;
  export type KeyDef = bigint;
  export type TupleFormat = bigint;
  export type TupleField = bigint;
  
  export class Iterator {
    /**
     * This type does not have a public constructor.
     */
    private constructor();
    static newIterator(index: Index, iteratorType: IteratorType, key: Uint8Array): Iterator;
    next(): BoxTuple;
  }
}

/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/txn@0.1.0' {
  export function txnId(): bigint;
  export function txnIsolation(): TxnIsolationLevel;
  export function txn(): boolean;
  export function begin(): void;
  export function commit(): void;
  export function rollback(): void;
  export function txnSetIsolation(isolation: TxnIsolationLevel): void;
  export function txnMakeSync(): void;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
  export type TxnIsolationLevel = import('tarantool:tarantool/types@0.1.0').TxnIsolationLevel;
}

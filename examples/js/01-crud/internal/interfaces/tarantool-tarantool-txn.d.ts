declare module 'tarantool:tarantool/txn@0.1.0' {
  /**
   * Returns the current transaction ID.
   * 
   * A transaction ID is a non-persistent unique identifier of the current transaction.
   * If no transaction is active, the return value is `-1`.
   * 
   * ### Returns
   * - A signed 64-bit integer representing the transaction ID, or `-1` if no transaction.
   */
  export function txnId(): bigint;
  /**
   * Returns the isolation level of the current transaction.
   * 
   * Returns `-1` if there is no active transaction.
   * 
   * ### Returns
   * - A `txn-isolation-level` enum value indicating the current isolation level.
   */
  export function txnIsolation(): TxnIsolationLevel;
  /**
   * Checks whether a transaction is currently active.
   * 
   * ### Returns
   * - `true` if a transaction is active; `false` otherwise.
   */
  export function txn(): boolean;
  /**
   * Begins a new transaction.
   * 
   * Must be called before any data manipulation (INSERT, UPDATE, etc.).
   * Fails if a transaction is already active in the fiber.
   * 
   * ### Raises
   * - `box-error
   */
  export function begin(): void;
  /**
   * Commits the current transaction.
   * 
   * Finalizes and applies all changes. If a disk error occurs, it returns an error.
   * 
   * ### Raises
   * - `box-error
   */
  export function commit(): void;
  /**
   * Rolls back the current transaction.
   * 
   * Discards all changes made within the current transaction.
   * 
   * ### Raises
   * - `box-error
   */
  export function rollback(): void;
  /**
   * Sets the isolation level for the current transaction.
   * 
   * Must be called before the first DML operation in the transaction.
   * 
   * ### Parameters
   * - `isolation`: A `txn-isolation-level` enum value, such as `read-committed`.
   * 
   * ### Raises
   * - `box-error
   */
  export function txnSetIsolation(isolation: TxnIsolationLevel): void;
  /**
   * Marks the current transaction as synchronous.
   * 
   * All changes will be synchronized to disk before commit completes.
   */
  export function txnMakeSync(): void;
}
import type { BoxError } from './tarantool-tarantool-types.js';
export { BoxError };
import type { TxnIsolationLevel } from './tarantool-tarantool-types.js';
export { TxnIsolationLevel };

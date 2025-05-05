declare module 'tarantool:tarantool/error@0.1.0' {
  /**
   * Creates a new error object with the given message, type, and error code.
   * 
   * ### Parameters
   * - `message`: The error message.
   * - `type`: The error type name.
   * - `code`: The numeric error code.
   * 
   * ### Returns
   * - A new `box-error` object.
   */
  export function errorNew(message: string, type: string, code: number, file: string | undefined, line: number | undefined): BoxError;
  /**
   * Sets the given error as the current "last error".
   * 
   * After calling this, `error.last()` will return the same error until cleared or overwritten.
   * 
   * ### Parameters
   * - `err`: The error to set.
   */
  export function set(err: BoxError): void;
  /**
   * Returns the last error set by the system or user code.
   * 
   * Similar to `errno` in C. Returns `none` if no error is currently set.
   * 
   * ### Returns
   * - `box-error` or `none` if no error is currently set.
   */
  export function last(): BoxError | undefined;
  /**
   * Clears the last error.
   * 
   * This is typically used to reset the error state before performing new operations.
   */
  export function clear(): void;
  /**
   * Converts a `box-error` object to a string representation.
   * 
   * ### Parameters
   * - `err`: The error to stringify.
   * 
   * ### Returns
   * - A human-readable string describing the error.
   */
  export function toString(err: BoxError): string;
}
import type { BoxError } from './tarantool-tarantool-types.js';
export { BoxError };

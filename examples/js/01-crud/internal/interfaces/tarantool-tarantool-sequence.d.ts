declare module 'tarantool:tarantool/sequence@0.1.0' {
  /**
   * Generates the next value of the specified sequence.
   * 
   * This updates the state of the sequence.
   * 
   * ### Parameters
   * - `seq`: The target sequence object.
   * 
   * ### Returns
   * - The next value of the sequence as `s64`.
   * 
   * ### Raises
   * - `box-error
   */
  export function next(seq: Sequence): bigint;
  /**
   * Returns the current value of the specified sequence.
   * 
   * Unlike `next()`, this does **not** increment the sequence.
   * Returns an error if the sequence was never accessed via `next()` or `set()`.
   * 
   * ### Parameters
   * - `seq`: The target sequence object.
   * 
   * ### Returns
   * - The current value of the sequence as `s64`.
   * 
   * ### Raises
   * - `box-error
   */
  export function current(seq: Sequence): bigint;
  /**
   * Sets the sequence to a specific value.
   * 
   * This function is useful when you want to jump to a specific number in the sequence.
   * 
   * ### Parameters
   * - `seq`: The target sequence object.
   * - `value`: The new value to assign.
   * 
   * ### Raises
   * - `box-error``
   */
  export function set(seq: Sequence, value: bigint): void;
  /**
   * Resets the sequence to its initial state.
   * 
   * This call clears the current sequence value, effectively "forgetting" the state.
   * After a reset, calling `current()` will return an error until `next()` or `set()` is called again.
   * 
   * Internally, if no transaction is active, this function starts and commits one.
   * 
   * ### Parameters
   * - `seq`: The target sequence object.
   * 
   * ### Raises
   * - `box-error
   */
  export function reset(seq: Sequence): void;
}
import type { Sequence } from './tarantool-tarantool-types.js';
export { Sequence };
import type { BoxError } from './tarantool-tarantool-types.js';
export { BoxError };

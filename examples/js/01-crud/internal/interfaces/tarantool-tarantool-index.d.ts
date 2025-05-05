declare module 'tarantool:tarantool/index@0.1.0' {
  /**
   * Returns the number of tuples stored in the index.
   * 
   * ### Parameters
   * - `index`: The index to query.
   * 
   * ### Returns
   * - The number of tuples in the index, or a `box-error` on failure.
   * 
   * ### Raises
   * - `box-error
   */
  export function len(index: Index): bigint;
  /**
   * Returns the memory size of the index in bytes.
   * 
   * ### Parameters
   * - `index`: The index to inspect.
   * 
   * ### Returns
   * - Size in bytes, or a `box-error`.
   * 
   * ### Raises
   * - `box-error
   */
  export function bsize(index: Index): bigint;
  /**
   * Returns a random tuple from the index.
   * 
   * ### Parameters
   * - `index`: The index to read from.
   * - `rnd`: A random seed value.
   * 
   * ### Returns
   * - A random tuple or a `box-error`.
   * 
   * ### Raises
   * - `box-error
   */
  export function random(index: Index, rnd: number): BoxTuple;
  /**
   * Gets a tuple by key from the index.
   * 
   * The key must be a MsgPack-encoded JSON array like `[1]`.
   * 
   * ### Parameters
   * - `index`: The index to search.
   * - `key`: MsgPack-encoded key (as JSON UTF-8 string).
   * 
   * ### Returns
   * - The found tuple or an error.
   * 
   * ### Raises
   * - `box-error
   */
  export function get(index: Index, key: Uint8Array): BoxTuple;
  /**
   * Finds the minimal tuple matching the given key.
   * 
   * ### Parameters
   * - `index`: The index to search.
   * - `key`: MsgPack-encoded key.
   * 
   * ### Returns
   * - The smallest matching tuple or an error.
   * 
   * ### Raises
   * - `box-error
   */
  export function min(index: Index, key: Uint8Array): BoxTuple;
  /**
   * Finds the maximal tuple matching the given key.
   * 
   * ### Parameters
   * - `index`: The index to search.
   * - `key`: MsgPack-encoded key.
   * 
   * ### Returns
   * - The largest matching tuple or an error.
   * 
   * ### Raises
   * - `box-error
   */
  export function max(index: Index, key: Uint8Array): BoxTuple;
  /**
   * Counts the number of tuples matching the given key and iterator type.
   * 
   * ### Parameters
   * - `index`: Index to query.
   * - `iter-type`: One of `iterator-type` values.
   * - `key`: MsgPack-encoded key.
   * 
   * ### Returns
   * - Count of matching tuples, or an error.
   * 
   * ### Raises
   * - `box-error
   */
  export function count(index: Index, iterType: IteratorType, key: Uint8Array): bigint;
}
import type { Index } from './tarantool-tarantool-types.js';
export { Index };
import type { BoxError } from './tarantool-tarantool-types.js';
export { BoxError };
import type { BoxTuple } from './tarantool-tarantool-types.js';
export { BoxTuple };
import type { IteratorType } from './tarantool-tarantool-types.js';
export { IteratorType };

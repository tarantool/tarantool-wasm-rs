declare module 'tarantool:tarantool/tuple@0.1.0' {
  export { TupleIterator };
  /**
   * Creates a new tuple using the specified format and raw MsgPack data.
   * 
   * The data must be a MsgPack-encoded array (e.g., `[1, "abc", {"x": 42}]`).
   * 
   * ### Parameters
   * - `format`: A tuple format object.
   * - `data`: MsgPack-encoded data (e.g., via `msgpack.encode(json.dumps(...))`)
   * 
   * ### Returns
   * - A new `box-tuple`, or `box-error`.
   * 
   * ### Raises
   * - `box-error
   */
  export { _new as new };
  declare function _new(data: Uint8Array): BoxTuple;
  /**
   * Increases the reference count of the tuple.
   * 
   * Call this if you want to store the tuple beyond the current function scope.
   * 
   * ### Parameters
   * - `t`: The tuple to retain.
   * 
   * ### Returns
   * - The new reference count as `s32`.
   */
  export function ref(t: BoxTuple): number;
  /**
   * Decreases the reference count of the tuple.
   * 
   * If the count reaches zero, the tuple is freed.
   * 
   * ### Parameters
   * - `t`: The tuple to release.
   */
  export function unref(t: BoxTuple): void;
  /**
   * Returns the number of fields in the tuple (i.e., array size).
   * 
   * ### Parameters
   * - `t`: The tuple.
   * 
   * ### Returns
   * - Field count as `u32`.
   */
  export function fieldCount(t: BoxTuple): number;
  /**
   * Returns the byte size of the internal MsgPack data.
   * 
   * Useful for buffer allocations.
   * 
   * ### Parameters
   * - `t`: The tuple.
   * 
   * ### Returns
   * - Number of bytes used as `u64`.
   */
  export function bsize(t: BoxTuple): bigint;
  /**
   * Serializes the tuple back to MsgPack byte array.
   * 
   * This data can be passed to `decode` or reused elsewhere.
   * 
   * ### Parameters
   * - `t`: The tuple.
   * 
   * ### Returns
   * - A MsgPack-encoded byte array, or `box-error`.
   * 
   * ### Raises
   * - `box-error
   */
  export function toBuf(t: BoxTuple): Uint8Array;
  /**
   * Returns the format object associated with the tuple.
   * 
   * ### Parameters
   * - `t`: The tuple.
   * 
   * ### Returns
   * - The `tuple-format` used to construct the tuple.
   * 
   * ### Raises
   * - `box-error
   */
  export function format(t: BoxTuple): TupleFormat;
  /**
   * Returns a pointer to the field at the specified index (zero-based).
   * 
   * The returned value is a pointer to raw MsgPack-encoded data and must be decoded.
   * 
   * ### Parameters
   * - `t`: The tuple.
   * - `idx`: Field index (starting from 0).
   * 
   * ### Returns
   * - `tuple-field` (pointer), or `box-error`.
   * 
   * ### Raises
   * - `box-error
   */
  export function field(t: BoxTuple, idx: number): TupleField;
  /**
   * Returns a pointer to the field by JSON path expression.
   * 
   * Index base:
   * - 0 → C-style (zero-based)
   * - 1 → Lua-style (one-based)
   * 
   * ### Parameters
   * - `t`: The tuple.
   * - `path`: Path expression (e.g., `"[2].x"`).
   * - `index-base`: Either 0 or 1.
   * 
   * ### Returns
   * - `tuple-field` (pointer).
   * 
   * ### Raises
   * - `box-error
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * field = tt_tuple.field_by_path(tup, "[2].x", 1)
   * value = json.loads(msgpack.decode_from_raw_ptr(field).decode('utf-8'))
   * ```
   */
  export function fieldByPath(t: BoxTuple, path: string, indexBase: number): TupleField;
  /**
   * Updates fields in a tuple using the given update expression.
   * 
   * The expression must be a MsgPack-encoded array of operations like:
   * `[["=", 1, "new_value"]]`, where the first item is an operation, second is field index, and third is the value.
   * 
   * ### Parameters
   * - `t`: The tuple to update.
   * - `expr`: MsgPack-encoded update expression.
   * 
   * ### Returns
   * - A new `box-tuple` with applied updates, or a `box-error`.
   * 
   * ### Raises
   * - `box-error
   */
  export function update(t: BoxTuple, expr: Uint8Array): BoxTuple;
  /**
   * Performs an upsert operation on the tuple using the provided expression.
   * 
   * This operation applies updates if the field exists or appends otherwise.
   * Common upsert usage includes incrementing fields: `[["+", 2, 1]]`.
   * 
   * ### Parameters
   * - `t`: The tuple.
   * - `expr`: MsgPack-encoded upsert expression.
   * 
   * ### Returns
   * - A new `box-tuple` with result, or a `box-error`.
   * 
   * ### Raises
   * - `box-error
   */
  export function upsert(t: BoxTuple, expr: Uint8Array): BoxTuple;
  /**
   * Validates a tuple against the given format.
   * 
   * Checks field types and nullability constraints.
   * 
   * ### Parameters
   * - `t`: The tuple to validate.
   * - `format`: Tuple format to validate against.
   * 
   * ### Returns
   * - `true` if valid, `false` otherwise.
   * 
   * ### Raises
   * - `box-error
   */
  export function validate(t: BoxTuple, format: TupleFormat): boolean;
  /**
   * Compares two tuples using a given key definition.
   * 
   * The comparison is lexicographical by key fields.
   * 
   * ### Parameters
   * - `key-def`: Definition of fields to compare.
   * - `left`: First tuple.
   * - `right`: Second tuple.
   * 
   * ### Returns
   * - `0` if equal
   * - `<0` if `left < right`
   * - `>0` if `left > right`
   */
  export function compare(keyDef: KeyDef, left: BoxTuple, right: BoxTuple): number;
  /**
   * Compares a tuple with a key using the given key definition.
   * 
   * ### Parameters
   * - `tuple`: Tuple to compare.
   * - `key`: MsgPack-encoded key (e.g., `[1]`)
   * - `key-def`: Key definition.
   * 
   * ### Returns
   * - `0` if key matches tuple
   * - `<0` if tuple < key
   * - `>0` if tuple > key
   */
  export function compareWithKey(tuple: BoxTuple, key: Uint8Array, keyDef: KeyDef): number;
}
import type { BoxError } from './tarantool-tarantool-types.js';
export { BoxError };
import type { BoxTuple } from './tarantool-tarantool-types.js';
export { BoxTuple };
import type { TupleField } from './tarantool-tarantool-types.js';
export { TupleField };
import type { TupleFormat } from './tarantool-tarantool-types.js';
export { TupleFormat };
import type { KeyDef } from './tarantool-tarantool-types.js';
export { KeyDef };

export class TupleIterator {
  /**
  * Creates a new iterator over the fields of a tuple.
  * 
  * This allows sequential or random access to MsgPack fields within the tuple.
  * 
  * ### Parameters
  * - `tuple`: The tuple to iterate.
  * 
  * ### Returns
  * - A new `tuple-iterator` object, or a `box-error`.
  * 
  * ### Raises
  * - `box-error
  */
  static newTupleIterator(tuple: BoxTuple): TupleIterator;
  /**
  * Returns the current iterator position (index of the next field).
  * 
  * ### Returns
  * - The next field index (zero-based).
  */
  position(): number;
  /**
  * Rewinds the iterator to the beginning of the tuple.
  * 
  * After this call, the next field will be the first.
  */
  rewind(): void;
  /**
  * Seeks to a given field index in the tuple.
  * 
  * The next call to `next()` will return this field.
  * 
  * ### Parameters
  * - `pos`: Zero-based field index to seek to.
  * 
  * ### Returns
  * - `tuple-field` pointer.
  * 
  * ### Raises
  * - `box-error
  */
  seek(pos: number): TupleField;
  /**
  * Returns the next field in the tuple.
  * 
  * Returns `null` if there are no more fields.
  * 
  * ### Returns
  * - `tuple-field` pointer.
  * 
  * ### Raises
  * - `box-error
  */
  next(): TupleField;
}

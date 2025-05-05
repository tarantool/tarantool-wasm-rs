declare module 'tarantool:tarantool/tuple-format@0.1.0' {
  /**
   * Returns the default tuple format.
   * 
   * This format is used to create tuples not bound to any specific space.
   * 
   * ### Returns
   * - A `tuple-format` object.
   */
  export { _default as default };
  declare function _default(): TupleFormat;
  /**
   * Creates a new tuple format based on a list of key definitions.
   * 
   * Key definitions define constraints and structure (field types, paths, etc.).
   * 
   * ### Parameters
   * - `key-defs`: A list of key definitions that describe indexable parts.
   * 
   * ### Returns
   * - A `tuple-format` object.
   * 
   * ### Raises
   * - `box-error
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * parts = [
   * KeyPart(field_no=0, field_type="unsigned", collation=None, path=None,
   * flags=KeyPartFlags.IS_NULLABLE),
   * KeyPart(field_no=1, field_type="string", collation=None, path=None,
   * flags=KeyPartFlags.EXCLUDE_NULL)
   * ]
   * kd = key_def.new(parts)
   * tf = tuple_format.new([kd])
   * ```
   */
  export { _new as new };
  declare function _new(keyDefs: BigUint64Array): TupleFormat;
  /**
   * Increments the reference counter of the tuple format.
   * 
   * You should call this if you plan to store the format and use it across calls or threads.
   * 
   * ### Parameters
   * - `tf`: Tuple format to retain.
   */
  export function ref(tf: TupleFormat): void;
  /**
   * Decrements the reference counter of the tuple format.
   * 
   * Must be called to release the format if it was previously retained via `ref`.
   * 
   * ### Parameters
   * - `tf`: Tuple format to release.
   */
  export function unref(tf: TupleFormat): void;
}
import type { KeyDef } from './tarantool-tarantool-types.js';
export { KeyDef };
import type { BoxError } from './tarantool-tarantool-types.js';
export { BoxError };
import type { TupleFormat } from './tarantool-tarantool-types.js';
export { TupleFormat };

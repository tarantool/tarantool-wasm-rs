declare module 'tarantool:tarantool/key-def@0.1.0' {
  /**
   * Creates a new key definition from a list of parts.
   * 
   * Each key part defines one field to be used in a key for indexing or validation.
   * The `KeyPart` structure includes field number, type, optional collation, path, and flags.
   * 
   * ### Parameters
   * - `parts`: List of `key-part` objects.
   * 
   * ### Returns
   * - A new `key-def` object on success.
   * 
   * ### Raises
   * - `box-error
   * 
   * **Python**
   * ```
   * parts = [
   * KeyPart(field_no=0, field_type="unsigned", collation=None, path=None,
   * flags=KeyPartFlags.IS_NULLABLE),
   * KeyPart(field_no=1, field_type="string", collation="unicode", path=None,
   * flags=KeyPartFlags.EXCLUDE_NULL)
   * ]
   * kd = key_def.new(parts)
   * ```
   */
  export { _new as new };
  declare function _new(parts: Array<KeyPart>): KeyDef;
  /**
   * Creates a deep copy of the given key definition.
   * 
   * The returned `key-def` is independent of the original and must be deleted manually.
   * 
   * ### Parameters
   * - `key-def`: The key definition to duplicate.
   * 
   * ### Returns
   * - A duplicated `key-def`.
   * 
   * ### Raises
   * - `box-error
   */
  export function dup(keyDef: KeyDef): KeyDef;
  /**
   * Merges two key definitions into one.
   * 
   * Fields from the right-hand side are appended if they are not duplicates of the left.
   * The resulting merged definition can be used for composite index construction.
   * 
   * ### Parameters
   * - `left`: The base key definition.
   * - `right`: The key definition to append fields from.
   * 
   * ### Returns
   * - A new merged `key-def`.
   * 
   * ### Raises
   * - `box-error
   */
  export function merge(left: KeyDef, right: KeyDef): KeyDef;
  /**
   * Deletes a key definition, freeing any associated memory.
   * 
   * You **must** call this for all key definitions you create with `new`, `dup`, or `merge`.
   * 
   * ### Parameters
   * - `key-def`: The key definition to delete.
   * 
   * ### Raises
   * - `box-error
   */
  export { _delete as delete };
  declare function _delete(keyDef: KeyDef): void;
  /**
   * Returns a list of key parts from a given key definition.
   * 
   * Useful for debugging and introspection — lets you inspect fields and their order.
   * 
   * ### Parameters
   * - `key-def`: The key definition to inspect.
   * 
   * ### Returns
   * - List of `key-part` objects.
   * 
   * ### Raises
   * - `box-error
   */
  export function dumpParts(keyDef: KeyDef): Array<KeyPart>;
  /**
   * Validates whether a given key conforms to the key definition.
   * 
   * Checks the number of fields and their types, allowing partial matches
   * (e.g. prefix of the full key).
   * 
   * ### Parameters
   * - `key-def`: Key definition to validate against.
   * - `key`: MsgPack-encoded key (e.g., `[1, "abc"]`, encoded with `msgpack.encode(json.dumps(...))`)
   * 
   * ### Returns
   * - A tuple `(is_valid: bool, size: u32)`
   * - `is_valid`: Whether the key is valid.
   * - `size`: Size of decoded key in bytes (if valid), or 0 if invalid.
   * 
   * ### Raises
   * - `box-error
   */
  export function validateKey(keyDef: KeyDef, key: Uint8Array): [boolean, number];
  /**
   * Validates whether a key **fully** matches the key definition.
   * 
   * Differs from `validate-key` by requiring exact number of parts.
   * 
   * ### Parameters
   * - `key-def`: Key definition to validate against.
   * - `key`: MsgPack-encoded key (e.g., `[1, "abc"]`)
   * 
   * ### Returns
   * - A tuple `(is_valid: bool, size: u32)` like in `validate-key`.
   * 
   * ### Raises
   * - `box-error
   */
  export function validateFullKey(keyDef: KeyDef, key: Uint8Array): [boolean, number];
  /**
   * Returns the number of parts in the key definition.
   * 
   * Each part corresponds to one field used in index comparison or validation.
   * 
   * ### Parameters
   * - `key-def`: The key definition.
   * 
   * ### Returns
   * - Number of parts as `u32`.
   * 
   * ### Raises
   * - `box-error
   */
  export function partCount(keyDef: KeyDef): number;
  /**
   * Extracts a key from a given tuple based on the key definition.
   * 
   * The result is a `box-tuple` containing only the fields specified by the key definition.
   * 
   * ### Parameters
   * - `key-def`: Key definition describing how to extract fields.
   * - `tuple`: Source `box-tuple`.
   * 
   * ### Returns
   * - A new `box-tuple` with key fields only.
   */
  export function extractKey(keyDef: KeyDef, tuple: BoxTuple): BoxTuple;
  /**
   * Validates whether the tuple contains all required fields
   * and types as specified by the key definition.
   * 
   * This is useful to check if a `box-tuple` is compatible with an index.
   * 
   * ### Parameters
   * - `key-def`: Key definition to validate against.
   * - `tuple`: A `box-tuple` to validate.
   * 
   * ### Returns
   * - `true` if the tuple satisfies the key definition, otherwise `false`.
   */
  export function validateTuple(keyDef: KeyDef, tuple: BoxTuple): boolean;
}
import type { BoxError } from './tarantool-tarantool-types.js';
export { BoxError };
import type { BoxTuple } from './tarantool-tarantool-types.js';
export { BoxTuple };
import type { KeyDef } from './tarantool-tarantool-types.js';
export { KeyDef };
import type { KeyPart } from './tarantool-tarantool-types.js';
export { KeyPart };

declare module 'tarantool:tarantool/ttbox@0.1.0' {
  /**
   * Returns the current schema version of the database.
   * 
   * The schema version increases whenever the schema changes, such as creating or dropping spaces or indexes.
   * 
   * ### Returns
   * - A 64-bit integer representing the current schema version.
   */
  export function schemaVersion(): bigint;
  /**
   * Retrieves a space object by its name.
   * 
   * This function performs a lookup for a space by name and returns a reference used in subsequent operations.
   * 
   * ### Parameters
   * - `name`: The name of the space to retrieve.
   * 
   * ### Returns
   * - A space object if found, or an error otherwise.
   */
  export function spaceByName(name: string): Space;
  /**
   * Retrieves an index object by name within a space.
   * 
   * This function looks up the specified index in the given space and returns a reference to it.
   * 
   * ### Parameters
   * - `space`: The space to search within.
   * - `name`: The name of the index to retrieve.
   * 
   * ### Returns
   * - An index object if found, or an error otherwise.
   */
  export function indexByName(space: Space, name: string): Index;
  /**
   * Inserts a new tuple into a space.
   * 
   * The tuple must be provided as a MsgPack-encoded array: `[field1, field2, ...]`.
   * 
   * ### Parameters
   * - `space`: The target space.
   * - `tup`: A MsgPack-encoded array representing the tuple.
   * 
   * ### Returns
   * - The inserted tuple.
   */
  export function insert(space: Space, tup: Uint8Array): BoxTuple;
  /**
   * Updates an existing tuple by key in the specified index.
   * 
   * - `key` must be a MsgPack array identifying the tuple, e.g. `[1]`
   * - `ops` must be a MsgPack array of operations, e.g. `[[ "=", 2, 30 ]]`
   * 
   * ### Parameters
   * - `index`: The index to use for locating the tuple.
   * - `key`: MsgPack-encoded key array.
   * - `ops`: MsgPack-encoded array of update operations.
   * 
   * ### Returns
   * - The updated tuple.
   */
  export function update(index: Index, key: Uint8Array, ops: Uint8Array): BoxTuple;
  /**
   * Replaces an existing tuple or inserts a new one.
   * 
   * ### Parameters
   * - `space`: The space to operate on.
   * - `tup`: A MsgPack-encoded array representing the new tuple.
   * 
   * ### Returns
   * - The replaced tuple.
   */
  export function replace(space: Space, tup: Uint8Array): BoxTuple;
  /**
   * Performs an UPSERT operation: update if exists, insert otherwise.
   * 
   * ### Parameters
   * - `index`: The index used for key lookup.
   * - `tup`: A MsgPack-encoded tuple to insert.
   * - `ops`: A MsgPack-encoded array of update operations.
   */
  export function upsert(index: Index, tup: Uint8Array, ops: Uint8Array): void;
  /**
   * Deletes a tuple by key from the specified index.
   * 
   * The key must be encoded as a MsgPack array, e.g. `[1]`.
   * 
   * ### Parameters
   * - `index`: The index used for key lookup.
   * - `key`: MsgPack-encoded key array.
   * 
   * ### Returns
   * - The deleted tuple, if found.
   */
  export { _delete as delete };
  declare function _delete(index: Index, key: Uint8Array): BoxTuple;
  /**
   * Removes all tuples from the space.
   * 
   * This operation is equivalent to SQL's `TRUNCATE TABLE`.
   * 
   * ### Parameters
   * - `space`: The target space to truncate.
   * 
   * ### Returns
   * - `null` on success, or an error.
   */
  export function truncate(space: Space): void;
}
import type { Space } from './tarantool-tarantool-types.js';
export { Space };
import type { BoxError } from './tarantool-tarantool-types.js';
export { BoxError };
import type { Index } from './tarantool-tarantool-types.js';
export { Index };
import type { BoxTuple } from './tarantool-tarantool-types.js';
export { BoxTuple };

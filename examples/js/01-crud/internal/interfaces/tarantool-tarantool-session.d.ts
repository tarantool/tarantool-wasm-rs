declare module 'tarantool:tarantool/session@0.1.0' {
  /**
   * Returns the current session.
   * 
   * The session object uniquely identifies the connection or context
   * (e.g. a client connection) from which the current code is executed.
   * 
   * ### Returns
   * - A `session` object representing the current session.
   */
  export function current(): Session;
  /**
   * Broadcasts a message to all watchers subscribed to the given key.
   * 
   * The `value` is interpreted as a string representation of a Lua object.
   * Internally maps to `box.broadcast(key, value)` from Lua.
   * 
   * ### Parameters
   * - `key`: A string event name to broadcast on.
   * - `value`: A stringified Lua object, e.g. `"{a = 1}"`.
   * 
   * ### Returns
   * - `ok` on success, or a `box-error` on failure.
   */
  export function broadcast(key: string, value: string): void;
  /**
   * Sends a raw IPROTO packet to a specific session.
   * 
   * Both the header and body must be valid MsgPack-encoded buffers.
   * 
   * Internally maps to `box_iproto_send(...)` from the C API.
   * 
   * ### Parameters
   * - `session`: Target session to send to.
   * - `header`: MsgPack-encoded map containing the request header.
   * - `body`: MsgPack-encoded body (optional but usually required).
   * 
   * ### Returns
   * - `ok` on success
   * 
   * ### Raises
   * - `box-error`
   */
  export function iprotoSend(session: Session, header: Uint8Array, body: Uint8Array): void;
}
import type { Session } from './tarantool-tarantool-types.js';
export { Session };
import type { BoxError } from './tarantool-tarantool-types.js';
export { BoxError };

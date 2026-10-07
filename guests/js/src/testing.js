// This entry point has no WIT imports and is usable in plain Node.js.
import {Tarantool} from './core.js';
export const createTarantool = (bindings) => new Tarantool({bindings});
export {createLuaCallbacks} from './lua-callback-core.js';

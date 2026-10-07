import * as bindings from 'docs:adder/lua-callback@0.1.3';

import {createLuaCallbacks} from './lua-callback-core.js';

export {LuaCallbackError} from './errors.js';
export {LuaCallback} from './lua-callback-core.js';

export const {open, withCallback} = createLuaCallbacks(bindings);

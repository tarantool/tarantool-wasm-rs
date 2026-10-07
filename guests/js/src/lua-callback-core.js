import {pack, unpack} from './codec.js';
import {LuaCallbackError} from './errors.js';
import {disposeHandle, Resource, usingResource} from './resource.js';

function invoke(fn, ...args) {
  try {
    return fn(...args);
  } catch (error) {
    if (typeof error?.payload === 'string') {
      throw new LuaCallbackError(error.payload, {cause: error});
    }
    throw error;
  }
}
/** Owns a Lua callback handle and exchanges complete MessagePack values. */
export class LuaCallback extends Resource {
  constructor(handle) {
    super(undefined, handle, disposeHandle);
  }
  /**
   * @return {import('./index.js').Value}
   */
  call(value) {
    if (arguments.length === 0) {
      value = null;
    }
    const h = this.getHandle_();
    return unpack(invoke(h.call.bind(h), pack(value)));
  }
}
export function createLuaCallbacks(bindings) {
  const open = (name) => {
    if (typeof name !== 'string' || !name) {
      throw new LuaCallbackError('callback name must be a nonempty string');
    }
    return new LuaCallback(invoke(bindings.open.bind(bindings), name));
  };
  return {open, withCallback: (name, fn) => usingResource(open(name), fn)};
}

import type {Bindings, Tarantool} from './index.js';
import type {LuaCallback} from './lua-callback.js';
export function createTarantool(bindings: Bindings): Tarantool;
export interface CallbackBindings {
  open(name: string):
      {call(payload: Uint8Array): Uint8Array;[Symbol.dispose](): void;};
}
export function createLuaCallbacks(bindings: CallbackBindings): {
  open(name: string): LuaCallback;
  withCallback<T>(name: string,
                  fn: (callback: LuaCallback) =>
                      T extends PromiseLike<unknown>? never : T): T;
};

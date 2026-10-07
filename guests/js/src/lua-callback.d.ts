import type {Value} from './codec.js';
import type {OwnedResource} from './index.js';
export {LuaCallbackError} from './index.js';
export class LuaCallback implements OwnedResource {
  private constructor();
  readonly closed: boolean;
  close(): void;
  [Symbol.dispose](): void;
  call(): Value;
  call(value: Value): Value;
}
export function open(name: string): LuaCallback;
export function withCallback<T>(
    name: string, fn: (callback: LuaCallback) => T extends PromiseLike<unknown>?
        never :
        T): T;

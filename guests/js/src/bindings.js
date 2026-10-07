import {TarantoolError} from './errors.js';

// ComponentizeJS throws WIT result errors as ComponentError(payload).
// Only records with the complete BoxError shape are converted; traps and JS
// exceptions keep their original identity.
/** Invokes WIT imports and preserves host error records. */
export class Bindings {
  constructor(modules) {
    this.modules = modules;
  }
  invoke(fn, ...args) {
    try {
      return fn(...args);
    } catch (error) {
      const value = error?.payload;
      if (value && typeof value.code === 'number' &&
          typeof value.message === 'string' &&
          typeof value.errorType === 'string') {
        throw new TarantoolError(value, {cause: error});
      }
      throw error;
    }
  }
  call(iface, method, ...args) {
    const module = this.modules[iface];
    return this.invoke(module[method].bind(module), ...args);
  }
  iterator(iface, ...args) {
    const type = this.modules[iface].Iterator;
    return this.invoke(type.new.bind(type), ...args);
  }
}

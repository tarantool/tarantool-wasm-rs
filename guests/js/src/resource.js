import {ClosedResourceError} from './errors.js';

/** Owns a host handle with explicit, idempotent release. */
export class Resource {
  constructor(bindings, handle, release) {
    /** @protected @const */
    this.bindings_ = bindings;
    /** @private */
    this.handle_ = handle;
    /** @private @const */
    this.release_ = release;
  }
  /** @return {boolean} Whether this handle has been released. */
  get closed() {
    return this.handle_ === undefined;
  }
  /** @protected @return {*} The open handle, or throws after close(). */
  getHandle_() {
    if (this.closed) {
      throw new ClosedResourceError(`${this.constructor.name} is closed`);
    }
    return this.handle_;
  }
  /** Releases the handle once; marks it closed even if host release fails. */
  close() {
    if (!this.closed) {
      const handle = this.handle_;
      this.handle_ = undefined;
      this.release_(handle);
    }
  }
}
if (Symbol.dispose) {
  Resource.prototype[Symbol.dispose] = Resource.prototype.close;
}

/** @param {!Disposable} handle A ComponentizeJS WIT resource. */
export function disposeHandle(handle) {
  if (!Symbol.dispose || typeof handle[Symbol.dispose] !== 'function') {
    throw new TypeError('WIT resource does not implement Symbol.dispose');
  }
  handle[Symbol.dispose]();
}
/**
 * @param {!Resource} resource Releases it without replacing a primary error.
 */
export function closeAfterError(resource) {
  try {
    resource.close();
  } catch {
    // Preserve the primary exception.
  }
}
/**
 * Runs a synchronous callback and releases the resource on every exit path.
 * @template R, T
 * @param {R} resource
 * @param {function(R): T} fn
 * @return {T}
 */
export function usingResource(resource, fn) {
  try {
    const result = fn(resource);
    if (result && typeof result.then === 'function') {
      throw new TypeError('resource helper requires a synchronous function');
    }
    resource.close();
    return result;
  } catch (error) {
    closeAfterError(resource);
    throw error;
  }
}
/**
 * Checks that every resource is open and belongs to the same binding adapter.
 * @param {!Object} owner
 * @param {...Resource} resources
 */
export function sameBindings(owner, ...resources) {
  for (const resource of resources) {
    if (!(resource instanceof Resource) ||
        owner.bindings_ !== resource.bindings_) {
      throw new TypeError('resources belong to different Tarantool bindings');
    }
    resource.getHandle_();
  }
}

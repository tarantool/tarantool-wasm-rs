import {enumValue, TransactionIsolation} from './types.js';

/** Manages synchronous transactions on the current host fiber. */
export class Transactions {
  constructor(bindings) {
    this.bindings_ = bindings;
    this.helperActive_ = false;
  }
  /**
   * @return {bigint|null}
   */
  get id() {
    return this.bindings_.call('transaction', 'id') ?? null;
  }
  /**
   * @return {import('./index.js').TransactionIsolation|null}
   */
  get isolation() {
    return this.bindings_.call('transaction', 'isolation') ?? null;
  }
  /**
   * @param {import('./index.js').TransactionOptions=} options
   */
  begin({isolation, synchronous = false} = {}) {
    if (this.id !== null) {
      throw new Error('nested transactions are unsupported');
    }
    if (isolation != null) {
      enumValue(TransactionIsolation, isolation);
    }
    this.bindings_.call('transaction', 'begin');
    try {
      if (isolation != null) {
        this.setIsolation(isolation);
      }
      if (synchronous) {
        this.makeSynchronous();
      }
    } catch (error) {
      try {
        this.rollback();
      } catch {
      }
      throw error;
    }
  }
  commit() {
    this.bindings_.call('transaction', 'commit');
  }
  rollback() {
    this.bindings_.call('transaction', 'rollback');
  }
  /**
   * @param {import('./index.js').TransactionIsolation} value
   */
  setIsolation(value) {
    this.bindings_.call(
        'transaction', 'setIsolation', enumValue(TransactionIsolation, value));
  }
  makeSynchronous() {
    this.bindings_.call('transaction', 'makeSynchronous');
  }
  /**
   * @template T
   * @param {function(): T} fn
   * @param {import('./index.js').TransactionOptions=} options
   * @return {T}
   */
  run(fn, options = {}) {
    if (typeof fn !== 'function' ||
        Object.prototype.toString.call(fn) === '[object AsyncFunction]') {
      throw new TypeError('transaction requires a synchronous function');
    }
    if (this.helperActive_) {
      throw new Error('nested transactions are unsupported');
    }
    this.begin(options);
    this.helperActive_ = true;
    let id;
    try {
      id = this.id;
      const result = fn();
      if (result && typeof result.then === 'function') {
        throw new TypeError(
            'async/thenable transaction results are unsupported');
      }
      if (this.id !== id) {
        throw new Error('transaction was ended or replaced inside its helper');
      }
      this.commit();
      return result;
    } catch (error) {
      // Do not roll back a replacement transaction belonging to the caller.
      try {
        if (id === undefined || this.id === id) {
          this.rollback();
        }
      } catch {
      }
      throw error;
    } finally {
      this.helperActive_ = false;
    }
  }
}

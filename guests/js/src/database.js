import {arrayBytes, keyBytes, operationsBytes, unpack} from './codec.js';
import {closeAfterError, disposeHandle, Resource, usingResource} from './resource.js';
import {enumValue, IteratorType} from './types.js';
import {integer} from './values.js';

export function unsigned(value, bits = 32) {
  return Number(integer(value, 0, 2n ** BigInt(bits) - 1n));
}
export function row(bindings, handle) {
  if (handle == null) {
    return null;
  }
  let value;
  try {
    value = unpack(bindings.call('boxTuple', 'toBytes', handle));
  } catch (error) {
    try {
      bindings.call('boxTuple', 'release', handle);
    } catch {
    }
    throw error;
  }
  bindings.call('boxTuple', 'release', handle);
  return value;
}
/** Owns a host index iterator and closes it on iteration completion. */
export class IndexIterator extends Resource {
  constructor(bindings, handle) {
    super(bindings, handle, disposeHandle);
  }
  /**
   * @return {this}
   */
  [Symbol.iterator]() {
    return this;
  }
  /**
   * @return {IteratorResult<import('./index.js').Value[]>}
   */
  next() {
    if (this.finished_) {
      return {done: true, value: undefined};
    }
    const handle = this.getHandle_();
    try {
      const tuple = this.bindings_.invoke(handle.next.bind(handle));
      if (tuple == null) {
        this.finished_ = true;
        this.close();
        return {done: true, value: undefined};
      }
      return {done: false, value: row(this.bindings_, tuple)};
    } catch (error) {
      closeAfterError(this);
      throw error;
    }
  }
  /**
   * @return {IteratorResult<import('./index.js').Value[]>}
   */
  return() {
    this.finished_ = true;
    this.close();
    return {done: true, value: undefined};
  }
  /**
   * @param {unknown} error
   * @return {IteratorResult<import('./index.js').Value[]>}
   */
  throw(error) {
    closeAfterError(this);
    throw error;
  }
}
/** Resolves a space name or ID on every operation to follow schema changes. */
export class Space {
  constructor(bindings, nameOrId) {
    this.bindings_ = bindings;
    this.nameOrId_ = identifier(nameOrId);
  }
  /** @private */
  resolve_() {
    if (typeof this.nameOrId_ !== 'string') {
      return {id: this.nameOrId_};
    }
    const value =
        this.bindings_.call('database', 'spaceByName', this.nameOrId_);
    if (value == null) {
      throw new RangeError(`space ${this.nameOrId_} does not exist`);
    }
    return value;
  }
  /**
   * @return {number}
   */
  get id() {
    return this.resolve_().id;
  }
  /**
   * @param {string|number=} nameOrId
   * @return {import('./index.js').Index}
   */
  index(nameOrId = 0) {
    return new Index(this.bindings_, this, nameOrId);
  }
  /**
   * @param {import('./index.js').Value[]} value
   * @return {import('./index.js').Value[]}
   */
  insert(value) {
    return row(
        this.bindings_,
        this.bindings_.call(
            'database', 'insert', this.resolve_(), arrayBytes(value)));
  }
  /**
   * @param {import('./index.js').Value[]} value
   * @return {import('./index.js').Value[]}
   */
  replace(value) {
    return row(
        this.bindings_,
        this.bindings_.call(
            'database', 'replace', this.resolve_(), arrayBytes(value)));
  }
  truncate() {
    this.bindings_.call('database', 'truncate', this.resolve_());
  }
  /**
   * @param {import('./index.js').Value} key
   * @return {import('./index.js').Value[]|null}
   */
  get(key) {
    return this.index().get(key);
  }
  /**
   * @param {import('./index.js').Value} key
   * @param {import('./index.js').Operations} operations
   * @return {import('./index.js').Value[]|null}
   */
  update(key, operations) {
    return this.index().update(key, operations);
  }
  /**
   * @param {import('./index.js').Value[]} value
   * @param {import('./index.js').Operations} operations
   */
  upsert(value, operations) {
    return this.index().upsert(value, operations);
  }
  /**
   * @param {import('./index.js').Value} key
   * @return {import('./index.js').Value[]|null}
   */
  delete(key) {
    return this.index().delete(key);
  }
  /**
   * @param {import('./index.js').Value=} key
   * @param {import('./index.js').ScanOptions=} options
   * @return {import('./index.js').Value[][]}
   */
  select(key = [], options = {}) {
    return this.index().select(key, options);
  }
  /**
   * @param {import('./index.js').Value=} key
   * @param {Pick<import('./index.js').ScanOptions, 'mode'>=} options
   * @return {import('./index.js').IndexIterator}
   */
  iter(key = [], options = {}) {
    return this.index().iter(key, options);
  }
}
function identifier(value) {
  return typeof value === 'string' ? value : unsigned(value);
}
/** Resolves an index within its space and decodes temporary tuples. */
export class Index {
  constructor(bindings, space, nameOrId) {
    this.bindings_ = bindings;
    this.space = space;
    this.nameOrId_ = identifier(nameOrId);
  }
  /** @private */
  resolve_() {
    const space = this.space.resolve_();
    if (typeof this.nameOrId_ !== 'string') {
      return {id: this.nameOrId_, spaceId: space.id};
    }
    const value =
        this.bindings_.call('database', 'indexByName', space, this.nameOrId_);
    if (value == null) {
      throw new RangeError(`index ${this.nameOrId_} does not exist`);
    }
    return value;
  }
  /**
   * @return {number}
   */
  get id() {
    return this.resolve_().id;
  }
  /**
   * @return {bigint}
   */
  get tupleCount() {
    return this.bindings_.call('index', 'tupleCount', this.resolve_());
  }
  /**
   * @return {bigint}
   */
  get memorySize() {
    return this.bindings_.call('index', 'memorySize', this.resolve_());
  }
  /**
   * @param {import('./index.js').Value} key
   * @return {import('./index.js').Value[]|null}
   */
  get(key) {
    return this.row_('get', keyBytes(key));
  }
  /**
   * @param {import('./index.js').Value=} key
   * @return {import('./index.js').Value[]|null}
   */
  min(key = []) {
    return this.row_('min', keyBytes(key));
  }
  /**
   * @param {import('./index.js').Value=} key
   * @return {import('./index.js').Value[]|null}
   */
  max(key = []) {
    return this.row_('max', keyBytes(key));
  }
  /**
   * @param {number} seed
   * @return {import('./index.js').Value[]|null}
   */
  random(seed) {
    return this.row_('random', unsigned(seed));
  }
  /** @private */
  row_(method, arg) {
    return row(
        this.bindings_,
        this.bindings_.call('index', method, this.resolve_(), arg));
  }
  /**
   * @param {import('./index.js').Value=} key
   * @param {{mode?: import('./index.js').IteratorType}=} options
   * @return {bigint}
   */
  count(key = [], {mode = IteratorType.equal} = {}) {
    return this.bindings_.call(
        'index',
        'count',
        this.resolve_(),
        enumValue(IteratorType, mode),
        keyBytes(key));
  }
  /**
   * @param {import('./index.js').Value=} key
   * @param {Pick<import('./index.js').ScanOptions, 'mode'>=} options
   * @return {import('./index.js').IndexIterator}
   */
  iter(
      key = [],
      {mode = Array.isArray(key) && !key.length ? IteratorType.all :
                                                  IteratorType.equal} = {}) {
    return new IndexIterator(
        this.bindings_,
        this.bindings_.iterator(
            'index',
            this.resolve_(),
            enumValue(IteratorType, mode),
            keyBytes(key)));
  }
  /**
   * @param {import('./index.js').Value=} key
   * @param {import('./index.js').ScanOptions=} options
   * @return {import('./index.js').Value[][]}
   */
  select(key = [], {mode, limit = Number.MAX_SAFE_INTEGER, offset = 0} = {}) {
    // Bounds for materializing JS arrays must be exactly representable.
    limit = Number(integer(limit, 0, Number.MAX_SAFE_INTEGER, 'limit'));
    offset = Number(integer(offset, 0, Number.MAX_SAFE_INTEGER, 'offset'));
    return usingResource(this.iter(key, {mode}), (cursor) => {
      const result = [];
      for (let i = 0; i < offset; i++) {
        if (cursor.next().done) {
          return result;
        }
      }
      while (result.length < limit) {
        const item = cursor.next();
        if (item.done) {
          break;
        }
        result.push(item.value);
      }
      return result;
    });
  }
  /**
   * @param {import('./index.js').Value} key
   * @param {import('./index.js').Operations} operations
   * @return {import('./index.js').Value[]|null}
   */
  update(key, operations) {
    return row(
        this.bindings_,
        this.bindings_.call(
            'database',
            'update',
            this.resolve_(),
            keyBytes(key),
            operationsBytes(operations)));
  }
  /**
   * @param {import('./index.js').Value[]} value
   * @param {import('./index.js').Operations} operations
   */
  upsert(value, operations) {
    this.bindings_.call(
        'database',
        'upsert',
        this.resolve_(),
        arrayBytes(value),
        operationsBytes(operations));
  }
  /**
   * @param {import('./index.js').Value} key
   * @return {import('./index.js').Value[]|null}
   */
  delete(key) {
    return row(
        this.bindings_,
        this.bindings_.call(
            'database', 'delete', this.resolve_(), keyBytes(key)));
  }
}

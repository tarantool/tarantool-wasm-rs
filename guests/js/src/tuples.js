import {keyBytes, operationsBytes, unpack} from './codec.js';
import {closeAfterError, disposeHandle, Resource, sameBindings} from './resource.js';

import {unsigned} from './database.js';
import {enumValue, FieldType, Collation} from './types.js';
import {bytes} from './values.js';

/** Owns one retained host tuple; close it explicitly when finished. */
export class Tuple extends Resource {
  constructor(bindings, handle) {
    super(bindings, handle, (h) => bindings.call('boxTuple', 'release', h));
  }
  /**
   * @return {import('./index.js').Tuple}
   */
  copy() {
    const h = this.getHandle_();
    this.bindings_.call('boxTuple', 'retain', h);
    return new Tuple(this.bindings_, h);
  }
  /**
   * @return {number}
   */
  get length() {
    return this.bindings_.call('boxTuple', 'fieldCount', this.getHandle_());
  }
  /**
   * @return {bigint}
   */
  get byteSize() {
    return this.bindings_.call('boxTuple', 'byteSize', this.getHandle_());
  }
  /**
   * @return {Uint8Array}
   */
  toBytes() {
    return bytes(this.bindings_.call('boxTuple', 'toBytes', this.getHandle_()));
  }
  /**
   * @return {import('./index.js').Value[]}
   */
  toArray() {
    return unpack(this.toBytes());
  }
  /**
   * @return {IterableIterator<import('./index.js').Value>}
   */
  [Symbol.iterator]() {
    return this.toArray()[Symbol.iterator]();
  }
  /**
   * @return {import('./index.js').TupleFormat}
   */
  get format() {
    return new TupleFormat(
        this.bindings_,
        this.bindings_.call('boxTuple', 'format', this.getHandle_()));
  }
  /**
   * @param {number|string} indexOrPath
   * @return {import('./index.js').Value}
   */
  field(indexOrPath) {
    const h = this.getHandle_();
    let value;
    if (typeof indexOrPath === 'string') {
      value = this.bindings_.call('boxTuple', 'fieldByPath', h, indexOrPath);
    } else {
      if (!Number.isSafeInteger(indexOrPath)) {
        throw new TypeError('field requires an integer or path');
      }
      const index = indexOrPath < 0 ? this.length + indexOrPath : indexOrPath;
      if (index < 0) {
        throw new RangeError('tuple field does not exist');
      }
      value = this.bindings_.call('boxTuple', 'field', h, unsigned(index));
    }
    if (value == null) {
      throw new RangeError(`tuple field ${indexOrPath} does not exist`);
    }
    return unpack(value);
  }
  /**
   * @return {import('./index.js').TupleCursor}
   */
  iterFields() {
    return new TupleCursor(
        this.bindings_, this.bindings_.iterator('boxTuple', this.getHandle_()));
  }
  /**
   * @param {import('./index.js').Operations} operations
   * @return {import('./index.js').Tuple}
   */
  update(operations) {
    return new Tuple(
        this.bindings_,
        this.bindings_.call(
            'boxTuple',
            'update',
            this.getHandle_(),
            operationsBytes(operations, true)));
  }
  /**
   * @param {import('./index.js').Operations} operations
   * @return {import('./index.js').Tuple}
   */
  upsert(operations) {
    return new Tuple(
        this.bindings_,
        this.bindings_.call(
            'boxTuple',
            'upsert',
            this.getHandle_(),
            operationsBytes(operations, true)));
  }
  /**
   * @param {import('./index.js').TupleFormat} format
   */
  validate(format) {
    sameBindings(this, format);
    this.bindings_.call(
        'boxTuple', 'validate', this.getHandle_(), format.getHandle_());
  }
  /**
   * @param {import('./index.js').Tuple} other
   * @param {import('./index.js').KeyDef} definition
   * @return {number}
   */
  compare(other, definition) {
    sameBindings(this, other, definition);
    return this.bindings_.call(
        'boxTuple',
        'compare',
        definition.getHandle_(),
        this.getHandle_(),
        other.getHandle_());
  }
  /**
   * @param {import('./index.js').Value} key
   * @param {import('./index.js').KeyDef} definition
   * @return {number}
   */
  compareWithKey(key, definition) {
    sameBindings(this, definition);
    return this.bindings_.call(
        'boxTuple',
        'compareWithKey',
        this.getHandle_(),
        keyBytes(key),
        definition.getHandle_());
  }
}
/** Owns a rewindable field cursor; iteration closes it on exhaustion. */
export class TupleCursor extends Resource {
  constructor(bindings, handle) {
    super(bindings, handle, disposeHandle);
  }
  /**
   * @return {this}
   */
  [Symbol.iterator]() {
    return this;
  }
  // read() deliberately keeps an exhausted cursor open for WIT rewind/seek.
  /**
   * @return {IteratorResult<import('./index.js').Value>}
   */
  read() {
    const h = this.getHandle_();
    try {
      const value = this.bindings_.invoke(h.next.bind(h));
      return value == null ? {done: true, value: undefined} :
                             {done: false, value: unpack(value)};
    } catch (error) {
      closeAfterError(this);
      throw error;
    }
  }
  /**
   * @return {IteratorResult<import('./index.js').Value>}
   */
  next() {
    if (this.finished_) {
      return {done: true, value: undefined};
    }
    const item = this.read();
    if (item.done) {
      this.finished_ = true;
      this.close();
    }
    return item;
  }
  /**
   * @return {IteratorResult<import('./index.js').Value>}
   */
  return() {
    this.finished_ = true;
    this.close();
    return {done: true, value: undefined};
  }
  /**
   * @param {unknown} error
   * @return {IteratorResult<import('./index.js').Value>}
   */
  throw(error) {
    closeAfterError(this);
    throw error;
  }
  /**
   * @return {number}
   */
  get position() {
    const h = this.getHandle_();
    try {
      return this.bindings_.invoke(h.position.bind(h));
    } catch (error) {
      closeAfterError(this);
      throw error;
    }
  }
  rewind() {
    const h = this.getHandle_();
    try {
      this.bindings_.invoke(h.rewind.bind(h));
    } catch (error) {
      closeAfterError(this);
      throw error;
    }
  }
  /**
   * @param {number} position
   * @return {import('./index.js').Value}
   */
  seek(position) {
    const h = this.getHandle_();
    try {
      const value = this.bindings_.invoke(h.seek.bind(h), unsigned(position));
      if (value == null) {
        throw new RangeError(`tuple field ${position} does not exist`);
      }
      return unpack(value);
    } catch (error) {
      closeAfterError(this);
      throw error;
    }
  }
}
/** Owns one reference to a host tuple format. */
export class TupleFormat extends Resource {
  constructor(bindings, handle) {
    super(bindings, handle, (h) => bindings.call('tupleFormat', 'release', h));
  }
  /**
   * @return {import('./index.js').TupleFormat}
   */
  copy() {
    const h = this.getHandle_();
    this.bindings_.call('tupleFormat', 'retain', h);
    return new TupleFormat(this.bindings_, h);
  }
}
/** Owns a host key definition used for validation and comparison. */
export class KeyDef extends Resource {
  constructor(bindings, handle) {
    super(bindings, handle, (h) => bindings.call('keyDef', 'release', h));
  }
  /**
   * @return {import('./index.js').KeyDef}
   */
  copy() {
    return new KeyDef(
        this.bindings_,
        this.bindings_.call('keyDef', 'duplicate', this.getHandle_()));
  }
  /**
   * @param {import('./index.js').KeyDef} other
   * @return {import('./index.js').KeyDef}
   */
  merge(other) {
    sameBindings(this, other);
    return new KeyDef(
        this.bindings_,
        this.bindings_.call(
            'keyDef', 'merge', this.getHandle_(), other.getHandle_()));
  }
  /**
   * @return {number}
   */
  get partCount() {
    return this.bindings_.call('keyDef', 'partCount', this.getHandle_());
  }
  /**
   * @return {import('./_tarantool.js').KeyPart[]}
   */
  get parts() {
    return this.bindings_.call('keyDef', 'dumpParts', this.getHandle_());
  }
  /**
   * @param {import('./index.js').Value} key
   * @param {{full?: boolean}=} options
   */
  validateKey(key, {full = false} = {}) {
    this.bindings_.call(
        'keyDef',
        full ? 'validateFullKey' : 'validateKey',
        this.getHandle_(),
        keyBytes(key));
  }
  /**
   * @param {import('./index.js').Tuple} tuple
   * @return {import('./index.js').Value[]}
   */
  extractKey(tuple) {
    sameBindings(this, tuple);
    return unpack(this.bindings_.call(
        'keyDef', 'extractKey', this.getHandle_(), tuple.getHandle_()));
  }
  /**
   * @param {import('./index.js').Tuple} tuple
   */
  validateTuple(tuple) {
    sameBindings(this, tuple);
    this.bindings_.call(
        'keyDef', 'validateTuple', this.getHandle_(), tuple.getHandle_());
  }
}
export function keyPart(part) {
  const flags = part.flags ?? {};
  if (typeof flags !== 'object' || flags === null ||
      Object.keys(flags).some(
          (k) => !['isNullable', 'excludeNull'].includes(k)) ||
      Object.values(flags).some((v) => typeof v !== 'boolean')) {
    throw new TypeError('invalid key-part flags');
  }
  if (part.path != null && typeof part.path !== 'string') {
    throw new TypeError('key-part path requires a string');
  }
  return {
    fieldNo: unsigned(part.fieldNo),
    fieldType: enumValue(FieldType, part.fieldType),
    collation: part.collation == null ? undefined :
                                        enumValue(Collation, part.collation),
    path: part.path ?? undefined,
    flags: {isNullable: false, excludeNull: false, ...flags},
  };
}

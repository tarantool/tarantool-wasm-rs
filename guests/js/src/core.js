import {Bindings} from './bindings.js';
import {arrayBytes} from './codec.js';
import {Space} from './database.js';
import {sameBindings} from './resource.js';
import {Errors, Log, MessagePack, Metrics, Sequence, Sessions} from './services.js';
import {Transactions} from './transaction.js';
import {KeyDef, keyPart, Tuple, TupleFormat} from './tuples.js';

/** Provides the database API over lazy, injected WIT bindings. */
export class Tarantool {
  constructor({bindings} = {}) {
    this.bindings_ = new Bindings(bindings);
    this.txn = new Transactions(this.bindings_);
    this.session = new Sessions(this.bindings_);
    this.error = new Errors(this.bindings_);
    this.log = new Log(this.bindings_);
    this.metrics = new Metrics(this.bindings_);
    this.msgpack = new MessagePack(this.bindings_);
  }
  /**
   * @return {number}
   */
  get schemaVersion() {
    return this.bindings_.call('database', 'schemaVersion');
  }
  /**
   * @param {string|number} nameOrId
   * @return {import('./index.js').Space}
   */
  space(nameOrId) {
    return new Space(this.bindings_, nameOrId);
  }
  /**
   * @param {number} id
   * @return {import('./index.js').Sequence}
   */
  sequence(id) {
    return new Sequence(this.bindings_, id);
  }
  /**
   * @param {import('./index.js').Value[]} value
   * @return {import('./index.js').Tuple}
   */
  tuple(value) {
    return new Tuple(
        this.bindings_,
        this.bindings_.call('boxTuple', 'new', arrayBytes(value)));
  }
  /**
   * @param {import('./index.js').KeyPart[]} parts
   * @return {import('./index.js').KeyDef}
   */
  keyDef(parts) {
    return new KeyDef(
        this.bindings_,
        this.bindings_.call('keyDef', 'new', parts.map(keyPart)));
  }
  /**
   * @param {import('./index.js').KeyDef[]=} definitions
   * @return {import('./index.js').TupleFormat}
   */
  tupleFormat(definitions) {
    if (definitions === undefined) {
      return new TupleFormat(
          this.bindings_, this.bindings_.call('tupleFormat', 'default'));
    }
    sameBindings(this, ...definitions);
    return new TupleFormat(
        this.bindings_,
        this.bindings_.call(
            'tupleFormat', 'new', definitions.map((d) => d.getHandle_())));
  }
  /**
   * @template T
   * @param {function(): T} fn
   * @param {import('./index.js').TransactionOptions=} options
   * @return {T}
   */
  transaction(fn, options) {
    return this.txn.run(fn, options);
  }
}

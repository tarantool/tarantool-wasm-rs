import {pack, unpack} from './codec.js';
import {bytes, integer, isMap, isPlainObject} from './values.js';

import {unsigned} from './database.js';
import {TarantoolError} from './errors.js';
import {enumValue, LogLevel} from './types.js';

/** Addresses an existing host sequence by ID. */
export class Sequence {
  constructor(bindings, id) {
    this.bindings_ = bindings;
    this.id = unsigned(id);
  }
  /** @private */
  call_(method, ...args) {
    return this.bindings_.call('sequence', method, {id: this.id}, ...args);
  }
  /**
   * @return {bigint}
   */
  next() {
    return this.call_('next');
  }
  /**
   * @return {bigint}
   */
  get current() {
    return this.call_('current');
  }
  /**
   * @param {number|bigint} value
   */
  set(value) {
    this.call_(
        'set', integer(value, -(2n ** 63n), 2n ** 63n - 1n, 'sequence value'));
  }
  reset() {
    this.call_('reset');
  }
}
function map(value) {
  if (!isMap(value) && !isPlainObject(value)) {
    throw new TypeError('iproto header/body must be a Map or plain object');
  }
  return pack(value);
}
/** Addresses an existing session for MessagePack iproto messages. */
export class Session {
  constructor(bindings, id) {
    this.bindings_ = bindings;
    this.id = integer(id, 0, 2n ** 64n - 1n, 'session id');
  }
  /**
   * @param {Map<import('./index.js').Value, import('./index.js').Value>|{[key:
   *     string]: import('./index.js').Value}} header
   * @param {Map<import('./index.js').Value, import('./index.js').Value>|{[key:
   *     string]: import('./index.js').Value}|null=} body
   */
  send(header, body) {
    this.bindings_.call(
        'session',
        'iprotoSend',
        {id: this.id},
        map(header),
        body == null ? undefined : map(body));
  }
}
/** Provides the current session and Lua broadcast operations. */
export class Sessions {
  constructor(bindings) {
    this.bindings_ = bindings;
  }
  /**
   * @return {import('./index.js').Session}
   */
  current() {
    return new Session(
        this.bindings_, this.bindings_.call('session', 'current').id);
  }
  /**
   * @param {number|bigint} id
   * @return {import('./index.js').Session}
   */
  get(id) {
    return new Session(this.bindings_, id);
  }
  /**
   * @param {string} key
   * @param {string} expression
   */
  broadcastLua(key, expression) {
    this.bindings_.call('session', 'broadcast', key, expression);
  }
}
/** Reads and updates the host diagnostic state. */
export class Errors {
  constructor(bindings) {
    this.bindings_ = bindings;
  }
  new(code, message, {file, line} = {}) {
    const value = file == null && line == null ?
        this.bindings_.call('error', 'new', unsigned(code), message) :
        this.bindings_.call(
            'error',
            'newWithLocation',
            unsigned(code),
            message,
            file ?? undefined,
            line == null ? undefined : unsigned(line));
    return new TarantoolError(value);
  }
  /** @private */
  record_(error) {
    if (!(error instanceof TarantoolError)) {
      throw new TypeError('expected TarantoolError');
    }
    return {
      code: unsigned(error.code),
      message: error.message,
      errorType: error.errorType,
      file: error.file ?? undefined,
      line: error.line ?? undefined,
    };
  }
  /**
   * @param {import('./index.js').TarantoolError} error
   */
  set(error) {
    this.bindings_.call('error', 'set', this.record_(error));
  }
  /**
   * @return {import('./index.js').TarantoolError|null}
   */
  last() {
    const value = this.bindings_.call('error', 'last');
    return value == null ? null : new TarantoolError(value);
  }
  clear() {
    this.bindings_.call('error', 'clear');
  }
  /**
   * @param {import('./index.js').TarantoolError} error
   * @return {string}
   */
  format(error) {
    return this.bindings_.call('error', 'toString', this.record_(error));
  }
}
/** Writes messages through the Tarantool logger. */
export class Log {
  constructor(bindings) {
    this.bindings_ = bindings;
  }
  /**
   * @param {import('./index.js').LogLevel} level
   * @param {unknown} message
   * @param {{context: ({file: string, line: number}|undefined)}=} options
   */
  write(level, message, {context} = {}) {
    this.bindings_.call(
        'log',
        'write',
        enumValue(LogLevel, level),
        String(message),
        context == null ? undefined :
                          {file: context.file, line: unsigned(context.line)});
  }
}
/** Reads host metrics exposed by the WIT contract. */
export class Metrics {
  constructor(bindings) {
    this.bindings_ = bindings;
  }
  /**
   * @return {number}
   */
  uptime() {
    return this.bindings_.call('metrics', 'uptime');
  }
}
/** Provides the value codec and explicit host JSON conversion helpers. */
export class MessagePack {
  constructor(bindings) {
    this.bindings_ = bindings;
  }
  /**
   * @param {import('./index.js').Value} value
   * @return {Uint8Array}
   */
  pack(value) {
    return pack(value);
  }
  /**
   * @param {Uint8Array} data
   * @param {{rawExtensions?: boolean}=} options
   * @return {import('./index.js').Value}
   */
  unpack(data, options) {
    return unpack(data, options);
  }
  /**
   * @param {string} value
   * @return {Uint8Array}
   */
  fromJson(value) {
    return bytes(this.bindings_.call('msgpack', 'fromJson', value));
  }
  /**
   * @param {Uint8Array} data
   * @return {string}
   */
  toJson(data) {
    return this.bindings_.call('msgpack', 'toJson', bytes(data));
  }
}

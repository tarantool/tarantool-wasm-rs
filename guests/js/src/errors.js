/** Preserves the error fields supplied by the Tarantool WIT contract. */
export class TarantoolError extends Error {
  constructor({code, message, errorType = '', file, line}, options) {
    super(message, options);
    this.name = 'TarantoolError';
    Object.assign(
        this, {code, errorType, file: file ?? null, line: line ?? null});
  }
}
/** Reports callback lookup or invocation failures. */
export class LuaCallbackError extends Error {
  constructor(message, options) {
    super(message, options);
    this.name = 'LuaCallbackError';
  }
}
/** Reports access to an explicitly released resource. */
export class ClosedResourceError extends Error {
  constructor(message) {
    super(message);
    this.name = 'ClosedResourceError';
  }
}

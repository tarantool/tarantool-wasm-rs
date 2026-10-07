export function integer(value, min, max, name = 'integer') {
  if (typeof value !== 'bigint' &&
      (typeof value !== 'number' || !Number.isSafeInteger(value))) {
    throw new TypeError(`${name} must be a safe integer or bigint`);
  }
  const result = BigInt(value);
  if (result < BigInt(min) || result > BigInt(max)) {
    throw new RangeError(`${name} is out of range`);
  }
  return result;
}
// ComponentizeJS lifts WIT values in a separate realm. Check intrinsic brands,
// not instanceof, and normalize public byte results into the caller's realm.
const typedArrayName =
    Object
        .getOwnPropertyDescriptor(
            Object.getPrototypeOf(Uint8Array.prototype), Symbol.toStringTag)
        .get;
export function isBytes(value) {
  return ArrayBuffer.isView(value) &&
      typedArrayName.call(value) === 'Uint8Array';
}
export function bytes(value) {
  if (!isBytes(value)) {
    throw new TypeError('expected Uint8Array');
  }
  return value instanceof Uint8Array ? value : new Uint8Array(value);
}
export function isMap(value) {
  if (!value || typeof value !== 'object') {
    return false;
  }
  try {
    Map.prototype.has.call(value, null);
    return true;
  } catch {
    return false;
  }
}
export function isPlainObject(value) {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const proto = Object.getPrototypeOf(value);
  return proto === null ||
      (Object.getPrototypeOf(proto) === null &&
       typeof proto.constructor === 'function' &&
       Function.prototype.toString.call(proto.constructor) ===
           Function.prototype.toString.call(Object));
}
/** Preserves an unknown MessagePack extension type and payload. */
export class Extension {
  constructor(type, data) {
    this.type = Number(integer(type, -128, 127, 'extension type'));
    this.data = new Uint8Array(bytes(data));
  }
}
/** Represents a Tarantool UUID as exactly sixteen bytes. */
export class UUID {
  constructor(value) {
    if (typeof value === 'string') {
      if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
               .test(value)) {
        throw new TypeError('invalid UUID');
      }
      this.bytes = Uint8Array.from(
          value.replaceAll('-', '').match(/../g), (x) => parseInt(x, 16));
    } else {
      if (bytes(value).length !== 16) {
        throw new RangeError('UUID must contain 16 bytes');
      }
      this.bytes = new Uint8Array(value);
    }
  }
  /**
   * @return {string}
   */
  toString() {
    const hex =
        Array.from(this.bytes, (x) => x.toString(16).padStart(2, '0')).join('');
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${
        hex.slice(16, 20)}-${hex.slice(20)}`;
  }
}
/** Preserves decimal digits and scale without conversion to a number. */
export class Decimal {
  constructor(value) {
    if (typeof value !== 'string') {
      throw new TypeError('Decimal requires a string');
    }
    const m = /^([+-]?)(\d+)(?:\.(\d*))?(?:[eE]([+-]?\d+))?$/.exec(value);
    if (!m) {
      throw new TypeError('invalid finite Decimal');
    }
    this.negative = m[1] === '-';
    this.digits = (m[2] + (m[3] ?? '')).replace(/^0+(?=\d)/, '');
    this.scale = Number(integer(
        BigInt((m[3] ?? '').length) - BigInt(m[4] ?? 0),
        -(2n ** 31n),
        2n ** 31n - 1n,
        'decimal scale'));
  }
  /**
   * @return {string}
   */
  toString() {
    const prefix = this.negative ? '-' : '';
    if (Math.abs(this.scale) > 1000) {
      return `${prefix}${this.digits}e${- this.scale}`;
    }
    if (this.scale <= 0) {
      return prefix + this.digits + '0'.repeat(-this.scale);
    }
    const digits = this.digits.padStart(this.scale + 1, '0');
    return prefix + digits.slice(0, -this.scale) + '.' +
        digits.slice(-this.scale);
  }
  valueOf() {
    throw new TypeError('Decimal cannot be converted implicitly to number');
  }
}
/** Preserves seconds, nanoseconds and Tarantool timezone fields. */
export class Datetime {
  constructor(seconds, {nanoseconds = 0, tzoffset = 0, tzindex = 0} = {}) {
    this.seconds = integer(
        seconds, -185604722870400n, 185480451417600n, 'datetime seconds');
    this.nanoseconds =
        Number(integer(nanoseconds, 0, 1999999999, 'nanoseconds'));
    this.tzoffset = Number(integer(tzoffset, -720, 840, 'tzoffset'));
    this.tzindex = Number(integer(tzindex, 0, 1024, 'tzindex'));
  }
}
/**
 * Tarantool interval adjustment modes.
 * @enum {number}
 */
export const IntervalAdjust = Object.freeze({excess: 0, limit: 1, snap: 2});
export const intervalFields = [
  'years',
  'months',
  'weeks',
  'days',
  'hours',
  'minutes',
  'seconds',
  'nanoseconds',
  'adjust',
];
/** Represents all Tarantool interval fields and adjustment mode. */
export class Interval {
  constructor(fields = {}) {
    for (const name of Object.keys(fields)) {
      if (!intervalFields.includes(name)) {
        throw new TypeError(`unknown interval field ${name}`);
      }
    }
    for (const name of intervalFields) {
      const limit = ['years', 'months', 'weeks', 'nanoseconds'].includes(name) ?
          2n ** 31n :
          2n ** 53n;
      const value =
          fields[name] ?? (name === 'adjust' ? IntervalAdjust.limit : 0);
      this[name] = Number(integer(
          value,
          name === 'adjust' ? 0 : -limit,
          name === 'adjust' ? 2 : limit - (limit === 2n ** 31n ? 1n : 0n),
          name));
    }
  }
}
function extras(value, reserved) {
  if (!isMap(value)) {
    throw new TypeError('error extra attributes require a Map');
  }
  for (const key of value.keys()) {
    const n = integer(key, 0, 2n ** 64n - 1n, 'extra error key');
    if (reserved.includes(Number(n))) {
      throw new RangeError('extra error key overrides a known field');
    }
  }
  return new Map(value);
}
/** Represents one Tarantool MessagePack error frame. */
export class ErrorFrame {
  constructor({
    code,
    message,
    errorType = 'ClientError',
    file = '',
    line = 0,
    errno = 0,
    fields = {},
    extra = new Map(),
  }) {
    for (const [name, value] of Object.entries({message, errorType, file})) {
      if (typeof value !== 'string') {
        throw new TypeError(`${name} must be a string`);
      }
    }
    for (const [name, value] of Object.entries({code, line, errno})) {
      this[name] = Number(integer(value, 0, 2n ** 32n - 1n, name));
    }
    if (!isPlainObject(fields)) {
      throw new TypeError('error fields must be a plain object');
    }
    Object.assign(this, {
      message,
      errorType,
      file,
      fields: {...fields},
      extra: extras(extra, [0, 1, 2, 3, 4, 5, 6]),
    });
  }
}
/** Preserves a Tarantool error stack and unknown numeric attributes. */
export class ErrorValue {
  constructor(stack, {extra = new Map()} = {}) {
    if (!Array.isArray(stack) || !stack.length ||
        !stack.every((x) => x instanceof ErrorFrame)) {
      throw new TypeError('error stack must contain ErrorFrame values');
    }
    this.stack = stack.slice();
    this.extra = extras(extra, [0]);
  }
}

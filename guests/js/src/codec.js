import {bytes, Datetime, Decimal, ErrorFrame, ErrorValue, Extension, integer, Interval, intervalFields, isBytes, isMap, isPlainObject, UUID} from './values.js';

export {Datetime, Decimal, ErrorFrame, ErrorValue, Extension, Interval, IntervalAdjust, UUID} from './values.js';

function concatenate(parts) {
  const result = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }
  return result;
}
function sized(tag, value, width) {
  const out = new Uint8Array(width + 1);
  const view = new DataView(out.buffer);
  out[0] = tag;
  if (width === 1) {
    view.setUint8(1, value);
  } else if (width === 2) {
    view.setUint16(1, value);
  } else if (width === 4) {
    view.setUint32(1, value);
  } else {
    view.setBigUint64(1, BigInt.asUintN(64, BigInt(value)));
  }
  return out;
}
function lengthHeader(n, fix, cap, tags) {
  if (n > 0xffffffff) {
    throw new RangeError('MessagePack container too large');
  }
  if (n < cap) {
    return Uint8Array.of(fix | n);
  }
  if (tags[0] && n < 256) {
    return sized(tags[0], n, 1);
  }
  return n < 65536 ? sized(tags[1], n, 2) : sized(tags[2], n, 4);
}
function encodeInteger(n) {
  integer(n, -(2n ** 63n), 2n ** 64n - 1n);
  if (n >= 0n) {
    if (n < 128n) {
      return Uint8Array.of(Number(n));
    }
    if (n < 256n) {
      return sized(0xcc, Number(n), 1);
    }
    if (n < 65536n) {
      return sized(0xcd, Number(n), 2);
    }
    if (n < 2n ** 32n) {
      return sized(0xce, Number(n), 4);
    }
    return sized(0xcf, n, 8);
  }
  if (n >= -32n) {
    return Uint8Array.of(Number(n) & 255);
  }
  if (n >= -128n) {
    return sized(0xd0, Number(n), 1);
  }
  if (n >= -32768n) {
    return sized(0xd1, Number(n), 2);
  }
  if (n >= -(2n ** 31n)) {
    return sized(0xd2, Number(n), 4);
  }
  return sized(0xd3, n, 8);
}
function extension(value, encode) {
  if (value instanceof Extension) {
    return value;
  }
  if (value instanceof UUID) {
    return new Extension(2, value.bytes);
  }
  if (value instanceof Decimal) {
    let nibbles = value.digits + (value.negative ? 'd' : 'c');
    if (nibbles.length % 2) {
      nibbles = '0' + nibbles;
    }
    const bcd = Uint8Array.from(nibbles.match(/../g), (x) => parseInt(x, 16));
    return new Extension(
        1, concatenate([encodeInteger(BigInt(value.scale)), bcd]));
  }
  if (value instanceof Datetime) {
    const out = new Uint8Array(
        value.nanoseconds || value.tzoffset || value.tzindex ? 16 : 8);
    const view = new DataView(out.buffer);
    view.setBigInt64(0, value.seconds, true);
    if (out.length === 16) {
      view.setInt32(8, value.nanoseconds, true);
      view.setInt16(12, value.tzoffset, true);
      view.setInt16(14, value.tzindex, true);
    }
    return new Extension(4, out);
  }
  if (value instanceof Interval) {
    const pairs = intervalFields.flatMap(
        (name, i) => value[name] ?
            [Uint8Array.of(i), encodeInteger(BigInt(value[name]))] :
            []);
    return new Extension(
        6, concatenate([Uint8Array.of(pairs.length / 2), ...pairs]));
  }
  if (value instanceof ErrorValue) {
    const frames = value.stack.map((f) => new Map([
                                     ...f.extra,
                                     [0, f.errorType],
                                     [1, f.file],
                                     [2, f.line],
                                     [3, f.message],
                                     [4, f.errno],
                                     [5, f.code],
                                     [6, f.fields],
                                   ]));
    return new Extension(3, encode(new Map([...value.extra, [0, frames]])));
  }
}
/**
 * Encodes JavaScript values directly, preserving Tarantool extension payloads.
 * @param {import('./codec.js').Value} value
 * @return {!Uint8Array}
 */
export function pack(value) {
  const seen = new Set();
  const encoder = new TextEncoder();
  function encode(v, depth = 0) {
    if (depth > 128) {
      throw new RangeError('MessagePack nesting exceeds 128');
    }
    if (v === null) {
      return Uint8Array.of(0xc0);
    }
    if (v === undefined) {
      throw new TypeError(
          'undefined is not a MessagePack value; use null explicitly');
    }
    if (typeof v === 'boolean') {
      return Uint8Array.of(v ? 0xc3 : 0xc2);
    }
    if (typeof v === 'bigint') {
      return encodeInteger(v);
    }
    if (typeof v === 'number') {
      if (Number.isInteger(v) && !Object.is(v, -0)) {
        if (!Number.isSafeInteger(v)) {
          throw new RangeError('unsafe integer number; use bigint');
        }
        return encodeInteger(BigInt(v));
      }
      const out = new Uint8Array(9);
      out[0] = 0xcb;
      new DataView(out.buffer).setFloat64(1, v);
      return out;
    }
    if (typeof v === 'string') {
      if (!v.isWellFormed()) {
        throw new TypeError('MessagePack strings require valid Unicode');
      }
      const data = encoder.encode(v);
      return concatenate(
          [lengthHeader(data.length, 0xa0, 32, [0xd9, 0xda, 0xdb]), data]);
    }
    if (typeof v !== 'object') {
      throw new TypeError(`unsupported MessagePack type ${typeof v}`);
    }
    if (seen.has(v)) {
      throw new TypeError('cyclic MessagePack value');
    }
    seen.add(v);
    try {
      const ext = extension(v, (x) => encode(x, depth + 1));
      if (ext) {
        const n = ext.data.length;
        const fix = [1, 2, 4, 8, 16].indexOf(n);
        const header = fix < 0 ? lengthHeader(n, 0, 0, [0xc7, 0xc8, 0xc9]) :
                                 Uint8Array.of(0xd4 + fix);
        return concatenate([header, Uint8Array.of(ext.type & 255), ext.data]);
      }
      if (isBytes(v)) {
        return concatenate(
            [lengthHeader(v.length, 0, 0, [0xc4, 0xc5, 0xc6]), v]);
      }
      if (Array.isArray(v)) {
        const parts = [lengthHeader(v.length, 0x90, 16, [0, 0xdc, 0xdd])];
        for (let i = 0; i < v.length; i++) {
          parts.push(encode(v[i], depth + 1));
        }
        return concatenate(parts);
      }
      const map = isMap(v);
      if (!map && !isPlainObject(v)) {
        throw new TypeError('unsupported MessagePack object');
      }
      if (!map && Object.getOwnPropertySymbols(v).length) {
        throw new TypeError('symbol map keys are unsupported');
      }
      const pairs =
          map ? [...Map.prototype.entries.call(v)] : Object.entries(v);
      return concatenate([
        lengthHeader(pairs.length, 0x80, 16, [0, 0xde, 0xdf]),
        ...pairs.flatMap(
            ([k, x]) => [encode(k, depth + 1), encode(x, depth + 1)]),
      ]);
    } finally {
      seen.delete(v);
    }
  }
  return encode(value);
}
function exactInteger(n) {
  return n >= BigInt(Number.MIN_SAFE_INTEGER) &&
          n <= BigInt(Number.MAX_SAFE_INTEGER) ?
      Number(n) :
      n;
}
/** Decodes one bounded MessagePack buffer without losing integer precision. */
class Reader {
  constructor(data, rawExtensions) {
    this.data = bytes(data);
    this.view = new DataView(
        this.data.buffer, this.data.byteOffset, this.data.byteLength);
    this.offset = 0;
    this.rawExtensions = rawExtensions;
  }
  take(n) {
    if (n > this.data.length - this.offset) {
      throw new RangeError('truncated MessagePack');
    }
    const out =
        new Uint8Array(this.data.subarray(this.offset, this.offset + n));
    this.offset += n;
    return out;
  }
  number(width, signed = false) {
    if (width > this.data.length - this.offset) {
      throw new RangeError('truncated MessagePack');
    }
    const name = `get${signed ? 'Int' : 'Uint'}${width * 8}`;
    const n = width === 8 ?
        exactInteger(
            this.view[signed ? 'getBigInt64' : 'getBigUint64'](this.offset)) :
        this.view[name](this.offset);
    this.offset += width;
    return n;
  }
  read(depth = 0) {
    if (depth > 128) {
      throw new RangeError('MessagePack nesting exceeds 128');
    }
    const tag = this.number(1);
    if (tag < 128) {
      return tag;
    }
    if (tag >= 224) {
      return tag - 256;
    }
    if ((tag & 0xe0) === 0xa0) {
      return this.string(tag & 31);
    }
    if ((tag & 0xf0) === 0x90) {
      return this.array(tag & 15, depth);
    }
    if ((tag & 0xf0) === 0x80) {
      return this.map(tag & 15, depth);
    }
    if (tag === 0xc0) {
      return null;
    }
    if (tag === 0xc2 || tag === 0xc3) {
      return tag === 0xc3;
    }
    if (tag >= 0xcc && tag <= 0xcf) {
      return this.number(2 ** (tag - 0xcc));
    }
    if (tag >= 0xd0 && tag <= 0xd3) {
      return this.number(2 ** (tag - 0xd0), true);
    }
    if (tag === 0xca || tag === 0xcb) {
      const width = tag === 0xca ? 4 : 8;
      const data = this.take(width);
      return new DataView(
          data.buffer)[width === 4 ? 'getFloat32' : 'getFloat64'](0);
    }
    if (tag >= 0xc4 && tag <= 0xc6) {
      return this.take(this.number(2 ** (tag - 0xc4)));
    }
    if (tag >= 0xd9 && tag <= 0xdb) {
      return this.string(this.number(2 ** (tag - 0xd9)));
    }
    if (tag === 0xdc || tag === 0xdd) {
      return this.array(this.number(tag === 0xdc ? 2 : 4), depth);
    }
    if (tag === 0xde || tag === 0xdf) {
      return this.map(this.number(tag === 0xde ? 2 : 4), depth);
    }
    if ((tag >= 0xd4 && tag <= 0xd8) || (tag >= 0xc7 && tag <= 0xc9)) {
      const n =
          tag >= 0xd4 ? 2 ** (tag - 0xd4) : this.number(2 ** (tag - 0xc7));
      const type = this.number(1, true);
      const data = this.take(n);
      return this.rawExtensions ? new Extension(type, data) :
                                  decodeExtension(type, data, depth);
    }
    throw new TypeError(`invalid MessagePack tag ${tag}`);
  }
  string(n) {
    return new TextDecoder('utf-8', {fatal: true, ignoreBOM: true})
        .decode(this.take(n));
  }
  array(n, depth) {
    if (n > this.data.length - this.offset) {
      throw new RangeError('truncated MessagePack array');
    }
    const out = [];
    for (let i = 0; i < n; i++) {
      out.push(this.read(depth + 1));
    }
    return out;
  }
  map(n, depth) {
    if (n > (this.data.length - this.offset) / 2) {
      throw new RangeError('truncated MessagePack map');
    }
    const out = new Map();
    for (let i = 0; i < n; i++) {
      const key = this.read(depth + 1);
      if (out.has(key)) {
        throw new TypeError('duplicate MessagePack map key');
      }
      out.set(key, this.read(depth + 1));
    }
    if ([...out.keys()].every((k) => typeof k === 'string')) {
      return Object.fromEntries(out);
    }
    return out;
  }
}
function decodeExtension(type, data, depth) {
  if (type === 2) {
    return new UUID(data);
  }
  if (type === 1) {
    const reader = new Reader(data, false);
    const scale = reader.read(depth + 1);
    integer(scale, -(2n ** 31n), 2n ** 31n - 1n, 'decimal scale');
    const bcd = reader.take(data.length - reader.offset);
    const nibbles = Array.from(bcd, (b) => [(b >> 4), b & 15]).flat();
    const sign = nibbles.pop();
    if (![10, 11, 12, 13, 14, 15].includes(sign) || !nibbles.length ||
        nibbles.some((n) => n > 9)) {
      throw new TypeError('invalid decimal BCD');
    }
    return new Decimal(`${sign === 11 || sign === 13 ? '-' : ''}${
        nibbles.join('')}e${- Number(scale)}`);
  }
  if (type === 4) {
    if (data.length !== 8 && data.length !== 16) {
      throw new RangeError('datetime payload requires 8 or 16 bytes');
    }
    const v = new DataView(data.buffer);
    return new Datetime(v.getBigInt64(0, true), data.length === 8 ? {} : {
      nanoseconds: v.getInt32(8, true),
      tzoffset: v.getInt16(12, true),
      tzindex: v.getInt16(14, true),
    });
  }
  if (type === 6) {
    const reader = new Reader(data, false);
    const n = reader.number(1);
    const seen = new Set();
    const fields = {adjust: 0};
    if (n > 9) {
      throw new RangeError('invalid interval field count');
    }
    for (let i = 0; i < n; i++) {
      const id = reader.number(1);
      if (id > 8 || seen.has(id)) {
        throw new TypeError('invalid or duplicate interval field');
      }
      seen.add(id);
      fields[intervalFields[id]] = reader.read(depth + 1);
    }
    if (reader.offset !== data.length) {
      throw new TypeError('trailing interval bytes');
    }
    return new Interval(fields);
  }
  if (type === 3) {
    const reader = new Reader(data, false);
    const value = reader.read(depth + 1);
    if (reader.offset !== data.length || !(value instanceof Map) ||
        !Array.isArray(value.get(0))) {
      throw new TypeError('invalid error stack');
    }
    const stack = value.get(0).map((f) => {
      if (!(f instanceof Map) || ![0, 1, 3].every((k) => f.has(k))) {
        throw new TypeError('invalid error frame');
      }
      return new ErrorFrame({
        code: f.get(5) ?? 0,
        message: f.get(3),
        errorType: f.get(0),
        file: f.get(1),
        line: f.get(2) ?? 0,
        errno: f.get(4) ?? 0,
        fields: f.get(6) ?? {},
        extra:
            new Map([...f].filter(([k]) => ![0, 1, 2, 3, 4, 5, 6].includes(k))),
      });
    });
    return new ErrorValue(
        stack, {extra: new Map([...value].filter(([k]) => k !== 0))});
  }
  return new Extension(type, data);
}
/**
 * Decodes exactly one MessagePack value; rejects trailing or malformed bytes.
 * @param {!Uint8Array} data
 * @param {{rawExtensions: boolean}=} options
 * @return {import('./codec.js').Value}
 */
export function unpack(data, {rawExtensions = false} = {}) {
  const reader = new Reader(data, rawExtensions);
  const value = reader.read();
  if (reader.offset !== data.length) {
    throw new TypeError('trailing MessagePack bytes');
  }
  return value;
}
export function arrayBytes(value) {
  if (!Array.isArray(value)) {
    throw new TypeError('tuple requires an array');
  }
  return pack(value);
}
export function keyBytes(value) {
  return pack(Array.isArray(value) ? value : [value]);
}
export function operationsBytes(value, tupleApi = false) {
  if (!Array.isArray(value)) {
    throw new TypeError('operations require an array');
  }
  return pack(value.map((op) => {
    if (!Array.isArray(op) || op.length < 2 || typeof op[0] !== 'string') {
      throw new TypeError('operation requires operator and field');
    }
    const item = op.slice();
    if (typeof item[1] === 'number') {
      if (!Number.isSafeInteger(item[1])) {
        throw new TypeError('operation field must be an integer');
      }
      if (tupleApi && item[1] >= 0) {
        item[1]++;
      }
    } else if (typeof item[1] !== 'string') {
      throw new TypeError('operation field must be number or path');
    }
    return item;
  }));
}

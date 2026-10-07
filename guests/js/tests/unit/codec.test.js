import assert from 'node:assert/strict';
import {test} from 'node:test';
import {runInNewContext} from 'node:vm';

import {Datetime, Decimal, ErrorFrame, ErrorValue, Extension, Interval, pack, unpack, UUID} from '../../src/codec.js';

test('MessagePack boundary integers and all container widths', () => {
  for (const n
           of [-(2n ** 63n),
               -2147483649n,
               -32769,
               -129,
               -33,
               -32,
               0,
               127,
               128,
               255,
               256,
               65535,
               65536,
               2n ** 32n,
               2n ** 53n,
               2n ** 64n - 1n]) {
    const decoded = unpack(pack(n));
    assert.equal(BigInt(decoded), BigInt(n));
    assert.equal(
        typeof decoded,
        BigInt(n) < BigInt(Number.MIN_SAFE_INTEGER) ||
                BigInt(n) > BigInt(Number.MAX_SAFE_INTEGER) ?
            'bigint' :
            'number');
  }
  for (const n of [0, 1, 31, 32, 255, 256, 65535, 65536]) {
    const string = 'x'.repeat(n);
    assert.equal(unpack(pack(string)), string);
    const binary = new Uint8Array(n).fill(255);
    assert.deepEqual(unpack(pack(binary)), binary);
  }
  for (const n of [0, 15, 16, 65536]) {
    assert.deepEqual(unpack(pack(Array(n).fill(null))), Array(n).fill(null));
  }
  for (const n of [15, 16, 65536]) {
    const value = new Map(Array.from({length: n}, (_, i) => [i, null]));
    assert.deepEqual(unpack(pack(value)), value);
  }
  for (const n of [-0, 1.5, Infinity, -Infinity, NaN]) {
    assert.ok(Object.is(unpack(pack(n)), n));
  }
  assert.equal(unpack(Uint8Array.of(0xca, 0x3f, 0xc0, 0, 0)), 1.5);
});
test('maps preserve non-string keys and prototype-looking string keys', () => {
  const value = new Map([
    [1, null],
    ['data', new Uint8Array([0, 255])],
    [true, [1, 'ёж😀']],
    [2n ** 63n, false],
  ]);
  assert.deepEqual(unpack(pack(value)), value);
  const obj =
      Object.fromEntries([['__proto__', {polluted: true}], ['constructor', 3]]);
  assert.deepEqual(unpack(pack(obj)), obj);
  assert.equal({}.polluted, undefined);
  assert.equal(unpack(pack('\ufefftext')), '\ufefftext');
  assert.deepEqual(
      unpack(pack({'\ufeffkey': 1, 'key': 2})), {'\ufeffkey': 1, 'key': 2});
});
test('reject lossy, malformed, cyclic and incomplete input', () => {
  for (const value
           of [undefined,
               [undefined],
               Array(1),
               {x: undefined},
               2 ** 53,
               2n ** 64n,
               -(2n ** 63n) - 1n,
               new Date(),
               Symbol(),
               () => 0,
               '\ud800']) {
    assert.throws(() => pack(value));
  }
  const cycle = [];
  cycle.push(cycle);
  assert.throws(() => pack(cycle), /cyclic/);
  for (const data
           of [[0xc1],
               [0x92, 1],
               [0xcf, 1],
               [0xa1, 0xff],
               [0, 0],
               [0x82, 1, 0, 1, 0],
               [0xdf, 255, 255, 255, 255]]) {
    assert.throws(() => unpack(Uint8Array.from(data)));
  }
  let deep = null;
  for (let i = 0; i < 130; i++) {
    deep = [deep];
  }
  assert.throws(() => pack(deep), /nesting/);
  const encoded = pack([1, 2]);
  for (let i = 0; i < encoded.length; i++) {
    assert.throws(() => unpack(encoded.slice(0, i)));
  }
});
test('Tarantool extension golden bytes match Python SDK layouts', () => {
  const examples = [
    [new Decimal('-12.34'), [0xd6, 1, 2, 1, 0x23, 0x4d]],
    [
      new UUID('00000000-0000-0000-0000-000000000001'),
      [0xd8, 2, ...Array(15).fill(0), 1],
    ],
    [new Datetime(-1n), [0xd7, 4, ...Array(8).fill(255)]],
    [
      new Interval({months: 200, days: -77}),
      [0xc7, 9, 6, 3, 1, 0xcc, 200, 3, 0xd0, 0xb3, 8, 1],
    ],
  ];
  for (const [value, expected] of examples) {
    assert.deepEqual([...pack(value)], expected);
    assert.deepEqual(unpack(pack(value)), value);
  }
  for (const value
           of [new Decimal('-0.000'),
               new Decimal('12345.6700'),
               new Decimal('1e10000'),
               new Datetime(
                   -10801n,
                   {nanoseconds: 123456789, tzoffset: 180, tzindex: 1}),
               new Interval({
                 years: -1,
                 months: 2,
                 weeks: 3,
                 days: -4,
                 hours: 5,
                 minutes: -6,
                 seconds: 7,
                 nanoseconds: -8,
                 adjust: 2,
               }),
               new ErrorValue(
                   [new ErrorFrame({
                     code: 42,
                     message: 'failure',
                     fields: {name: 'sdk'},
                     extra: new Map([[9, 2n ** 63n]]),
                   })],
                   {extra: new Map([[7, 'extra']])})]) {
    assert.deepEqual(unpack(pack(value)), value);
  }
  assert.throws(() => +new Decimal('1.2'), /implicitly/);
  for (const sign of [-1n, 1n]) {
    const value = new Interval({
      days: sign * 2n ** 53n,
      hours: sign * 2n ** 53n,
      minutes: sign * 2n ** 53n,
      seconds: sign * 2n ** 53n,
    });
    assert.deepEqual(unpack(pack(value)), value);
  }
  for (const code of [-128, -1, 0, 7, 127]) {
    for (const length of [0, 1, 2, 4, 8, 16, 17, 256, 65536]) {
      const value = new Extension(code, new Uint8Array(length));
      assert.deepEqual(unpack(pack(value)), value);
    }
  }
});
test('raw extensions and malformed known payloads', () => {
  const value = new Decimal('1.00');
  const raw = unpack(pack(value), {rawExtensions: true});
  assert.equal(raw.type, 1);
  assert.deepEqual(unpack(pack(raw)), value);
  for (const value
           of [new Extension(1, Uint8Array.of(0, 0x1a, 0x2c)),
               new Extension(2, new Uint8Array(15)),
               new Extension(4, new Uint8Array(9)),
               new Extension(6, Uint8Array.of(2, 8, 0, 8, 1)),
               new Extension(6, Uint8Array.of(0, 0)),
               new Extension(3, pack({wrong: []}))]) {
    assert.throws(() => unpack(pack(value)));
  }
});
test(
    'Uint8Array subclasses and offset buffers decode and copy correctly',
    () => {
      for (const value
               of [1.5,
                   new Datetime(1n, {nanoseconds: 17}),
                   new Decimal('1.20')]) {
        const data = Buffer.concat([Buffer.of(0), Buffer.from(pack(value))]);
        assert.deepEqual(unpack(data.subarray(1)), value);
      }
      const buffer = Buffer.of(1, 2, 3);
      const ext = new Extension(127, buffer);
      const decoded = unpack(Buffer.from(pack(buffer)));
      buffer[0] = 9;
      assert.equal(ext.data[0], 1);
      assert.equal(decoded[0], 1);
    });
test('values lifted in another JavaScript realm retain their intrinsic types', () => {
  const [binary, record, map, encoded] = runInNewContext(
      '[Uint8Array.of(0,255), {flags:{isNullable:true}}, new Map([[1,"one"]]), Uint8Array.of(0x92,1,2)]');
  assert.equal(binary instanceof Uint8Array, false);
  assert.deepEqual(unpack(pack(binary)), Uint8Array.of(0, 255));
  assert.deepEqual(unpack(pack(record)), {flags: {isNullable: true}});
  assert.deepEqual(unpack(pack(map)), new Map([[1, 'one']]));
  assert.deepEqual(unpack(encoded), [1, 2]);
  assert.deepEqual(new Extension(7, binary).data, Uint8Array.of(0, 255));
  for (
      const source of
          ['new Uint16Array([1])',
           'new (class Custom {})()',
           'new (class Custom extends null {constructor(){return Object.create(new.target.prototype)}})()']) {
    assert.throws(() => pack(runInNewContext(source)), /unsupported/);
  }
  const spoof = Uint16Array.of(1);
  Object.defineProperty(spoof, Symbol.toStringTag, {value: 'Uint8Array'});
  assert.throws(() => pack(spoof), /unsupported/);
});

import {box, ClosedResourceError, Datetime, Decimal, ErrorFrame, ErrorValue, Extension, Interval, LogLevel, pack, TarantoolError, unpack, usingResource, UUID} from 'twr-js-sdk';
import * as raw from 'twr-js-sdk/_tarantool';

function assert(value, message = 'assertion failed') {
  if (!value) {
    throw Error(message);
  }
}
function equal(a, b) {
  assert(
      String(pack(a)) === String(pack(b)),
      `values differ: ${String(pack(a))} / ${String(pack(b))}`);
}
function raises(Type, fn) {
  try {
    fn();
  } catch (error) {
    assert(error instanceof Type, `expected ${Type.name}, got ${error}`);
    return error;
  }
  throw Error(`expected ${Type.name}`);
}
const users = box.space('js_users');
const savedIndex = users.index('primary');
export function schema() {
  return savedIndex.id + users.id + box.schemaVersion;
}
export function roundtrip(data) {
  return pack(unpack(data));
}
const cases = {
  crud() {
    assert(box.schemaVersion > 0);
    equal(box.space(users.id).id, users.id);
    assert(savedIndex.id === 0);
    equal(users.insert([1, 'Alice', 10]), [1, 'Alice', 10]);
    equal(users.get(1), [1, 'Alice', 10]);
    const e = raises(TarantoolError, () => users.insert([1, 'dup', 1]));
    assert(e.code > 0 && e.errorType && e.message);
    equal(users.replace([1, 'Bob', 20]), [1, 'Bob', 20]);
    equal(users.update(1, [['+', 2, 10]]), [1, 'Bob', 30]);
    equal(users.index('name').get('Bob'), [1, 'Bob', 30]);
    users.upsert([1, 'ignored', 0], [['+', 2, 5]]);
    equal(users.get(1), [1, 'Bob', 35]);
    users.upsert([2, 'Eve', 40], []);
    equal(users.delete(2), [2, 'Eve', 40]);
    assert(users.delete(99) === null);
    assert(users.update(99, [['=', 1, 'x']]) === null);
    raises(RangeError, () => box.space('js_missing').id);
    raises(RangeError, () => users.index('missing').id);
    raises(TypeError, () => users.insert({id: 2}));
    users.truncate();
    assert(users.get(1) === null);
  },
  indexes() {
    for (const row of [[1, 'Alice', 10], [2, 'Bob', 20], [3, 'Eve', 30]]) {
      users.replace(row);
    }
    const index = users.index(0);
    assert(index.tupleCount === 3n && index.memorySize > 0n);
    equal(index.min(), [1, 'Alice', 10]);
    equal(index.max(), [3, 'Eve', 30]);
    assert(index.random(123) !== null);
    assert(index.count(2) === 1n);
    assert(index.count(2, {mode: 'greater-than-or-equal'}) === 2n);
    equal(users.select([], {limit: 1, offset: 1}), [[2, 'Bob', 20]]);
    equal(users.select([], {limit: 0}), []);
    equal(users.select([], {offset: 99}), []);
    const cursor = users.iter();
    for (const row of cursor) {
      assert(row[0] === 1);
      break;
    }
    assert(cursor.closed);
    cursor.close();
    const failing = users.iter();
    raises(Error, () => {
      for (const row of failing) {
        assert(row.length > 0);
        throw Error('body');
      }
    });
    assert(failing.closed);
    equal([...users.iter()], users.select());
    equal(index.select(2, {mode: 'less-than'}), [[1, 'Alice', 10]]);
    equal(users.index('name').update('Bob', [['=', 2, 22]]), [2, 'Bob', 22]);
    equal(users.index('name').delete('Eve'), [3, 'Eve', 30]);
  },
  tuples() {
    const value = [1, 'abc', {x: [null, new Uint8Array([0, 255])]}];
    const original = box.tuple(value);
    usingResource(original, (t) => {
      assert(t.length === 3 && t.byteSize === BigInt(t.toBytes().length));
      equal(t.field(0), 1);
      equal(t.field(-2), 'abc');
      equal(t.field('[2].x[0]'), null);
      equal([...t], value);
      raises(RangeError, () => t.field(99));
      usingResource(t.copy(), (r) => equal(r.toArray(), value));
      usingResource(
          t.update([['=', 1, 'xyz']]), (r) => equal(r.field(1), 'xyz'));
      usingResource(t.upsert([['+', 0, 100]]), (r) => equal(r.field(0), 101));
      usingResource(t.format, (f) => t.validate(f));
      usingResource(t.iterFields(), (f) => {
        assert(f.position === 0);
        equal(f.read().value, 1);
        equal(f.seek(1), 'abc');
        assert(f.position === 2);
        equal(f.read().value, value[2]);
        assert(f.read().done);
        f.rewind();
        equal([...f], value);
      });
    });
    original.close();
    raises(ClosedResourceError, () => original.toArray());
  },
  keys() {
    usingResource(
        box.keyDef(
            [{fieldNo: 0, fieldType: 'unsigned', flags: {isNullable: true}}]),
        (first) => {
          assert(first.partCount === 1 && first.parts[0].flags.isNullable);
          usingResource(
              box.keyDef([{fieldNo: 1, fieldType: 'string'}]), (second) => {
                usingResource(
                    first.copy(),
                    (copy) => equal(
                        copy.parts.map(
                            (p) =>
                                [p.fieldNo,
                                 p.fieldType,
                                 p.collation ?? null,
                                 p.path ?? null,
                                 p.flags]),
                        first.parts.map(
                            (p) =>
                                [p.fieldNo,
                                 p.fieldType,
                                 p.collation ?? null,
                                 p.path ?? null,
                                 p.flags])));
                usingResource(first.merge(second), (merged) => {
                  assert(merged.partCount === 2);
                  merged.validateKey([1]);
                  merged.validateKey([1, 'abc'], {full: true});
                  raises(
                      TarantoolError,
                      () => merged.validateKey([1], {full: true}));
                });
                usingResource(
                    box.tuple([1, 'abc']),
                    (left) => usingResource(box.tuple([2, 'xyz']), (right) => {
                      first.validateTuple(left);
                      equal(first.extractKey(left), [1]);
                      assert(
                          left.compare(right, first) < 0 &&
                          left.compareWithKey(1, first) === 0);
                      usingResource(box.tupleFormat([first, second]), (f) => {
                        left.validate(f);
                        usingResource(f.copy(), (copy) => left.validate(copy));
                        usingResource(
                            box.tuple(['bad', 'abc']),
                            (bad) =>
                                raises(TarantoolError, () => bad.validate(f)));
                      });
                      usingResource(box.tupleFormat(), (f) => left.validate(f));
                    }));
              });
        });
    usingResource(
        box.keyDef(
            [{fieldNo: 1, fieldType: 'string', collation: 'unicode-ci'}]),
        (d) => assert(d.parts[0].collation === 'unicode-ci'));
  },
  sequences() {
    const seq = box.sequence(box.space('js_ids').get(1)[1]);
    seq.set(20);
    assert(seq.current === 20n && seq.next() === 21n);
    seq.reset();
    assert(seq.next() === 1n);
  },
  diagnostics() {
    const e = box.error.new(1, 'expected', {file: 'guest.js', line: 17});
    assert(e.code === 1 && e.file === 'guest.js' && e.line === 17);
    assert(box.error.format(e).includes('expected'));
    box.error.set(e);
    assert(box.error.last().line === 17);
    box.error.clear();
    assert(box.error.last() === null);
    assert(box.error.new(1, 'simple').message === 'simple');
    assert(box.metrics.uptime() >= 0);
  },
  logging() {
    for (const level of Object.values(LogLevel)) {
      box.log.write(
          level, 'JS SDK log ' + level, {context: {file: 'guest.js', line: 1}});
    }
  },
  messagepack() {
    const value = new Map([
      [1, null],
      ['bin', new Uint8Array([0, 255])],
      ['values', [false, 2n ** 63n, -3, 1.5]],
      ['ext', new Extension(127, Uint8Array.of(42))],
    ]);
    equal(box.msgpack.unpack(box.msgpack.pack(value)), value);
    equal(
        box.msgpack.unpack(box.msgpack.fromJson('{"a":[1,true]}')),
        {a: [1, true]});
    assert(box.msgpack.toJson(pack({a: 1})).includes('a'));
    raises(TarantoolError, () => box.msgpack.fromJson('{'));
    const handle = raw.boxTuple.new(pack([42]));
    try {
      equal(unpack(raw.boxTuple.toBytes(handle)), [42]);
    } finally {
      raw.boxTuple.release(handle);
    }
  },
  sessions() {
    assert(box.session.current().id >= 0n);
    box.session.broadcastLua('js_event', '{answer=42}');
    const target = box.session.get(box.space('js_ids').get(2)[1]);
    target.send(new Map([[0, 0], [1, 77]]), new Map([[0x30, 'sdk']]));
    target.send(new Map([[0, 0], [1, 78]]));
  },
  extensions() {
    const value = [
      1,
      new Decimal('-12.34'),
      new UUID('f6423bdf-b49e-4913-b361-0740c9702e4b'),
      new Datetime(-10801n, {nanoseconds: 123456789, tzoffset: 180}),
      new Interval({years: 1, months: 200, days: -77}),
      new ErrorValue([new ErrorFrame({
        code: 42,
        message: 'JS error',
        errorType: 'CustomError',
        file: 'guest.js',
        line: 17,
        fields: {object_name: 'sdk'},
      })]),
    ];
    const s = box.space('js_specials');
    equal(s.replace(value), value);
    equal(s.get(1), value);
    for (const [field, type] of
             [[1, 'decimal'], [2, 'uuid'], [3, 'datetime']]) {
      usingResource(
          box.keyDef([{fieldNo: field, fieldType: type}]),
          (d) => usingResource(box.tuple(value), (t) => {
            d.validateKey(value[field], {full: true});
            equal(d.extractKey(t), [value[field]]);
            assert(t.compareWithKey(value[field], d) === 0);
          }));
    }
    for (const adjust of [0, 1, 2]) {
      const row = [adjust + 2, new Interval({months: 2, days: -1, adjust})];
      equal(box.space('js_intervals').replace(row), row);
    }
    const big = [4, 2n ** 64n - 1n, -(2n ** 63n)];
    equal(box.space('js_bigints').replace(big), big);
    equal(box.space('js_bigints').get(4), big);
  },
  transactionRejected() {
    const error = raises(TarantoolError, () => box.txn.begin());
    assert(error.message.includes('reschedulable wasm.run'));
  },
  transactions() {
    assert(box.txn.id === null);
    box.transaction(() => {
      assert(box.txn.id !== null && box.txn.isolation === 'read-confirmed');
      box.txn.setIsolation('best-effort');
      raises(Error, () => box.transaction(() => 1));
      users.replace([11, 'commit', 1]);
    }, {isolation: 'read-confirmed'});
    raises(Error, () => box.transaction(() => {
      users.replace([12, 'rollback', 1]);
      throw Error('rollback');
    }));
    raises(TypeError, () => box.transaction(async () => 1));
    raises(TypeError, () => box.transaction(() => ({then() {}})));
    box.txn.begin();
    box.txn.makeSynchronous();
    box.txn.rollback();
    box.txn.begin();
    users.replace([14, 'explicit', 1]);
    box.txn.commit();
    box.txn.begin();
    users.replace([13, 'rollback', 1]);
    box.txn.rollback();
    assert(
        users.get(11) !== null && users.get(12) === null &&
        users.get(13) === null && users.get(14) !== null);
  },
};
export function check(name) {
  if (!Object.hasOwn(cases, name)) {
    throw Error('unknown case ' + name);
  }
  cases[name]();
  return 'ok';
}
export const run = {
  run() {
    try {
      cases.transactions();
    } catch (error) {
      console.error('transaction case:', error.stack);
      throw error;
    }
  },
};

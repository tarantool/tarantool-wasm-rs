import assert from 'node:assert/strict';
import {test} from 'node:test';

import {pack} from '../../src/codec.js';
import {ClosedResourceError, LuaCallbackError, TarantoolError} from '../../src/errors.js';
import {usingResource} from '../../src/resource.js';
import {createLuaCallbacks, createTarantool} from '../../src/testing.js';

import {fakeBindings} from './fake.js';

test('lazy construction, name/id resolution observes schema changes', () => {
  const f = fakeBindings();
  const box = createTarantool(f.bindings);
  const space = box.space('users');
  const index = space.index('primary');
  assert.equal(f.calls.length, 0);
  assert.equal(index.id, 0);
  f.schema(1024, 3);
  assert.equal(space.id, 1024);
  assert.equal(index.id, 3);
  assert.equal(box.space(512).index(2).id, 2);
  assert.throws(() => box.space('missing').id, /does not exist/);
  assert.throws(() => space.index('missing').id, /does not exist/);
  assert.throws(() => box.space(-1));
});
test('CRUD decodes rows and releases every temporary tuple', () => {
  const f = fakeBindings();
  const box = createTarantool(f.bindings);
  const s = box.space('users');
  assert.deepEqual(s.insert([1, 'name']), [1, 'name']);
  s.replace([1, 2n ** 63n]);
  assert.deepEqual(s.get(1), [1, 2n ** 63n]);
  assert.equal(s.get(2), null);
  s.update(1, [['=', 1, 7]]);
  s.upsert([1, 'new'], []);
  s.delete(2);
  s.index().min();
  s.index().max();
  s.index().random(9);
  s.index().count(1);
  s.truncate();
  assert.equal(f.handles.size, 0);
  assert.throws(() => s.insert({id: 1}));
});
test('tuple, formats and key definitions have independent ownership', () => {
  const f = fakeBindings();
  const box = createTarantool(f.bindings);
  const tuple = box.tuple([1, null, 'name']);
  assert.equal(tuple.length, 3);
  assert.equal(tuple.byteSize, BigInt(tuple.toBytes().length));
  assert.equal(tuple.field(-1), 'name');
  assert.equal(tuple.field(1), null);
  assert.throws(() => tuple.field(99), RangeError);
  const copy = tuple.copy();
  tuple.close();
  assert.deepEqual(copy.toArray(), [1, null, 'name']);
  tuple.close();
  assert.throws(() => tuple.copy(), ClosedResourceError);
  const kd = box.keyDef(
      [{fieldNo: 0, fieldType: 'unsigned', flags: {isNullable: true}}]);
  const other = box.keyDef([{fieldNo: 2, fieldType: 'string'}]);
  assert.equal(kd.partCount, 1);
  assert.equal(kd.parts[0].flags.isNullable, true);
  usingResource(kd.copy(), (k) => assert.equal(k.partCount, 1));
  usingResource(kd.merge(other), (k) => assert.equal(k.partCount, 2));
  kd.validateKey(1);
  kd.validateKey([1], {full: true});
  kd.validateTuple(copy);
  assert.deepEqual(kd.extractKey(copy), [1]);
  assert.equal(copy.compare(copy, kd), -1);
  assert.equal(copy.compareWithKey(1, kd), 0);
  for (const format
           of [box.tupleFormat(), box.tupleFormat([kd, other]), copy.format]) {
    copy.validate(format);
    usingResource(format.copy(), (x) => copy.validate(x));
    format.close();
  }
  usingResource(
      copy.update([['=', 1, 99]]), (x) => assert.equal(x.field(1), 99));
  usingResource(
      copy.upsert([]), (x) => assert.deepEqual(x.toArray(), copy.toArray()));
  const foreign = createTarantool(f.bindings).tupleFormat();
  assert.throws(() => copy.validate(foreign), /different/);
  foreign.close();
  copy.close();
  kd.close();
  other.close();
  assert.equal(f.handles.size, 0);
});
test(
    'iterators close on exhaustion, break, body error and decoding error',
    () => {
      const f = fakeBindings();
      const box = createTarantool(f.bindings);
      const s = box.space(512);
      s.replace([1]);
      s.replace([2]);
      assert.deepEqual(s.select([], {offset: 1, limit: 1}), [[2]]);
      assert.deepEqual(s.select([], {offset: 10}), []);
      assert.deepEqual(s.select([], {limit: 0}), []);
      for (const row of s.iter()) {
        assert.ok(row);
        break;
      }
      assert.throws(() => {
        for (const row of s.iter()) {
          assert.ok(row);
          throw Error('body');
        }
      }, /body/);
      assert.equal(f.cursors, 0);
      assert.equal(f.handles.size, 0);
      const exhausted = s.iter();
      assert.deepEqual([...exhausted], [[1], [2]]);
      assert.equal(exhausted.next().done, true);
      assert.equal(exhausted.next().done, true);
      const explicit = s.iter();
      explicit.close();
      assert.throws(() => explicit.next(), ClosedResourceError);
      usingResource(box.tuple([1, null]), (tuple) => {
        const cursor = tuple.iterFields();
        assert.equal(cursor.position, 0);
        assert.equal(cursor.seek(1), null);
        assert.equal(cursor.read().done, true);
        cursor.rewind();
        assert.deepEqual([...cursor], [1, null]);
        assert.ok(cursor.closed);
        assert.equal(cursor.next().done, true);
        assert.throws(() => cursor.seek(0), ClosedResourceError);
      });
      f.bindings.boxTuple.toBytes = () => Uint8Array.of(0xc1);
      const cursor = s.iter();
      assert.throws(() => cursor.next());
      assert.ok(cursor.closed);
      assert.equal(f.cursors, 0);
      assert.equal(f.handles.size, 0);
    });
test(
    'errors preserve WIT payload and cleanup never replaces a primary error',
    () => {
      const f = fakeBindings();
      const box = createTarantool(f.bindings);
      const cause = Object.assign(Error('component'), {
        payload: {
          code: 0xffffffff,
          message: 'failure',
          errorType: 'ClientError',
          file: 'x',
          line: 17,
        },
      });
      f.bindings.database.insert = () => {
        throw cause;
      };
      assert.throws(
          () => box.space(512).insert([]),
          (e) => e instanceof TarantoolError && e.code === 0xffffffff &&
              e.cause === cause);
      const primary = Error('decode');
      f.bindings.boxTuple.toBytes = () => {
        throw primary;
      };
      f.bindings.boxTuple.release = () => {
        throw Error('release');
      };
      assert.throws(() => box.space(512).replace([1]), (e) => e === primary);
      const t = box.tuple([]);
      assert.throws(() => usingResource(t, () => {
                      throw primary;
                    }), (e) => e === primary);
      assert.ok(t.closed);
      t.close();
    });
test(
    'tuple cursor operations release handles on host and decode failures',
    () => {
      const f = fakeBindings();
      const box = createTarantool(f.bindings);
      const tuple = box.tuple([1]);
      for (const method of ['seek', 'rewind', 'position']) {
        const cursor = tuple.iterFields();
        const primary = Error(method);
        cursor.getHandle_()[method] = () => {
          throw primary;
        };
        assert.throws(
            () => method === 'position' ? cursor.position : cursor[method](0),
            (e) => e === primary);
        assert.ok(cursor.closed);
        assert.equal(f.cursors, 0);
      }
      const malformed = tuple.iterFields();
      malformed.getHandle_().seek = () => Uint8Array.of(0xc1);
      assert.throws(() => malformed.seek(0));
      assert.ok(malformed.closed);
      const missing = tuple.iterFields();
      assert.throws(() => missing.seek(99), RangeError);
      assert.ok(missing.closed);
      tuple.close();
      assert.equal(f.cursors, 0);
      assert.equal(f.handles.size, 0);
    });
test(
    'transaction helper commits, rolls back, rejects async and replacement/nesting',
    () => {
      const f = fakeBindings();
      const box = createTarantool(f.bindings);
      assert.equal(box.txn.id, null);
      assert.equal(box.transaction(() => {
        assert.equal(box.txn.isolation, 'read-confirmed');
        assert.throws(() => box.transaction(() => 0), /nested/);
        box.txn.setIsolation('best-effort');
        return 42;
      }, {isolation: 'read-confirmed', synchronous: true}), 42);
      assert.equal(box.txn.id, null);
      assert.equal(f.calls.filter((c) => c[1] === 'commit').length, 1);
      assert.throws(() => box.transaction(async () => 1), /synchronous/);
      assert.throws(
          () => box.transaction(() => Promise.resolve(1)), /thenable/);
      const primary = Error('body');
      f.bindings.transaction.rollback = () => {
        throw Error('rollback');
      };
      assert.throws(
          () => box.transaction(() => {
            throw primary;
          }),
          (e) => e === primary);
      f.bindings.transaction.rollback = f.modules.transaction.rollback;
      box.txn.rollback();
      assert.throws(
          () => box.transaction(() => box.txn.commit()), /ended or replaced/);
      assert.throws(
          () => box.transaction(() => {
            box.txn.rollback();
            box.txn.begin();
          }),
          /ended or replaced/);
      assert.notEqual(box.txn.id, null);
      box.txn.rollback();
      box.txn.begin();
      assert.throws(() => box.txn.begin(), /nested/);
      box.txn.rollback();
    });
test(
    'diagnostics, sequences, sessions, logging, host msgpack and metrics',
    () => {
      const f = fakeBindings();
      const box = createTarantool(f.bindings);
      for (const e
               of [box.error.new(1, 'simple'),
                   box.error.new(
                       42, 'location', {file: 'guest.js', line: 17})]) {
        box.error.set(e);
        assert.equal(box.error.last().message, e.message);
        assert.equal(box.error.format(e), e.message);
      }
      box.error.clear();
      assert.equal(box.error.last(), null);
      assert.equal(box.metrics.uptime(), 42);
      assert.equal(box.schemaVersion, 1);
      box.log.write('info', 'message', {context: {file: 'test', line: 1}});
      assert.throws(() => box.log.write('fatal', 'stop'));
      const sequence = box.sequence(1);
      sequence.set(2n ** 63n - 1n);
      assert.equal(sequence.current, 20n);
      assert.equal(sequence.next(), 1n);
      sequence.reset();
      assert.throws(() => sequence.set(2 ** 53));
      const session = box.session.current();
      assert.equal(session.id, 1n);
      session.send(new Map([[0, 0], [1, 77]]), {data: 'sdk'});
      box.session.get(2).send({});
      box.session.broadcastLua('event', '{a=1}');
      assert.throws(() => session.send([]));
      assert.deepEqual(
          box.msgpack.unpack(box.msgpack.fromJson('{"a":1}')), {a: 1});
      assert.equal(box.msgpack.toJson(pack({a: 1})), '{"a":1}');
    });
test(
    'Lua callbacks use MessagePack, expose typed errors, close via finally',
    () => {
      let live = 0;
      const host = {
        open(name) {
          live++;
          return {
            call(data) {
              if (name === 'fail') {
                throw Object.assign(
                    new Error('component'), {payload: 'callback failure'});
              }
              return data;
            },
            [Symbol.dispose]() {
              live--;
            },
          };
        },
      };
      const callbacks = createLuaCallbacks(host);
      assert.equal(live, 0);
      assert.deepEqual(
          callbacks.withCallback('echo', (c) => c.call({x: [null, 2n ** 63n]})),
          {x: [null, 2n ** 63n]});
      assert.equal(live, 0);
      assert.equal(callbacks.withCallback('echo', (c) => c.call()), null);
      assert.throws(
          () => callbacks.withCallback('echo', (c) => c.call(undefined)),
          TypeError);
      assert.equal(live, 0);
      assert.throws(
          () => callbacks.withCallback('fail', (c) => c.call(1)),
          LuaCallbackError);
      assert.equal(live, 0);
      const c = callbacks.open('echo');
      c.close();
      c.close();
      assert.throws(() => c.call(), ClosedResourceError);
      assert.equal(live, 0);
    });

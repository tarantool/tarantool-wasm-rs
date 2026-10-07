import assert from 'node:assert/strict';
import {readdirSync, readFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';

import {Index, IndexIterator, Space} from '../src/database.js';
import {LuaCallback} from '../src/lua-callback-core.js';
import {Sequence, Session} from '../src/services.js';
import {createTarantool} from '../src/testing.js';
import {KeyDef, Tuple, TupleCursor, TupleFormat} from '../src/tuples.js';

const root = fileURLToPath(new URL('../', import.meta.url));
const SDK_METHODS = {
  'tarantool:tarantool/metrics@0.2.0': {
    'uptime': 'box.metrics.uptime',
  },
  'tarantool:tarantool/box-tuple@0.2.0': {
    'new': 'box.tuple',
    'retain': 'Tuple.copy',
    'release': 'Tuple.close',
    'field-count': 'Tuple.length',
    'byte-size': 'Tuple.byteSize',
    'to-bytes': 'Tuple.toBytes',
    'format': 'Tuple.format',
    'field': 'Tuple.field',
    'field-by-path': 'Tuple.field',
    'update': 'Tuple.update',
    'upsert': 'Tuple.upsert',
    'validate': 'Tuple.validate',
    'compare': 'Tuple.compare',
    'compare-with-key': 'Tuple.compareWithKey',
    'iterator.new': 'Tuple.iterFields',
    'iterator.position': 'TupleCursor.position',
    'iterator.rewind': 'TupleCursor.rewind',
    'iterator.seek': 'TupleCursor.seek',
    'iterator.next': 'TupleCursor.read',
  },
  'tarantool:tarantool/database@0.2.0': {
    'schema-version': 'box.schemaVersion',
    'space-by-name': 'Space.id',
    'index-by-name': 'Index.id',
    'insert': 'Space.insert',
    'update': 'Index.update',
    'replace': 'Space.replace',
    'upsert': 'Index.upsert',
    'delete': 'Index.delete',
    'truncate': 'Space.truncate',
  },
  'tarantool:tarantool/error@0.2.0': {
    'new': 'box.error.new',
    'new-with-location': 'box.error.new',
    'set': 'box.error.set',
    'last': 'box.error.last',
    'clear': 'box.error.clear',
    'to-string': 'box.error.format',
  },
  'tarantool:tarantool/index@0.2.0': {
    'tuple-count': 'Index.tupleCount',
    'memory-size': 'Index.memorySize',
    'random': 'Index.random',
    'get': 'Index.get',
    'min': 'Index.min',
    'max': 'Index.max',
    'count': 'Index.count',
    'iterator.new': 'Index.iter',
    'iterator.next': 'IndexIterator.next',
  },
  'tarantool:tarantool/key-def@0.2.0': {
    'new': 'box.keyDef',
    'duplicate': 'KeyDef.copy',
    'merge': 'KeyDef.merge',
    'release': 'KeyDef.close',
    'dump-parts': 'KeyDef.parts',
    'validate-key': 'KeyDef.validateKey',
    'validate-full-key': 'KeyDef.validateKey',
    'part-count': 'KeyDef.partCount',
    'extract-key': 'KeyDef.extractKey',
    'validate-tuple': 'KeyDef.validateTuple',
  },
  'tarantool:tarantool/log@0.2.0': {
    'write': 'box.log.write',
  },
  'tarantool:tarantool/msgpack@0.2.0': {
    'from-json': 'box.msgpack.fromJson',
    'to-json': 'box.msgpack.toJson',
  },
  'tarantool:tarantool/sequence@0.2.0': {
    'next': 'Sequence.next',
    'current': 'Sequence.current',
    'set': 'Sequence.set',
    'reset': 'Sequence.reset',
  },
  'tarantool:tarantool/session@0.2.0': {
    'current': 'box.session.current',
    'broadcast': 'box.session.broadcastLua',
    'iproto-send': 'Session.send',
  },
  'tarantool:tarantool/transaction@0.2.0': {
    'id': 'box.txn.id',
    'isolation': 'box.txn.isolation',
    'begin': 'box.txn.begin',
    'commit': 'box.txn.commit',
    'rollback': 'box.txn.rollback',
    'set-isolation': 'box.txn.setIsolation',
    'make-synchronous': 'box.txn.makeSynchronous',
  },
  'tarantool:tarantool/tuple-format@0.2.0': {
    'default': 'box.tupleFormat',
    'new': 'box.tupleFormat',
    'retain': 'TupleFormat.copy',
    'release': 'TupleFormat.close',
  },
  'docs:adder/lua-callback@0.1.3': {
    'open': 'open',
    'handler.call': 'LuaCallback.call',
  },
  'docs:adder/zlib@0.1.3': {
    'compress': 'rawZlib.compress',
    'decompress': 'rawZlib.decompress',
    'crc32': 'rawZlib.crc32',
    'adler32': 'rawZlib.adler32',
  },
};

const coverage = Object.fromEntries(
    Object.entries(SDK_METHODS).flatMap(([iface, methods]) => {
      const [name, version] = iface.split('@');
      return Object.entries(methods).map(
          ([method, sdk]) => [`${name}/${method}@${version}`, sdk]);
    }));
const dependency =
    readdirSync(root + 'wit/deps').find((x) => x.startsWith('tarantool'));
assert.ok(dependency, 'fetch released WIT before checking coverage');
const source =
    readFileSync(root + 'wit/deps/' + dependency + '/package.wit', 'utf8');
assert.match(source, /package tarantool:tarantool@0\.2\.0;/);
const runtime = readFileSync(root + 'wit/runtime/host.wit', 'utf8');
function functions(text, packageName, version) {
  text = text.replace(/\/\/[^\n]*/g, '');
  const out = [];
  for (const iface of text.matchAll(/interface ([\w-]+)\s*{/g)) {
    const pos = iface.index + iface[0].length;
    let end = pos;
    let depth = 1;
    while (depth) {
      if (text[end] === '{') {
        depth++;
      }
      if (text[end] === '}') {
        depth--;
      }
      end++;
    }
    let body = text.slice(pos, end - 1);
    for (const res of body.matchAll(/resource ([\w-]+)\s*{([^}]*)}/g)) {
      for (const fn of res[2].matchAll(/([\w-]+):\s*(?:static )?func\(/g)) {
        out.push(`${packageName}/${iface[1]}/${res[1]}.${fn[1]}@${version}`);
      }
    }
    body = body.replace(/resource [\w-]+\s*{[^}]*}/g, '');
    for (const fn of body.matchAll(/([\w-]+):\s*func\(/g)) {
      out.push(`${packageName}/${iface[1]}/${fn[1]}@${version}`);
    }
  }
  return out;
}
const expected = [
  ...functions(source, 'tarantool:tarantool', '0.2.0'),
  ...functions(runtime, 'docs:adder', '0.1.3'),
].sort();
assert.deepEqual(
    Object.keys(coverage).sort(),
    expected,
    'WIT coverage must have exactly one entry per function/method');
const box = createTarantool({});
const owners = {
  box,
  Space: Space.prototype,
  Index: Index.prototype,
  IndexIterator: IndexIterator.prototype,
  Tuple: Tuple.prototype,
  TupleCursor: TupleCursor.prototype,
  KeyDef: KeyDef.prototype,
  TupleFormat: TupleFormat.prototype,
  Sequence: Sequence.prototype,
  Session: Session.prototype,
  LuaCallback: LuaCallback.prototype,
};
const extras = new Set(['open']);
const rawZlib = readFileSync(root + 'src/_zlib.js', 'utf8');
const rawExports =
    rawZlib.match(/export\s*{([^}]+)}\s*from\s*'docs:adder\/zlib@0\.1\.3'/);
assert.ok(rawExports, 'raw zlib must re-export the maintained WIT interface');
const rawNames = rawExports[1].split(',').map((name) => name.trim());
function descriptor(owner, key) {
  for (let obj = owner; obj; obj = Object.getPrototypeOf(obj)) {
    const d = Object.getOwnPropertyDescriptor(obj, key);
    if (d) {
      return d;
    }
  }
}
for (const [wit, sdk] of Object.entries(coverage)) {
  const segments = sdk.split('.');
  if (sdk.startsWith('rawZlib.')) {
    assert.equal(segments[0], 'rawZlib');
    assert.ok(
        rawNames.includes(segments[1]), `${wit} maps to a missing raw export`);
  } else if (!extras.has(sdk)) {
    let object = owners[segments.shift()];
    assert.ok(object, sdk);
    while (segments.length > 1) {
      object = object[segments.shift()];
    }
    assert.ok(descriptor(object, segments[0]), `${wit} maps to missing ${sdk}`);
  }
}
// Compare packaged extra interfaces byte-for-byte with the maintained ABI.
const host = readFileSync(root + '../../wit/adder.wit', 'utf8');
assert.equal(
    runtime,
    'package docs:adder@0.1.3;\n\n' +
        host.slice(host.indexOf('interface zlib')));
console.log(`${
    expected
        .length} WIT functions/methods covered (75 database, 2 callback, 4 raw zlib)`);

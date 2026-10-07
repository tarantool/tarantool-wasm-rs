import {pack, unpack} from '../../src/codec.js';

export function fakeBindings() {
  const calls = [];
  const handles = new Map();
  let next = 1n;
  let txid;
  let txcounter = 0n;
  let isolation;
  let diagnostic;
  const owned = (value) => {
    const h = {handle: next++};
    handles.set(h.handle, {value, refs: 1});
    return h;
  };
  const entry = (h) => {
    const item = handles.get(h.handle);
    if (!item) {
      throw new Error('invalid handle');
    }
    return item;
  };
  const release = (h) => {
    const e = entry(h);
    if (--e.refs === 0) {
      handles.delete(h.handle);
    }
  };
  const data = new Map();
  let spaceId = 512;
  let indexId = 0;
  const modules = {
    database: {
      schemaVersion: () => 1,
      spaceByName: (n) => n === 'missing' ? undefined : {id: spaceId},
      indexByName: (_, n) =>
          n === 'missing' ? undefined : {id: indexId, spaceId},
      insert: (_, v) => owned(unpack(v)),
      replace: (_, v) => {
        const row = unpack(v);
        data.set(row[0], row);
        return owned(row);
      },
      update: () => undefined,
      delete: () => undefined,
      upsert: () => {},
      truncate: () => data.clear(),
    },
    boxTuple: {
      new: (v) => owned(unpack(v)),
      retain: (h) => entry(h).refs++,
      release,
      fieldCount: (h) => entry(h).value.length,
      byteSize: (h) => BigInt(pack(entry(h).value).length),
      toBytes: (h) => pack(entry(h).value),
      format: () => owned('format'),
      field: (h, n) => {
        const v = entry(h).value;
        return n < v.length ? pack(v[n]) : undefined;
      },
      fieldByPath: () => undefined,
      update: (h, ops) => {
        const row = entry(h).value.slice();
        for (const [op, i, v] of unpack(ops)) {
          if (op === '=') {
            row[i - 1] = v;
          }
        }
        return owned(row);
      },
      upsert: (h) => owned(entry(h).value),
      validate: () => {},
      compare: () => -1,
      compareWithKey: () => 0,
    },
    tupleFormat: {
      default: () => owned('format'),
      new: () => owned('format'),
      retain: (h) => entry(h).refs++,
      release,
    },
    keyDef: {
      new: (parts) => owned(parts),
      duplicate: (h) => owned(entry(h).value),
      merge: (a, b) => owned([...entry(a).value, ...entry(b).value]),
      release,
      dumpParts: (h) => entry(h).value,
      partCount: (h) => entry(h).value.length,
      validateKey: () => {},
      validateFullKey: () => {},
      extractKey: (_, h) => pack([entry(h).value[0]]),
      validateTuple: () => {},
    },
    index: {
      tupleCount: () => BigInt(data.size),
      memorySize: () => 64n,
      get: (_, key) => data.has(unpack(key)[0]) ?
          owned(data.get(unpack(key)[0])) :
          undefined,
      min: () => data.size ? owned([...data.values()][0]) : undefined,
      max: () => data.size ? owned([...data.values()].at(-1)) : undefined,
      random: () => data.size ? owned([...data.values()][0]) : undefined,
      count: () => BigInt(data.size),
    },
    transaction: {
      id: () => txid,
      isolation: () => isolation,
      begin: () => {
        txid = ++txcounter;
        isolation = 'default';
      },
      commit: () => {
        txid = undefined;
      },
      rollback: () => {
        txid = undefined;
      },
      setIsolation: (v) => {
        isolation = v;
      },
      makeSynchronous: () => {},
    },
    sequence:
        {next: () => 1n, current: () => 20n, set: () => {}, reset: () => {}},
    session:
        {current: () => ({id: 1n}), broadcast: () => {}, iprotoSend: () => {}},
    error: {
      new: (code, message) => ({code, message, errorType: 'ClientError'}),
      newWithLocation: (code, message, file, line) =>
          ({code, message, errorType: 'ClientError', file, line}),
      set: (v) => {
        diagnostic = v;
      },
      last: () => diagnostic,
      clear: () => {
        diagnostic = undefined;
      },
      toString: (v) => v.message,
    },
    log: {write: () => {}},
    metrics: {uptime: () => 42},
    msgpack: {
      fromJson: (v) => pack(JSON.parse(v)),
      toJson: (v) => JSON.stringify(unpack(v)),
    },
  };
  let cursors = 0;
  function iterator(values, tuples) {
    let pos = 0;
    cursors++;
    return {
      next() {
        return pos < values.length ?
            (tuples ? owned(values[pos++]) : pack(values[pos++])) :
            undefined;
      },
      position: () => pos,
      rewind: () => {
        pos = 0;
      },
      seek(n) {
        pos = n;
        return this.next();
      },
      [Symbol.dispose]() {
        cursors--;
      },
    };
  }
  modules.index.Iterator = {new: () => iterator([...data.values()], true)};
  modules.boxTuple.Iterator = {new: (h) => iterator(entry(h).value, false)};
  const bindings = {};
  for (const [iface, module] of Object.entries(modules)) {
    bindings[iface] = {};
    for (const [method, fn] of Object.entries(module)) {
      bindings[iface][method] = method === 'Iterator' ? fn : (...args) => {
        calls.push([iface, method, ...args]);
        return fn(...args);
      };
    }
  }
  return {
    bindings,
    modules,
    calls,
    handles,
    data,
    get cursors() {
      return cursors;
    },
    schema(space, index) {
      spaceId = space;
      indexId = index;
    },
  };
}

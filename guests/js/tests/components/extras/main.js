import {adler32, compress, crc32, decompress} from 'twr-js-sdk/_zlib';
import {Datetime, Decimal, ErrorFrame, ErrorValue, Interval, pack, UUID} from 'twr-js-sdk/codec';
import {LuaCallbackError, open, withCallback} from 'twr-js-sdk/lua-callback';

let saved;
function assert(value, message = 'assertion failed') {
  if (!value) {
    throw Error(message);
  }
}
function equal(a, b) {
  if (a === null || b === null || typeof a !== 'object' ||
      typeof b !== 'object') {
    assert(a === b);
    return;
  }
  if (Object.getPrototypeOf(a) === Object.prototype &&
      Object.getPrototypeOf(b) === Object.prototype) {
    assert(Object.keys(a).length === Object.keys(b).length);
    for (const k of Object.keys(a)) {
      assert(Object.hasOwn(b, k));
      equal(a[k], b[k]);
    }
    return;
  }
  assert(
      String(pack(a)) === String(pack(b)),
      `callback values differ: ${String(pack(a))} / ${String(pack(b))}`);
}
export function invoke(value) {
  saved ??= open('js_replace');
  return BigInt(saved.call(value));
}
export function close() {
  saved?.close();
  saved = undefined;
}
export function check(name) {
  if (name === 'lua-callback') {
    const value = {
      a: [1, null, 2n ** 63n, -(2n ** 63n)],
      decimal: new Decimal('-12.34'),
      uuid: new UUID('00000000-0000-0000-0000-000000000001'),
      datetime: new Datetime(1n, {nanoseconds: 123456789, tzoffset: 180}),
      interval: new Interval({months: 2}),
      error: new ErrorValue([new ErrorFrame({code: 42, message: 'callback'})]),
    };
    withCallback('js_echo', (c) => {
      equal(c.call(value), value);
      equal(c.call([]), []);
      equal(c.call({}), {});
      const result =
          c.call(new Map([[1, 'one'], [3, null], ['large', 2n ** 63n]]));
      assert(
          result instanceof Map && result.size === 3 &&
          result.get(1) === 'one' && result.get(3) === null &&
          result.get('large') === 2n ** 63n);
    });
    withCallback('js_echo', (c) => assert(c.call() === null));
    withCallback(
        'js_binary',
        (c) => equal(c.call(Uint8Array.of(0, 255)), Uint8Array.of(0, 255)));
    for (const name of ['js_fail', 'js_missing']) {
      let failed = false;
      try {
        withCallback(name, (c) => c.call(1));
      } catch (e) {
        assert(e instanceof LuaCallbackError);
        failed = true;
      }
      assert(failed);
    }
    const c = open('js_echo');
    c.close();
    c.close();
    let failed = false;
    try {
      c.call(1);
    } catch (e) {
      failed = e.name === 'ClosedResourceError';
    }
    assert(failed);
  } else if (name === 'zlib') {
    const data = new TextEncoder().encode('123456789');
    equal(decompress(compress(data, 6)), data);
    assert(crc32(data) === 0xcbf43926 && adler32(data) === 0x091e01de);
    for (const level of [0, 1, 9]) {
      equal(decompress(compress(data, level)), data);
    }
    equal(decompress(compress(new Uint8Array(), 6)), new Uint8Array());
  } else {
    throw Error('unknown case');
  }
  return 'ok';
}

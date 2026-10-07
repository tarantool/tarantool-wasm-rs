import {box, Tarantool, IteratorType, Decimal, UUID, Datetime, Interval, ErrorFrame, ErrorValue, Extension, usingResource, type Value} from 'twr-js-sdk';
import * as raw from 'twr-js-sdk/_tarantool';
import {open, withCallback} from 'twr-js-sdk/lua-callback';
import {compress, decompress} from 'twr-js-sdk/_zlib';
import {createTarantool} from 'twr-js-sdk/testing';
const db: Tarantool = createTarantool(raw);
const row: Value[] = [
  1,
  'user',
  2n ** 63n,
  new Decimal('1.20'),
  new UUID('00000000-0000-0000-0000-000000000001'),
  new Datetime(1n),
  new Interval(),
  new ErrorValue([new ErrorFrame({code: 1, message: 'error'})]),
  new Extension(127, new Uint8Array()),
];
box.space('users').insert(row);
box.space(512).index('primary').select([], {mode: IteratorType.all, limit: 1n});
for (const item of db.space('users').iter()) {
  item[0];
  break;
}
using tuple = box.tuple(row);
using key = box.keyDef(
    [{fieldNo: 0, fieldType: 'unsigned', flags: {isNullable: true}}]);
using format = box.tupleFormat([key]);
tuple.validate(format);
tuple.compareWithKey(1, key);
key.validateKey(1, {full: true});
const result: number = box.transaction(() => 42, {isolation: 'read-confirmed'});
// @ts-expect-error async transaction bodies are explicitly unsupported
box.transaction(async () => 1);
// @ts-expect-error undefined cannot silently turn into nil
box.msgpack.pack(undefined);
// @ts-expect-error unsupported key part type
box.keyDef([{fieldNo: 0, fieldType: 'any'}]);
using callback = open('echo');
callback.call(row);
withCallback('echo', c => c.call(null));
// @ts-expect-error omitted payload means nil; explicit undefined is unsupported
callback.call(undefined);
usingResource(tuple.copy(), t => t.toArray());
box.session.get(5n).send(new Map([[0, 0], [1, 77]]));
const data: Uint8Array = decompress(compress(new Uint8Array(), 9));
new Interval({days: 2n ** 53n, hours: -(2n ** 53n), adjust: 0n});
const handle: raw.BoxTuple = raw.boxTuple.new(data);
raw.boxTuple.release(handle);
raw.tupleFormat.default();
const count: bigint = raw.index.tupleCount({id: 0, spaceId: 512});
void[result, count];

// @ts-expect-error raw zlib requires the WIT level argument
compress(new Uint8Array());
// @ts-expect-error raw zlib has no options-object wrapper
compress(new Uint8Array(), {level: 9});

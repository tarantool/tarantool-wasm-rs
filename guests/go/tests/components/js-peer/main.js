import {box, Datetime, Decimal, Interval, UUID} from 'twr-js-sdk';
function assert(ok) { if (!ok) throw new Error('Go/JS value mismatch'); }
export function exchange() {
  const row = box.space('go_specials').get(1);
  assert(row[1] instanceof Decimal && row[1].digits === '12345678901234567890123456789012345678');
  assert(row[2] instanceof UUID && row[2].bytes[0] === 1);
  assert(row[3] instanceof Datetime && row[3].seconds === -1n && row[3].nanoseconds === 999999999);
  assert(row[4] instanceof Interval && row[4].years === 2 && row[4].adjust === 2);
  assert(row[5].stack[0].message === 'go');
  const big = box.space('go_bigints').get(1);
  assert(big[1] === 2n**64n-1n && big[2] === -(2n**63n) && big[3] === 2n**53n);
  box.space('go_specials').replace([3, ...row.slice(1)]);
  return 'ok';
}
export function roundtrip(data) { return box.msgpack.pack(box.msgpack.unpack(data)); }

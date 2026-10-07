import {crc32, deflateSync, gunzipSync, gzipSync, inflateSync} from 'node:zlib';

export function check() {
  const bytes = new TextEncoder().encode('123456789');
  if (new TextDecoder().decode(inflateSync(deflateSync(bytes))) !==
      '123456789') {
    throw new Error('deflate');
  }
  if (new TextDecoder().decode(gunzipSync(gzipSync(bytes))) !== '123456789') {
    throw new Error('gzip');
  }
  return String(crc32(bytes));
}

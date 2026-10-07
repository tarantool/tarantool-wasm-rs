import {existsSync, readFileSync} from 'node:fs';
import {box, Decimal, pack, unpack} from 'twr-js-sdk';
import * as raw from 'twr-js-sdk/_tarantool';

const manifestUrl =
    new URL(import.meta.resolve('twr-js-sdk/sdk-manifest.json'));
const manifest = JSON.parse(readFileSync(manifestUrl));

if (unpack(pack(new Decimal('1.20'))).toString() !== '1.20') {
  throw new Error('codec mismatch');
}
if (typeof raw.database.insert !== 'function' ||
    manifest.sdk_version !== '0.1.0') {
  throw new Error('package mismatch');
}
const root = new URL('./', manifestUrl);
for (const name of ['api-coverage.json', 'LICENSE']) {
  if (existsSync(new URL(name, root))) {
    throw new Error(`unexpected package file: ${name}`);
  }
}
box.space('users');

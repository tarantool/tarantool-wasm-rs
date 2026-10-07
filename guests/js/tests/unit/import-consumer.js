import {box, Tarantool} from 'twr-js-sdk';
import * as raw from 'twr-js-sdk/_tarantool';

new Tarantool();
box.space('users').index('primary');
if (typeof raw.database.insert !== 'function') {
  throw new Error('missing raw export');
}

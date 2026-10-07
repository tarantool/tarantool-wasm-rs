import * as raw from './_tarantool.js';
import {Tarantool as Core} from './core.js';

export {Index, IndexIterator, Space} from './database.js';
export {ClosedResourceError, LuaCallbackError, TarantoolError} from './errors.js';
export {usingResource} from './resource.js';
export {KeyDef, Tuple, TupleCursor, TupleFormat} from './tuples.js';

export * from './types.js';
export {Extension, UUID, Decimal, Datetime, Interval, IntervalAdjust, ErrorFrame, ErrorValue, pack, unpack} from './codec.js';
/** Provides the database API over lazy, injected WIT bindings. */
export class Tarantool extends Core {
  constructor({bindings = raw} = {}) {
    super({bindings});
  }
}
// Construction only installs JS wrappers. No host calls or handles at import.
export const box = new Tarantool();

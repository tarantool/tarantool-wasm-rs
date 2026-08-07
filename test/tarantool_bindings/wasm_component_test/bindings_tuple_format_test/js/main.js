import { new as keyDefNew } from 'tarantool:tarantool/key-def@0.2.0';
import { new as TupleFormatNew, retain, release, default as TupleFormatDefault } from 'tarantool:tarantool/tuple-format@0.2.0';

export function tDefault() {
    return TupleFormatDefault()
}

export function tKeyDefNew(parts) {
    return keyDefNew(parts)
}

export function tNew(keyDefs) {
    return TupleFormatNew(keyDefs)
}

export function tRef(tf) {
    retain(tf)
}

export function tUnref(tf) {
    release(tf)
}

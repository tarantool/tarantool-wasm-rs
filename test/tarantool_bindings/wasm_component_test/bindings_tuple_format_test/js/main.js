import { new as keyDefNew } from 'tarantool:tarantool/key-def@0.1.2';
import { new as TupleFormatNew, ref, unref, default as TupleFormatDefault } from 'tarantool:tarantool/tuple-format@0.1.2';

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
    ref(tf)
}

export function tUnref(tf) {
    unref(tf)
}

export function tDelete(keyDef) {
    return deleteKeyDef(keyDef)
}

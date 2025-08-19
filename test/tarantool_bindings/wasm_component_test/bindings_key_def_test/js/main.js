import { new as keyDefNew, dup, merge, delete as deleteKeyDef, dumpParts, validateFullKey, validateKey, partCount, extractKey, validateTuple } from 'tarantool:tarantool/key-def@0.1.2';
import { decodeFromRawPtr as decode } from 'tarantool:tarantool/msgpack@0.1.2';
import { new as boxTupleNew } from 'tarantool:tarantool/box-tuple@0.1.2';

export function tTupleNew(data) {
    return boxTupleNew(data)
}

export function tNew(parts) {
    return keyDefNew(parts)
}

export function tDup(keyDef) {
    return dup(keyDef)
}

export function tMerge(left, right) {
    return merge(left, right)
}

export function tDelete(keyDef) {
    return deleteKeyDef(keyDef)
}

export function tDumpParts(keyDef) {
    return dumpParts(keyDef)
}

export function tValidateKey(keyDef, key) {
    return validateKey(keyDef, key)
}

export function tValidateFullKey(keyDef, key) {
    return validateFullKey(keyDef, key)
}

export function tPartCount(keyDef) {
    return partCount(keyDef)
}

export function tExtractKey(keyDef, tuple) {
    return extractKey(keyDef, tuple)
}

export function tValidateTuple(keyDef, tuple) {
    return validateTuple(keyDef, tuple)
}

export function decodeFromRawPtr(ptr) {
    return decode(ptr)
}

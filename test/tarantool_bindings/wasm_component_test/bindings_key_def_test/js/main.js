import {
    new as keyDefNew,
    duplicate,
    merge,
    release,
    dumpParts,
    validateFullKey,
    validateKey,
    partCount,
    extractKey,
    validateTuple,
} from 'tarantool:tarantool/key-def@0.2.0';
import { new as boxTupleNew } from 'tarantool:tarantool/box-tuple@0.2.0';

export function tTupleNew(data) { return boxTupleNew(data) }
export function tNew(parts) { return keyDefNew(parts) }
export function tDuplicate(keyDef) { return duplicate(keyDef) }
export function tMerge(left, right) { return merge(left, right) }
export function tRelease(keyDef) { release(keyDef) }
export function tDumpParts(keyDef) { return dumpParts(keyDef) }
export function tValidateKey(keyDef, key) { return validateKey(keyDef, key) }
export function tValidateFullKey(keyDef, key) { return validateFullKey(keyDef, key) }
export function tPartCount(keyDef) { return partCount(keyDef) }
export function tExtractKey(keyDef, tuple) { return extractKey(keyDef, tuple) }
export function tValidateTuple(keyDef, tuple) { return validateTuple(keyDef, tuple) }

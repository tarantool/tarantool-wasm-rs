import {
    new as boxTupleNew,
    retain,
    release,
    fieldCount,
    byteSize,
    toBytes,
    format,
    field,
    fieldByPath,
    update,
    upsert,
    validate,
    compare,
    compareWithKey,
    Iterator,
} from 'tarantool:tarantool/box-tuple@0.2.0';
import { new as keyDefNew } from 'tarantool:tarantool/key-def@0.2.0';
import { new as tupleFormatNew, default as tupleFormatDefault } from 'tarantool:tarantool/tuple-format@0.2.0';
import { fromJson, toJson } from 'tarantool:tarantool/msgpack@0.2.0';

export function tNew(data) {
    return boxTupleNew(data)
}

export function tRetain(tuple) {
    retain(tuple)
}

export function tRelease(tuple) {
    release(tuple)
}

export function tFieldCount(tuple) {
    return fieldCount(tuple)
}

export function tByteSize(tuple) {
    return byteSize(tuple)
}

export function tToBytes(tuple) {
    return toBytes(tuple)
}

export function tFormat(tuple) {
    return format(tuple)
}

export function tField(tuple, index) {
    return field(tuple, index)
}

export function tFieldByPath(tuple, path) {
    return fieldByPath(tuple, path)
}

export function tUpdate(tuple, expression) {
    return update(tuple, expression)
}

export function tUpsert(tuple, expression) {
    return upsert(tuple, expression)
}

export function tValidate(tuple, tupleFormat) {
    return validate(tuple, tupleFormat)
}

export function tCompare(keyDef, left, right) {
    return compare(keyDef, left, right)
}

export function tCompareWithKey(tuple, key, keyDef) {
    return compareWithKey(tuple, key, keyDef)
}

export function tKeyDefNew(parts) {
    return keyDefNew(parts)
}

export function tTupleFormatDefault() {
    return tupleFormatDefault()
}

export function tTupleFormatNew(keyDefs) {
    return tupleFormatNew(keyDefs)
}

function decoded(value) {
    return JSON.parse(toJson(value))
}

export function tIterTestAll(tuple) {
    const iterator = Iterator.new(tuple)
    const result = []
    try {
        while (true) {
            const position = iterator.position()
            const value = iterator.next()
            if (value === undefined) {
                break
            }
            result.push([position, decoded(value)])
        }

        iterator.rewind()
        const rewindPosition = iterator.position()
        const rewindValue = iterator.next()
        result.push(['rewind', rewindPosition, decoded(rewindValue)])

        const seekValue = iterator.seek(1)
        result.push(['seek', iterator.position(), decoded(seekValue)])
    } finally {
        iterator[Symbol.dispose]()
    }
    return fromJson(JSON.stringify(result))
}

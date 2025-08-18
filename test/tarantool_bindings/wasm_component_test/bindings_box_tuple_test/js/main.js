import { new as boxTupleNew, ref, unref, fieldCount, bsize, toBuf, format, field, fieldByPath, update, upsert, validate, compare, compareWithKey, TupleIterator } from 'tarantool:tarantool/box-tuple@0.1.2';
import { new as keyDefNew } from 'tarantool:tarantool/key-def@0.1.2';
import { new as tupleFormatNew, default as tupleFormatDefault } from 'tarantool:tarantool/tuple-format@0.1.2';
import { encode, decodeFromRawPtr } from 'tarantool:tarantool/msgpack@0.1.2';

export function tNew(data) {
    return boxTupleNew(data)
}

export function tRef(t) {
    return ref(t)
}

export function tUnref(t) {
    return unref(t)
}

export function tFieldCount(t) {
    return fieldCount(t)
}

export function tBsize(t) {
    return bsize(t)
}

export function tToBuf(t) {
    return toBuf(t)
}

export function tFormat(t) {
    return format(t)
}

export function tField(t, idx) {
    return field(t, idx)
}

export function tFieldByPath(t, path, indexBase) {
    return fieldByPath(t, path, indexBase)
}

export function tUpdate(t, expr) {
    return update(t, expr)
}

export function tUpsert(t, expr) {
    return upsert(t, expr)
}

export function tValidate(t, format) {
    return validate(t, format)
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
    console.log("Eeeeee1")
    return tupleFormatNew(keyDefs)
}

export function tDecodeFromRawPtr(ptr) {
    return decodeFromRawPtr(ptr)
}
  
export function tIterTestAll(t) {
    const iter = TupleIterator.newTupleIterator(t)

    const result = []

    // Iterate through all fields
    while (true) {
        const pos = iter.position()
        const field = iter.next()
        if (field.ptr == 0)
            break
        result.push([pos, field.ptr.toString()])
    }

    // Rewind and take next field again
    iter.rewind()
    const rewind_pos = iter.position()
    const rewind_field = iter.next()
    result.push(['rewind', rewind_pos, rewind_field.ptr.toString()])

    // Seek to 1
    const seek_field = iter.seek(1)
    const seek_pos = iter.position()
    result.push(['seek', seek_pos, seek_field.ptr.toString()])

    const jsonString = JSON.stringify(result);
    const jsonBytes = new TextEncoder().encode(jsonString);
    const encodedMsgPack = encode(jsonBytes);
    return encodedMsgPack;
}
    
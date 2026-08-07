import {
    schemaVersion,
    spaceByName,
    indexByName,
    insert,
    update,
    replace,
    upsert,
    delete as databaseDelete,
    truncate,
} from 'tarantool:tarantool/database@0.2.0';
import { toBytes, release } from 'tarantool:tarantool/box-tuple@0.2.0';

function tupleToBytes(tuple) {
    try {
        return toBytes(tuple)
    } finally {
        release(tuple)
    }
}

function optionalTupleToBytes(tuple) {
    return tuple === undefined || tuple === null ? undefined : tupleToBytes(tuple)
}

export function tSchemaVersion() {
    return schemaVersion()
}

export function tSpaceByName(name) {
    return spaceByName(name)
}

export function tIndexByName(space, name) {
    return indexByName(space, name)
}

export function tInsert(space, value) {
    return tupleToBytes(insert(space, value))
}

export function tUpdate(index, key, operations) {
    return optionalTupleToBytes(update(index, key, operations))
}

export function tReplace(space, value) {
    return tupleToBytes(replace(space, value))
}

export function tUpsert(index, value, operations) {
    upsert(index, value, operations)
}

export function tDelete(index, key) {
    return optionalTupleToBytes(databaseDelete(index, key))
}

export function tTruncate(space) {
    truncate(space)
}

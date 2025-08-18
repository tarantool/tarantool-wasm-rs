import {schemaVersion, spaceByName, indexByName, insert, update, replace, upsert, delete as ttDelete, truncate} from 'tarantool:tarantool/ttbox@0.1.2';
import { toBuf, unref } from 'tarantool:tarantool/box-tuple@0.1.2';

export function tSchemaVersion() {
    return schemaVersion();
}

export function tSpaceByName(name) {
    return spaceByName(name);
}

export function tIndexByName(space, name) {
    return indexByName(space, name);
}

export function tInsert(space, tup) {
    const tuple = insert(space, tup);
    const buf = toBuf(tuple);
    return buf;
}

export function tUpdate(index, key, ops) {
    const tuple = update(index, key, ops);
    const buf = toBuf(tuple);
    return buf;
}

export function tReplace(space, tup) {
    const tuple = replace(space, tup);
    const buf = toBuf(tuple);
    return buf;
}

export function tUpsert(index, tup, ops) {
    upsert(index, tup, ops);
}

export function tDelete(index, key) {
    const tuple = ttDelete(index, key);
    const buf = toBuf(tuple);
    return buf;
}

export function tTruncate(space) {
    truncate(space);
}

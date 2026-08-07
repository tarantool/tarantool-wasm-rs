import { tupleCount, memorySize, random, get, min, max, count } from 'tarantool:tarantool/index@0.2.0';
import { toBytes, release } from 'tarantool:tarantool/box-tuple@0.2.0';

function optionalTupleToBytes(tuple) {
    if (tuple === undefined || tuple === null) {
        return undefined
    }
    try {
        return toBytes(tuple)
    } finally {
        release(tuple)
    }
}

export function tLen(index) {
    return tupleCount(index)
}

export function tBsize(index) {
    return memorySize(index)
}

export function tRandom(index, rnd) {
    return optionalTupleToBytes(random(index, rnd))
}

export function tGet(index, key) {
    return optionalTupleToBytes(get(index, key))
}

export function tMin(index, key) {
    return optionalTupleToBytes(min(index, key))
}

export function tMax(index, key) {
    return optionalTupleToBytes(max(index, key))
}

export function tCount(index, iterType, key) {
    return count(index, iterType, key)
}

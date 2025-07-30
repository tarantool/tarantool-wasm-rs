import { len, bsize, random, get, min, max, count } from 'tarantool:tarantool/index@0.1.0';
import { toBuf } from 'tarantool:tarantool/box-tuple@0.1.0';

export function tLen(index) {
    return len(index)
}

export function tBsize(index) {
    return bsize(index)
}

export function tRandom(index, rnd) {
    return toBuf(random(index, rnd))
}

export function tGet(index, key) {
    return toBuf(get(index, key))
}

export function tMin(index, key) {
    return toBuf(min(index, key))
}

export function tMax(index, key) {
    return toBuf(max(index, key))
}

export function tCount(index, iterType, key) {
    return count(index, iterType, key)
}

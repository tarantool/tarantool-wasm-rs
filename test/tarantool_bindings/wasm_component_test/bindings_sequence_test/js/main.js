import { next, current, set, reset } from 'tarantool:tarantool/sequence@0.2.0';

export function tNext(seq) {
    return next(seq)
}

export function tCurrent(seq) {
    return current(seq)
}

export function tSet(seq, value) {
    return set(seq, value)
}

export function tReset(seq) {
    return reset(seq)
}

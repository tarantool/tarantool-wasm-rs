import { errorNew, set, clear, last, toString } from 'tarantool:tarantool/error@0.1.0';

export function tErrorNew(message, type, code, file, line) {
    return errorNew(message, type, code, file, line)
}

export function tSet(err) {
    set(err)
}

export function tLast() {
    return last()
}

export function tClear() {
    clear()
}

export function tToString(err) {
    return toString(err)
}

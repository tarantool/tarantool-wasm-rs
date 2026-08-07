import { new as errorNew, newWithLocation, set, clear, last, toString } from 'tarantool:tarantool/error@0.2.0';

export function tNew(code, message) {
    return errorNew(code, message)
}

export function tNewWithLocation(code, message, file, line) {
    return newWithLocation(code, message, file, line)
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

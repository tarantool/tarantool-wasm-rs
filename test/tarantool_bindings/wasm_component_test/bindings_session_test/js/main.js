import { current, broadcast, iprotoSend } from 'tarantool:tarantool/session@0.2.0';

export function tCurrent() {
    return current()
}

export function tBroadcast(key, value) {
    return broadcast(key, value)
}

export function tIprotoSend(session, header, body) {
    return iprotoSend(session, header, body)
}

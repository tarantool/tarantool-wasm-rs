import { write } from 'tarantool:tarantool/log@0.2.0';

export function tSay(level, message, context) {
    write(level, message, context)
}
export function tSayError(message, context) {
    write('error', message, context)
}
export function tSayCrit(message, context) {
    write('critical', message, context)
}
export function tSayWarn(message, context) {
    write('warning', message, context)
}
export function tSayInfo(message, context) {
    write('info', message, context)
}
export function tSayVerbose(message, context) {
    write('verbose', message, context)
}
export function tSayDebug(message, context) {
    write('debug', message, context)
}

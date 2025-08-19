import { say, sayError, sayCrit, sayWarn, sayInfo, sayVerbose, sayDebug, saySyserror } from 'tarantool:tarantool/say@0.1.2';

export function tSay(level, msg, ctx) {
    say(level, msg, ctx)
}

export function tSayError(msg, ctx) {
    sayError(msg, ctx)
}

export function tSayCrit(msg, ctx) {
    sayCrit(msg, ctx)
}

export function tSayWarn(msg, ctx) {
    sayWarn(msg, ctx)
}

export function tSayInfo(msg, ctx) {
    sayInfo(msg, ctx)
}

export function tSayVerbose(msg, ctx) {
    sayVerbose(msg, ctx)
}

export function tSayDebug(msg, ctx) {
    sayDebug(msg, ctx)
}

export function tSaySyserror(msg, ctx) {
    saySyserror(msg, ctx)
}

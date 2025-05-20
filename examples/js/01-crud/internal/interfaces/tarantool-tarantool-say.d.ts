/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/say@0.1.0' {
  export function say(level: LogLevel, msg: string, ctx: LogContext | undefined): void;
  export function sayError(msg: string, ctx: LogContext | undefined): void;
  export function sayCrit(msg: string, ctx: LogContext | undefined): void;
  export function sayWarn(msg: string, ctx: LogContext | undefined): void;
  export function sayInfo(msg: string, ctx: LogContext | undefined): void;
  export function sayVerbose(msg: string, ctx: LogContext | undefined): void;
  export function sayDebug(msg: string, ctx: LogContext | undefined): void;
  export function saySyserror(msg: string, ctx: LogContext | undefined): void;
  export type LogLevel = import('tarantool:tarantool/types@0.1.0').LogLevel;
  export type LogContext = import('tarantool:tarantool/types@0.1.0').LogContext;
}

/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/error@0.1.0' {
  export function errorNew(message: string, type: string, code: number, file: string | undefined, line: number | undefined): BoxError;
  export function set(err: BoxError): void;
  export function last(): BoxError | undefined;
  export function clear(): void;
  export function toString(err: BoxError): string;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
}

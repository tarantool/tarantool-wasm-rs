/// <reference path="./tarantool-tarantool-types.d.ts" />
declare module 'tarantool:tarantool/session@0.1.0' {
  export function current(): Session;
  export function broadcast(key: string, value: string): void;
  export function iprotoSend(session: Session, header: Uint8Array, body: Uint8Array): void;
  export type Session = import('tarantool:tarantool/types@0.1.0').Session;
  export type BoxError = import('tarantool:tarantool/types@0.1.0').BoxError;
}

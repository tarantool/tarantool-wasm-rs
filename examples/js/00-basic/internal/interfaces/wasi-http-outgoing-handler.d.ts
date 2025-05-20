/// <reference path="./wasi-http-types.d.ts" />
declare module 'wasi:http/outgoing-handler@0.2.0' {
  export function handle(request: OutgoingRequest, options: RequestOptions | undefined): FutureIncomingResponse;
  export type OutgoingRequest = import('wasi:http/types@0.2.0').OutgoingRequest;
  export type RequestOptions = import('wasi:http/types@0.2.0').RequestOptions;
  export type FutureIncomingResponse = import('wasi:http/types@0.2.0').FutureIncomingResponse;
  export type ErrorCode = import('wasi:http/types@0.2.0').ErrorCode;
}

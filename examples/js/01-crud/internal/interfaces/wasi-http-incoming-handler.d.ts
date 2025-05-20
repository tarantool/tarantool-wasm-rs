/// <reference path="./wasi-http-types.d.ts" />
declare module 'wasi:http/incoming-handler@0.2.0' {
  export function handle(request: IncomingRequest, responseOut: ResponseOutparam): void;
  export type IncomingRequest = import('wasi:http/types@0.2.0').IncomingRequest;
  export type ResponseOutparam = import('wasi:http/types@0.2.0').ResponseOutparam;
}

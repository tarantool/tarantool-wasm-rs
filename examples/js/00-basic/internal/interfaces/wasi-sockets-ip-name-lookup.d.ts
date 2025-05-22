/// <reference path="./wasi-io-poll.d.ts" />
/// <reference path="./wasi-sockets-network.d.ts" />
declare module 'wasi:sockets/ip-name-lookup@0.2.3' {
  export function resolveAddresses(network: Network, name: string): ResolveAddressStream;
  export type Pollable = import('wasi:io/poll@0.2.3').Pollable;
  export type Network = import('wasi:sockets/network@0.2.3').Network;
  export type ErrorCode = import('wasi:sockets/network@0.2.3').ErrorCode;
  export type IpAddress = import('wasi:sockets/network@0.2.3').IpAddress;
  
  export class ResolveAddressStream {
    /**
     * This type does not have a public constructor.
     */
    private constructor();
    resolveNextAddress(): IpAddress | undefined;
    subscribe(): Pollable;
  }
}

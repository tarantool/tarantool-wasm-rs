/// <reference path="./wasi-io-poll.d.ts" />
/// <reference path="./wasi-sockets-network.d.ts" />
declare module 'wasi:sockets/udp@0.2.0' {
  export type Pollable = import('wasi:io/poll@0.2.0').Pollable;
  export type Network = import('wasi:sockets/network@0.2.0').Network;
  export type ErrorCode = import('wasi:sockets/network@0.2.0').ErrorCode;
  export type IpSocketAddress = import('wasi:sockets/network@0.2.0').IpSocketAddress;
  export type IpAddressFamily = import('wasi:sockets/network@0.2.0').IpAddressFamily;
  export interface IncomingDatagram {
    data: Uint8Array,
    remoteAddress: IpSocketAddress,
  }
  export interface OutgoingDatagram {
    data: Uint8Array,
    remoteAddress?: IpSocketAddress,
  }
  
  export class IncomingDatagramStream {
    /**
     * This type does not have a public constructor.
     */
    private constructor();
    receive(maxResults: bigint): Array<IncomingDatagram>;
    subscribe(): Pollable;
  }
  
  export class OutgoingDatagramStream {
    /**
     * This type does not have a public constructor.
     */
    private constructor();
    checkSend(): bigint;
    send(datagrams: Array<OutgoingDatagram>): bigint;
    subscribe(): Pollable;
  }
  
  export class UdpSocket {
    /**
     * This type does not have a public constructor.
     */
    private constructor();
    startBind(network: Network, localAddress: IpSocketAddress): void;
    finishBind(): void;
    stream(remoteAddress: IpSocketAddress | undefined): [IncomingDatagramStream, OutgoingDatagramStream];
    localAddress(): IpSocketAddress;
    remoteAddress(): IpSocketAddress;
    addressFamily(): IpAddressFamily;
    unicastHopLimit(): number;
    setUnicastHopLimit(value: number): void;
    receiveBufferSize(): bigint;
    setReceiveBufferSize(value: bigint): void;
    sendBufferSize(): bigint;
    setSendBufferSize(value: bigint): void;
    subscribe(): Pollable;
  }
}

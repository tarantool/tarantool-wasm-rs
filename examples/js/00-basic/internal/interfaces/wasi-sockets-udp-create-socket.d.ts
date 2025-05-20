/// <reference path="./wasi-sockets-network.d.ts" />
/// <reference path="./wasi-sockets-udp.d.ts" />
declare module 'wasi:sockets/udp-create-socket@0.2.0' {
  export function createUdpSocket(addressFamily: IpAddressFamily): UdpSocket;
  export type Network = import('wasi:sockets/network@0.2.0').Network;
  export type ErrorCode = import('wasi:sockets/network@0.2.0').ErrorCode;
  export type IpAddressFamily = import('wasi:sockets/network@0.2.0').IpAddressFamily;
  export type UdpSocket = import('wasi:sockets/udp@0.2.0').UdpSocket;
}

/// <reference path="./wasi-sockets-network.d.ts" />
/// <reference path="./wasi-sockets-tcp.d.ts" />
declare module 'wasi:sockets/tcp-create-socket@0.2.3' {
  export function createTcpSocket(addressFamily: IpAddressFamily): TcpSocket;
  export type Network = import('wasi:sockets/network@0.2.3').Network;
  export type ErrorCode = import('wasi:sockets/network@0.2.3').ErrorCode;
  export type IpAddressFamily = import('wasi:sockets/network@0.2.3').IpAddressFamily;
  export type TcpSocket = import('wasi:sockets/tcp@0.2.3').TcpSocket;
}

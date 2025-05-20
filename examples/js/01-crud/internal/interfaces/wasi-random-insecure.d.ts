declare module 'wasi:random/insecure@0.2.0' {
  export function getInsecureRandomBytes(len: bigint): Uint8Array;
  export function getInsecureRandomU64(): bigint;
}

declare module 'docs:adder/zlib@0.1.0' {
  export function compress(data: Uint8Array, level: number): Uint8Array;
  export function decompress(data: Uint8Array): Uint8Array;
  export function crc32(data: Uint8Array): number;
  export function adler32(data: Uint8Array): number;
}

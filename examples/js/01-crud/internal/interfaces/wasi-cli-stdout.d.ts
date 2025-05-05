declare module 'wasi:cli/stdout@0.2.0' {
  export function getStdout(): OutputStream;
}
import type { OutputStream } from './wasi-io-streams.js';
export { OutputStream };

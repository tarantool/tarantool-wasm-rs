declare module 'wasi:cli/stdin@0.2.0' {
  export function getStdin(): InputStream;
}
import type { InputStream } from './wasi-io-streams.js';
export { InputStream };

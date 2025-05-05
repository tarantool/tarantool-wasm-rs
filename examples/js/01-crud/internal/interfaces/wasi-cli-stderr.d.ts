declare module 'wasi:cli/stderr@0.2.0' {
  export function getStderr(): OutputStream;
}
import type { OutputStream } from './wasi-io-streams.js';
export { OutputStream };

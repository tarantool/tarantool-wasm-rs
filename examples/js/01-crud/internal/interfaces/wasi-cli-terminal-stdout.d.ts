declare module 'wasi:cli/terminal-stdout@0.2.0' {
  /**
   * If stdout is connected to a terminal, return a `terminal-output` handle
   * allowing further interaction with it.
   */
  export function getTerminalStdout(): TerminalOutput | undefined;
}
import type { TerminalOutput } from './wasi-cli-terminal-output.js';
export { TerminalOutput };

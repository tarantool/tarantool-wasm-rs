declare module 'wasi:cli/terminal-stderr@0.2.0' {
  /**
   * If stderr is connected to a terminal, return a `terminal-output` handle
   * allowing further interaction with it.
   */
  export function getTerminalStderr(): TerminalOutput | undefined;
}
import type { TerminalOutput } from './wasi-cli-terminal-output.js';
export { TerminalOutput };

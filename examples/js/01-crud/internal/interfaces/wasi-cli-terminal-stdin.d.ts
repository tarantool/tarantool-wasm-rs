/// <reference path="./wasi-cli-terminal-input.d.ts" />
declare module 'wasi:cli/terminal-stdin@0.2.3' {
  export function getTerminalStdin(): TerminalInput | undefined;
  export type TerminalInput = import('wasi:cli/terminal-input@0.2.3').TerminalInput;
}

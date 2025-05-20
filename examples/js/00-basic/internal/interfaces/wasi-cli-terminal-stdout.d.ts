/// <reference path="./wasi-cli-terminal-output.d.ts" />
declare module 'wasi:cli/terminal-stdout@0.2.0' {
  export function getTerminalStdout(): TerminalOutput | undefined;
  export type TerminalOutput = import('wasi:cli/terminal-output@0.2.0').TerminalOutput;
}

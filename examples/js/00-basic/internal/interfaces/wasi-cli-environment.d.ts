declare module 'wasi:cli/environment@0.2.0' {
  export function getEnvironment(): Array<[string, string]>;
  export function getArguments(): Array<string>;
  export function initialCwd(): string | undefined;
}

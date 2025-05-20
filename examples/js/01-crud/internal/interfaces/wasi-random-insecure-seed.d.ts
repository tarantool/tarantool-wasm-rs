declare module 'wasi:random/insecure-seed@0.2.0' {
  export function insecureSeed(): [bigint, bigint];
}

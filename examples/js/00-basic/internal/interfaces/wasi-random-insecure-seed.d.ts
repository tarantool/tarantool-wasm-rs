declare module 'wasi:random/insecure-seed@0.2.3' {
  export function insecureSeed(): [bigint, bigint];
}

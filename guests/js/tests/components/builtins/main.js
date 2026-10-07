// Capability probe: deliberately no SDK or optional host imports.
export function features() {
  return JSON.stringify(Object.fromEntries(
      [
        'CompressionStream',
        'DecompressionStream',
        'fetch',
        'WebSocket',
        'TextEncoder',
        'ReadableStream',
      ].map((name) => [name, typeof globalThis[name]])
          .concat([['Symbol.dispose', typeof Symbol.dispose]])));
}
export async function compress(format, data) {
  const stream =
      new Blob([data]).stream().pipeThrough(new CompressionStream(format));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}
export async function decompress(format, data) {
  const stream =
      new Blob([data]).stream().pipeThrough(new DecompressionStream(format));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}
export async function fetchUrl(url) {
  const response = await fetch(url);
  return `${response.status}:${await response.text()}`;
}

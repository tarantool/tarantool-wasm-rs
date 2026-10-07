// For import/packaging checks ONLY. Every host call fails, including at import.
const functions = [
  'schemaVersion', 'spaceByName',
  'indexByName',   'insert',
  'replace',       'update',
  'upsert',        'delete',
  'truncate',      'new',
  'default',       'retain',
  'release',       'fieldCount',
  'byteSize',      'toBytes',
  'format',        'field',
  'fieldByPath',   'validate',
  'compare',       'compareWithKey',
  'tupleCount',    'memorySize',
  'random',        'get',
  'min',           'max',
  'count',         'duplicate',
  'merge',         'dumpParts',
  'validateKey',   'validateFullKey',
  'partCount',     'extractKey',
  'validateTuple', 'newWithLocation',
  'set',           'last',
  'clear',         'toString',
  'write',         'uptime',
  'fromJson',      'toJson',
  'next',          'current',
  'reset',         'broadcast',
  'iprotoSend',    'id',
  'isolation',     'begin',
  'commit',        'rollback',
  'setIsolation',  'makeSynchronous',
];
export function resolve(specifier, context, nextResolve) {
  if (specifier.startsWith('docs:adder/')) {
    throw Error('base SDK unexpectedly imports ' + specifier);
  }
  if (!specifier.startsWith('tarantool:tarantool/')) {
    return nextResolve(specifier, context);
  }
  const src =
      `function unavailable(){throw Error('host call during module import')}\n` +
      functions
          .map(
              (name, i) =>
                  `const f${i}=unavailable; export {f${i} as ${name}};`)
          .join('\n') +
      '\nexport class Iterator { static new(){unavailable()} }';
  return {
    url: 'data:text/javascript,' + encodeURIComponent(src),
    shortCircuit: true,
  };
}

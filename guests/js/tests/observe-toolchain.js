import {execFileSync} from 'node:child_process';
import {existsSync, readFileSync, realpathSync, writeFileSync} from 'node:fs';
import {dirname, join} from 'node:path';

const jcoRoot = dirname(dirname(
    realpathSync(execFileSync('which', ['jco'], {encoding: 'utf8'}).trim())));
const read = (root) => JSON.parse(readFileSync(join(root, 'package.json')));
const dependency = (name, from) => {
  let dir = from;
  while (true) {
    const root = join(dir, 'node_modules', name);
    if (existsSync(join(root, 'package.json'))) {
      return root;
    }
    if (dirname(dir) === dir) {
      throw new Error('Missing toolchain package: ' + name);
    }
    dir = dirname(dir);
  }
};
const componentizer = dependency('@bytecodealliance/componentize-js', jcoRoot);
const observed = {
  node: process.versions.node,
  jco: read(jcoRoot).version,
  componentize_js: read(componentizer).version,
  preview2_shim:
      read(dependency('@bytecodealliance/preview2-shim', componentizer))
          .version,
  rolldown: read(dependency('rolldown', jcoRoot)).version,
  jco_std: read(dependency('@bytecodealliance/jco-std', jcoRoot)).version,
  wasm_tools: execFileSync('wasm-tools', ['--version'], {encoding: 'utf8'})
                  .trim()
                  .split(' ')
                  .at(-1),
  wkg: execFileSync('wkg', ['--version'], {encoding: 'utf8'})
           .trim()
           .split(' ')
           .at(-1),
  componentize_py:
      execFileSync('componentize-py', ['--version'], {encoding: 'utf8'})
          .trim()
          .split(' ')
          .at(-1),
};
writeFileSync(
    '/workspace/guests/js/dist/toolchain-observed.json',
    JSON.stringify(observed, null, 2));

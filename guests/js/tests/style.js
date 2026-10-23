import {execFileSync} from 'node:child_process';
import {existsSync, readdirSync, readFileSync} from 'node:fs';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';

const ROOT = fileURLToPath(new URL('../', import.meta.url));
const FIX = process.argv.includes('--fix');
const files = ['eslint.config.js'];
for (const dir of ['src', 'tests/unit', 'tests/types', 'tests/components']) {
  collect(join(ROOT, dir));
}
collect(join(ROOT, 'tests'), false);

/**
 * Collects maintained JS and TypeScript sources, excluding generated bindings.
 */
function collect(dir, recursive = true) {
  for (const entry of readdirSync(dir, {withFileTypes: true})) {
    const path = join(dir, entry.name);
    if (entry.isFile() && /\.(?:js|ts)$/.test(entry.name)) {
      files.push(path);
    } else if (recursive && entry.isDirectory() && dir.endsWith('components')) {
      const main = join(path, 'main.js');
      if (existsSync(main)) {
        files.push(main);
      }
    }
  }
}

const eslint = join(ROOT, 'node_modules/.bin/eslint');
const formatter = join(ROOT, 'node_modules/.bin/clang-format');
execFileSync(
    eslint,
    [
      ...(FIX ? ['--fix'] : []),
      ...new Set(files.filter((file) => file.endsWith('.js'))),
    ],
    {cwd: ROOT, stdio: 'inherit'});
for (const file of new Set(files)) {
  if (FIX) {
    execFileSync(formatter, ['-i', file], {cwd: ROOT, stdio: 'inherit'});
  } else {
    const formatted = execFileSync(formatter, [file], {cwd: ROOT});
    if (!formatted.equals(readFileSync(file))) {
      throw new Error(`Google style mismatch: ${file}; run npm run format`);
    }
  }
}

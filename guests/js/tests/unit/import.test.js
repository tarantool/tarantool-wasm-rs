import {execFileSync} from 'node:child_process';
import {test} from 'node:test';
import {fileURLToPath} from 'node:url';

test(
    'base SDK import executes no host calls and has no optional imports',
    () => {
      execFileSync(
          process.execPath,
          [
            '--import',
            fileURLToPath(new URL('./wit-loader.js', import.meta.url)),
            fileURLToPath(new URL('./import-consumer.js', import.meta.url)),
          ],
          {
            cwd: fileURLToPath(new URL('../../', import.meta.url)),
            stdio: 'pipe',
          });
    });

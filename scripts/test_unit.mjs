import { readdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// Pass explicit files so discovery works equally in Windows and Unix shells.
const root = fileURLToPath(new URL('../', import.meta.url));
const files = readdirSync(new URL('../tests/', import.meta.url))
  .filter(name => name.endsWith('.test.ts')).sort().map(name => `tests/${name}`);
if (!files.length) throw new Error('No TypeScript unit tests were found.');
const result = spawnSync(process.execPath, ['--experimental-strip-types', '--test', ...files], { cwd: root, stdio: 'inherit' });
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;

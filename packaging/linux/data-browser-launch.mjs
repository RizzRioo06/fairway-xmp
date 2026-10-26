// Invoked by the native CLI. Setup is serialized there with flock.
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';

const payload = dirname(fileURLToPath(import.meta.url));
const cache = process.env.FAIRWAY_DATA_BROWSER_CACHE;
const ready = join(cache, '.ready');
const prepare = process.argv[2] === '--prepare';
const require = createRequire(join(cache, 'package.json'));

function verify() {
  for (const name of ['package.json', 'package-lock.json', 'server.mjs', 'sqlite-query.mjs']) {
    if (!readFileSync(join(payload, name)).equals(readFileSync(join(cache, name)))) {
      throw new Error('Cached browser files are incomplete.');
    }
  }
  const Database = require('better-sqlite3');
  const db = new Database(':memory:');
  try { db.prepare('select 1').get(); } finally { db.close(); }
  require('pg'); require('mysql2/promise'); require('drizzle-orm/better-sqlite3');
}

if (prepare) {
  if (existsSync(ready)) {
    try { verify(); process.exit(0); } catch { rmSync(ready); }
  }
  mkdirSync(cache, { recursive: true, mode: 0o700 });
  for (const name of ['package.json', 'package-lock.json', 'server.mjs', 'sqlite-query.mjs']) {
    copyFileSync(join(payload, name), join(cache, name));
  }
  console.log('Setting up data browser dependencies for this Node version (internet required)…');
  const result = spawnSync('npm', ['ci', '--omit=dev', '--ignore-scripts', '--no-audit', '--no-fund', '--update-notifier=false'], {
    cwd: cache, stdio: 'inherit',
    env: { ...process.env, PATH: `${dirname(process.execPath)}:${process.env.PATH ?? ''}` },
  });
  if (result.error || result.status !== 0) {
    console.error('Data browser setup failed. Check your connection and npm installation, then retry fairway data-browser.');
    if (result.error) console.error(result.error.message);
    process.exit(1);
  }
  try { verify(); } catch (error) {
    console.error(`Data browser dependencies could not load: ${error.message}`);
    console.error('Try a supported Node LTS version using fnm, then retry.');
    process.exit(1);
  }
  writeFileSync(ready, 'ready\n', { mode: 0o600 });
} else {
  if (!existsSync(ready)) throw new Error('Data browser setup is incomplete. Retry fairway data-browser.');
  await import(pathToFileURL(join(cache, 'server.mjs')).href);
}

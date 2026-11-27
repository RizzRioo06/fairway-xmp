// Local desktop launcher. All profile writes stay under the selected repository.
import { constants, closeSync, existsSync, fsyncSync, lstatSync, mkdirSync, openSync, readSync, fstatSync, writeFileSync } from 'node:fs';
import { delimiter, join, resolve } from 'node:path';
import { homedir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

const markerName = '.fairway-desktop-dev.json';
function owned(stat) { return !process.getuid || stat.uid === process.getuid(); }
function directory(path, privateMode = false) {
  if (!existsSync(path)) {
    // mkdir is deliberately non-recursive: an existing alias is never traversed.
    try { mkdirSync(path, { mode: 0o700 }); } catch (error) { if (error.code !== 'EEXIST') throw error; }
  }
  const stat = lstatSync(path);
  if (!stat.isDirectory() || stat.isSymbolicLink() || !owned(stat) || privateMode && process.platform !== 'win32' && (stat.mode & 0o077) !== 0) {
    throw new Error('Development profile path is not an owned, safe directory. Existing files were kept.');
  }
}
export function prepareProfile(root) {
  const work = join(root, '.runtime-work'); directory(work);
  const profile = join(work, 'desktop-dev');
  let created = false;
  try { mkdirSync(profile, { mode: 0o700 }); created = true; } catch (error) { if (error.code !== 'EEXIST') throw error; }
  directory(profile, true);
  const marker = join(profile, markerName);
  if (!created) {
    const before = lstatSync(marker);
    if (!before.isFile() || before.isSymbolicLink() || !owned(before) || before.nlink !== 1 || before.size > 1024) throw new Error('Unsafe development profile marker. Existing files were kept.');
    const fd = openSync(marker, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
    try {
      const actual = fstatSync(fd);
      if (before.dev !== actual.dev || before.ino !== actual.ino || !actual.isFile() || actual.nlink !== 1 || actual.size > 1024) throw new Error('Development profile marker changed.');
      const buffer = Buffer.alloc(1025);
      const size = readSync(fd, buffer, 0, buffer.length, 0);
      if (size > 1024) throw new Error('Development profile marker is oversized.');
      const value = JSON.parse(buffer.subarray(0, size).toString('utf8'));
      if (value.kind !== 'fairway-desktop-dev' || value.version !== 1) throw new Error('Unrecognized development profile. Existing files were kept.');
    } finally { closeSync(fd); }
  }
  directory(join(profile, 'home'), true); directory(join(profile, 'data'), true);
  if (created) {
    const fd = openSync(marker, 'wx', 0o600);
    try { writeFileSync(fd, JSON.stringify({ kind: 'fairway-desktop-dev', version: 1 }) + '\n'); fsyncSync(fd); } finally { closeSync(fd); }
    if (process.platform !== 'win32') {
      const fd = openSync(profile, 'r'); try { fsyncSync(fd); } finally { closeSync(fd); }
    }
  }
  return profile;
}
export function existingAcceptanceProfile(root, name) {
  if (!/^pg-(?:ui|gui)-[A-Za-z0-9_-]+$/.test(name)) throw new Error('Choose an existing pg-ui-* or pg-gui-* profile name, without a path.');
  const profile = join(root, '.runtime-work', name);
  // Read-only selection: no directory creation, adoption, migration or vault access.
  for (const path of [join(root, '.runtime-work'), profile, join(profile, 'home'), join(profile, 'data')]) {
    const stat = lstatSync(path);
    if (!stat.isDirectory() || stat.isSymbolicLink() || !owned(stat)) throw new Error('Existing profile is not an owned, safe directory.');
  }
  return profile;
}
export function launchSpec(root, profile, inherited = process.env) {
  const env = { ...inherited, FAIRWAY_DEV_HOME: join(profile, 'home'), FAIRWAY_DEV_DATA: join(profile, 'data'), ZDOTDIR: join(profile, 'home') };
  const pathKey = Object.keys(env).find(key => key.toLowerCase() === 'path') ?? 'PATH';
  env[pathKey] = [join(homedir(), '.cargo', 'bin'), env[pathKey] ?? ''].filter(Boolean).join(delimiter);
  const config = {
    bundle: { resources: [] }, identifier: /^pg-(ui|gui)-/.test(profile.split(/[/\\]/).at(-1)) ? 'dev.fairway.personal.pgacceptance' : 'dev.fairway.personal.localdev',
    build: { beforeDevCommand: 'npm run dev -- --port 1440 --strictPort', devUrl: 'http://127.0.0.1:1440' },
  };
  return { command: process.execPath, args: [join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js'), 'dev', '--config', JSON.stringify(config)], cwd: root, env };
}
function main() {
  const args = process.argv.slice(2);
  let previous;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--check') continue;
    if (args[i] === '--profile' && !previous && args[i + 1]) { previous = args[++i]; continue; }
    throw new Error('Usage: npm run dev:desktop [-- --check] [-- --profile pg-ui-NAME]');
  }
  const root = fileURLToPath(new URL('../', import.meta.url));
  if (!existsSync(join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js'))) throw new Error('Run npm ci in this repository first.');
  const profile = previous ? existingAcceptanceProfile(root, previous) : prepareProfile(root);
  const spec = launchSpec(root, profile);
  console.log(`Fairway desktop development — isolated profile: .runtime-work/${previous ?? 'desktop-dev'}`);
  console.log('Saved connections and test settings are reused on the next launch.');
  if (args.includes('--check')) { console.log('Launcher configuration checked; no app or server started.'); return; }
  const child = spawn(spec.command, spec.args, { cwd: spec.cwd, env: spec.env, stdio: 'inherit', shell: false });
  // The terminal sends signals to the shared foreground process group. Let Tauri
  // shut down its own dev server/app before this wrapper exits.
  const interrupted = () => {};
  const terminated = () => { child.kill('SIGTERM'); };
  process.on('SIGINT', interrupted); process.on('SIGTERM', terminated);
  child.on('error', () => { console.error('Desktop launcher could not start. Check Node, Rust and the local Tauri installation.'); process.exitCode = 1; });
  child.on('close', (code, signal) => {
    process.off('SIGINT', interrupted); process.off('SIGTERM', terminated);
    process.exitCode = code ?? (signal === 'SIGINT' ? 130 : 1);
  });
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); } catch (error) { console.error(error.message); process.exitCode = 1; }
}

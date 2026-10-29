// Controlled fake npm for native supervisor regressions. Never used by IPC.
const fs = require('node:fs');
if (process.argv[2] === '--version') { console.log('10.9.0'); process.exit(0); }
if (process.argv[2] !== 'ci' || !process.argv.includes('--ignore-scripts') || !process.argv.includes('--strict-ssl=true')) throw Error('Unsafe fixture invocation');
if (process.env.NODE_OPTIONS || process.env.NODE_PATH || process.env.NPM_TOKEN || process.env.npm_config_registry) throw Error('Inherited tool configuration');
const mode = '__MODE__';
fs.mkdirSync('node_modules');
fs.writeFileSync('node_modules/fixture.json', JSON.stringify({ node: process.execPath, version: process.version, home: process.env.HOME, cache: process.env.npm_config_cache }));
console.log('Installer café 日本\nsecond line');
if (mode === 'stubborn') {
  process.on('SIGTERM', () => {});
  const child = require('node:child_process').spawn(process.execPath, ['-e', 'process.on("SIGTERM",()=>{});setInterval(()=>{},1000)'], {stdio:'inherit'});
  fs.writeFileSync('node_modules/fixture-pids.json', JSON.stringify([process.pid, child.pid]));
  setInterval(() => {}, 1000);
} else if (mode === 'slow-success') {
  setTimeout(() => process.exit(0), 2000);
} else if (mode === 'fail') {
  console.error('Controlled registry error'); process.exit(7);
} else if (mode === 'flood') {
  process.stdout.write('x'.repeat(12 * 1024 * 1024), () => process.stderr.write('日本\nend\n'));
} else if (mode === 'replace-src') {
  fs.renameSync('src', 'original-src'); fs.mkdirSync('src'); fs.writeFileSync('src/index.js', 'unreviewed');
} else if (mode === 'touch-source') {
  fs.appendFileSync('package.json', ' ');
}
if (mode === 'late-child' || mode === 'late-stubborn') {
  const code = `const fs=require('node:fs');
    process.on('SIGTERM',()=>{${mode === 'late-stubborn' ? '' : "setTimeout(()=>{fs.writeFileSync('node_modules/after-cleanup.txt','cleanup finished 日本');process.exit(0)},650)"}});
    setInterval(()=>{},1000);console.log('CHILD_READY');`;
  const child = require('node:child_process').spawn(process.execPath, ['-e', code], {stdio:['ignore','pipe','inherit']});
  child.stdout.once('data', () => {
    fs.writeFileSync('node_modules/fixture-pids.json', JSON.stringify([process.pid, child.pid]));
    process.exit(0);
  });
}

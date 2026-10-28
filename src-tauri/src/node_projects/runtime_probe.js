// A fixed tool probe. Never require the imported package or evaluate its scripts.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const [npmRoot, json] = process.argv.slice(1);
const input = JSON.parse(json);
const fail = (code, message) => { throw Object.assign(new Error(message), { code }); };
try {
  if (process.version !== input.version || fs.realpathSync(process.execPath) !== input.nodePath) {
    fail('runtimeChanged', 'The selected Node executable or exact version changed. Inspect and select the runtime again.');
  }
  const entry = fs.realpathSync(require.resolve(path.join(npmRoot, 'node_modules/semver')));
  if (!entry.startsWith(npmRoot + path.sep) || entry.startsWith(input.root + path.sep)) {
    fail('npmUnavailable', 'npm must include its own semver dependency.');
  }
  const semver = require(entry);
  if (!semver.valid(input.npmVersion) || typeof semver.validRange !== 'function') {
    fail('npmUnavailable', 'The paired npm installation is incomplete.');
  }
  for (const requirement of input.requirements) {
    const range = typeof requirement.range === 'string' ? requirement.range.trim() : '';
    if (!range || range.length > 4096 || !semver.validRange(range)) {
      fail('requirementConflict', `${requirement.source} needs a supported npm semver range; aliases are not resolved automatically.`);
    }
    if (!semver.satisfies(requirement.kind === 'npm' ? input.npmVersion : process.version, range)) {
      fail('requirementConflict', `The selected ${requirement.kind} version does not satisfy ${requirement.source}: ${range}`);
    }
  }
  process.stdout.write(JSON.stringify({ nodePath: fs.realpathSync(process.execPath), version: process.version, npmVersion: input.npmVersion }));
} catch (error) {
  const code = ['runtimeChanged', 'requirementConflict', 'npmUnavailable'].includes(error.code) ? error.code : 'npmUnavailable';
  process.stdout.write(JSON.stringify({ error: { code, message: code === 'npmUnavailable' ? 'Could not validate the paired npm installation and its dependencies.' : error.message } }));
}

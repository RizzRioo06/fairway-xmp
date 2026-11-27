import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtempSync, readFileSync, writeFileSync, rmSync, symlinkSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
// @ts-expect-error The launcher is a dependency-free Node ESM entry point.
import { prepareProfile, existingAcceptanceProfile, launchSpec } from '../scripts/dev_desktop.mjs';
function fixture() { const root = mkdtempSync(join(tmpdir(), 'fairway-launcher-')); return { root, dispose: () => rmSync(root, { recursive: true, force: true }) }; }
test('desktop launcher reuses its owned profile without overwriting saved test data', () => {
  const f = fixture(); try {
    const profile = prepareProfile(f.root); writeFileSync(join(profile, 'data', 'fixture.json'), 'retained');
    assert.equal(prepareProfile(f.root), profile); assert.equal(readFileSync(join(profile, 'data', 'fixture.json'), 'utf8'), 'retained');
    const spec = launchSpec(f.root, profile, { PATH: '/fixture/bin' });
    assert.equal(spec.env.FAIRWAY_DEV_DATA, join(profile, 'data')); assert.equal(spec.env.ZDOTDIR, join(profile, 'home'));
    const config = JSON.parse(spec.args.at(-1)); assert.equal(config.identifier, 'dev.fairway.personal.localdev');
    assert.deepEqual(config.bundle.resources, []); assert.equal(config.build.devUrl, 'http://127.0.0.1:1440');
  } finally { f.dispose(); }
});
test('desktop launcher refuses an unmarked existing profile', () => {
  const f = fixture(); try {
    mkdirSync(join(f.root, '.runtime-work')); mkdirSync(join(f.root, '.runtime-work', 'desktop-dev'), { mode: 0o700 });
    assert.throws(() => prepareProfile(f.root));
  } finally { f.dispose(); }
});
test('desktop launcher refuses profile path aliases without touching their destination', { skip: process.platform === 'win32' }, () => {
  const f = fixture(); try {
    const outside = join(f.root, 'retained'); mkdirSync(outside); writeFileSync(join(outside, 'fixture'), 'retained');
    symlinkSync(outside, join(f.root, '.runtime-work'));
    assert.throws(() => prepareProfile(f.root)); assert.equal(readFileSync(join(outside, 'fixture'), 'utf8'), 'retained');
  } finally { f.dispose(); }
});

test('existing acceptance profiles can be reopened without migrating or overwriting their data', () => {
  const f = fixture(); try {
    mkdirSync(join(f.root, '.runtime-work'));
    const profile = join(f.root, '.runtime-work', 'pg-ui-fixture');
    mkdirSync(profile, { mode: 0o700 }); mkdirSync(join(profile, 'home')); mkdirSync(join(profile, 'data'));
    writeFileSync(join(profile, 'data', 'fixture'), 'retained');
    assert.equal(existingAcceptanceProfile(f.root, 'pg-ui-fixture'), profile);
    assert.equal(readFileSync(join(profile, 'data', 'fixture'), 'utf8'), 'retained');
    assert.equal(JSON.parse(launchSpec(f.root, profile).args.at(-1)).identifier, 'dev.fairway.personal.pgacceptance');
    assert.throws(() => existingAcceptanceProfile(f.root, '../outside'));
    assert.throws(() => existingAcceptanceProfile(f.root, 'pg-ui-missing'));
  } finally { f.dispose(); }
});

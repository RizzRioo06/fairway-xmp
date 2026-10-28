import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { creationErrorCodes, isCreationError } from '../src/node-project-creation/types.ts';
import type { NodeTemplate, PlanCreationRequest, CreationPlanReference } from '../src/node-project-creation/types.ts';
test('creation error codes and native catalog preserve the read-only boundary', () => {
  for (const code of creationErrorCodes) assert.ok(isCreationError(JSON.parse(JSON.stringify({ code, message: 'Plan again.' }))));
  for (const value of [null, {}, 'failure', { code: 'futureCode', message: 'unknown' }, { code: 'planExpired', message: 3 }]) assert.equal(isCreationError(value), false);
  const catalog: { schemaVersion: number; templates: NodeTemplate[] } = JSON.parse(readFileSync(new URL('../src-tauri/src/node_project_creation/catalog.json', import.meta.url), 'utf8'));
  assert.equal(catalog.schemaVersion, 1);
  assert.deepEqual(catalog.templates.map(template => template.id), ['node-js', 'node-ts', 'hono-ts', 'express-js', 'react-vite-js', 'react-vite-ts']);
  for (const template of catalog.templates) {
    assert.equal(template.qualification, 'planningOnly');
    assert.equal(template.lockfileStatus, 'notPrepared');
    assert.equal(template.packageManager.name, 'npm');
    assert.ok(template.intendedFiles.includes('package.json'));
    for (const dependency of template.dependencies) assert.match(dependency.version, /^\d+\.\d+\.\d+$/);
  }
});
test('planning requests use runtime IDs and validation accepts references, not editable plan bodies', () => {
  const request: PlanCreationRequest = { requestId: 'a'.repeat(32), catalogRevision: 'b'.repeat(64), templateId: 'node-js', projectName: 'example', parent: '/tmp/日本 folder', runtimeId: 'c'.repeat(64), packageManager: 'npm' };
  assert.equal('nodePath' in request, false);
  const reference: CreationPlanReference = { planId: 'd'.repeat(32), digest: 'e'.repeat(64) };
  assert.deepEqual(Object.keys(reference), ['planId', 'digest']);
});

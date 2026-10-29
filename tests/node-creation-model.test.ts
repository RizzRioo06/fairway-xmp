import assert from 'node:assert/strict';
import test from 'node:test';
import { complete, creationError, mergeJob, nameError, needsReview, phaseLabel, planExpired, recovery } from '../src/node-project-creation/model.ts';
import type { CreationJobStatus, NodeCreationPlan } from '../src/node-project-creation/types.ts';
const job = (): CreationJobStatus => ({ jobId: 'a'.repeat(32), revision: 3, phase: 'ready', destination: '/tmp/日本 folder/app', stagingPath: '/tmp/owned', projectId: null, error: null, active: true, recoveryRequired: false, installation: { jobId: 'b'.repeat(32), creationId: 'a'.repeat(32), runId: 'c'.repeat(32), revision: 3, state: 'installing', progress: 'verifying', runtime: { nodePath: '/node22/bin/node', version: 'v22.23.4' }, lockfileSha256: 'd'.repeat(64), registry: 'https://registry.npmjs.org/', ciCompleted: false, cancelRequested: false, exitCode: null, error: null } });
test('name hints and review expiration never grant execution permission', () => {
  for (const name of ['', 'Hello', '../x', '-name', 'has space', '日本', 'trailing-', 'a'.repeat(65)]) assert.ok(nameError(name));
  for (const name of ['app', 'node-22', 'a'.repeat(64)]) assert.equal(nameError(name), null);
  const plan = { expiresAt: 100 } as NodeCreationPlan;
  assert.equal(planExpired(plan, 99), false); assert.equal(planExpired(plan, 100), true);
  assert.ok(needsReview({ code: 'planExpired', message: 'Expired' }));
  assert.equal(needsReview({ code: 'installationFailed', message: 'Failed' }), false);
});
test('only a published and registered completed setup is success', () => {
  const value = job(); value.installation!.state = 'completed'; value.installation!.ciCompleted = true;
  assert.equal(complete(value), false);
  value.phase = 'completed'; assert.equal(complete(value), false);
  value.projectId = value.jobId; assert.equal(complete(value), true);
  value.installation!.state = 'failed'; assert.equal(complete(value), false);
});
test('native phases are displayed without inferring percentages or readiness', () => {
  const value = job(); assert.equal(phaseLabel(value), 'Verifying project files and dependencies');
  value.phase = 'needsRegistration'; assert.equal(phaseLabel(value), 'Registering project');
  value.installation!.state = 'interrupted'; assert.equal(phaseLabel(value), 'Setup interrupted');
  value.installation!.state = 'cancelled'; assert.equal(phaseLabel(value), 'Setup cancelled');
  value.installation!.state = 'installing'; value.installation!.cancelRequested = true; assert.equal(phaseLabel(value), 'Cancelling and cleaning up');
});
test('recovery requires durable completed ci; no destructive cleanup is offered', () => {
  const value = job(); assert.equal(recovery(value), null);
  value.active = false; value.installation!.state = 'interrupted'; assert.equal(recovery(value), null);
  value.installation!.ciCompleted = true; assert.equal(recovery(value), 'finalize');
  value.installation = null; value.phase = 'needsRegistration'; assert.equal(recovery(value), 'retryRegistration');
  value.phase = 'staging'; assert.equal(recovery(value), null);
});
test('old revisions and mixed installation/run identities cannot replace current state', () => {
  const value = job(); let incoming = structuredClone(value); incoming.revision--; assert.equal(mergeJob(value, incoming), value);
  incoming = structuredClone(value); incoming.revision++; incoming.installation!.revision--; assert.equal(mergeJob(value, incoming), value);
  incoming = structuredClone(value); incoming.revision++; incoming.installation!.runId = 'old-run'; assert.equal(mergeJob(value, incoming), value);
  incoming = structuredClone(value); incoming.revision++; incoming.installation!.revision++; assert.equal(mergeJob(value, incoming), incoming);
});
test('typed errors preserve native details and actionable guidance without environments', () => {
  assert.match(creationError({ code: 'destinationExists', message: 'Occupied' }), /Existing files will be kept/);
  assert.match(creationError({ code: 'registrationFailed', message: 'Save failed' }), /Retry Registration/);
  assert.match(creationError({ code: 'invalidLockfile', message: 'Invalid' }), /No unpinned installation/);
  assert.match(creationError(new Error('Network unavailable')), /Network unavailable/);
});

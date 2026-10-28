import { expect, test, type Page } from '@playwright/test';
import type { NodeProjectsSnapshot, NodeRunsSnapshot } from '../../src/node-projects/types';
async function fixture(page: Page, options: Record<string, unknown>) {
  await page.evaluate(async value => { const path = '/src/ui-test/node-projects.ts'; (await import(path)).configure(value); }, options);
}
async function snapshot(page: Page): Promise<NodeProjectsSnapshot> {
  return page.evaluate(async () => { const path = '/src/ui-test/backend.ts'; return (await import(path)).invoke('node_projects_snapshot'); });
}
async function runs(page: Page): Promise<NodeRunsSnapshot> {
  return page.evaluate(async () => { const path = '/src/ui-test/backend.ts'; return (await import(path)).invoke('node_project_runs'); });
}
async function importProject(page: Page, name = 'First app', root = '/controlled/first 日本', script = 'dev', runtime = 'node-22') {
  await fixture(page, { root });
  await page.getByRole('button', { name: 'Import Node project', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Import Node project', exact: true });
  await dialog.getByRole('button', { name: 'Choose folder…' }).click();
  await dialog.getByLabel('Project name', { exact: true }).fill(name);
  await dialog.getByLabel('npm script', { exact: true }).selectOption(script);
  await dialog.getByLabel('Node runtime', { exact: true }).selectOption(runtime);
  await dialog.getByRole('button', { name: 'Save project', exact: true }).click();
  await expect(dialog).toBeHidden();
}
async function start(page: Page, name: string) {
  await page.getByRole('button', { name: `Start ${name}`, exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Start Node project?', exact: true });
  await expect(dialog).toContainText('lifecycle hooks');
  await dialog.getByRole('button', { name: 'Start project', exact: true }).click();
  await expect(dialog).toBeHidden();
}
test.beforeEach(async ({ page }) => {
  await page.goto('/'); await page.getByLabel('Example paths', { exact: true }).selectOption('macOS');
  await page.getByRole('button', { name: 'Node Projects', exact: true }).click();
  await expect(page.getByText('No Node projects yet', { exact: true })).toBeVisible();
});
test('imports, edits and removes registrations without executing scripts; dialogs support keyboard focus', async ({ page }) => {
  await importProject(page);
  expect((await runs(page)).runs).toHaveLength(0);
  expect((await snapshot(page)).projects).toHaveLength(1);
  const card = page.getByRole('listitem', { name: 'First app', exact: true });
  await expect(card).toContainText('v22.1.0'); await expect(card).toContainText('/controlled/first 日本');
  await card.getByRole('button', { name: 'Edit First app', exact: true }).click();
  const editor = page.getByRole('dialog', { name: 'Edit Node project', exact: true });
  await expect(editor.getByLabel('Project name', { exact: true })).toBeEnabled();
  await editor.getByRole('button', { name: 'Save project', exact: true }).focus();
  await page.keyboard.press('Tab');
  await expect(editor.getByLabel('Project folder', { exact: true })).toBeFocused();
  await editor.getByLabel('Project name', { exact: true }).fill('Renamed');
  await editor.getByLabel('npm script', { exact: true }).selectOption('build');
  await editor.getByLabel('Node runtime', { exact: true }).selectOption('node-24');
  await editor.getByLabel('Local URL (optional)', { exact: true }).fill('http://evil.example/');
  await expect(editor.getByRole('button', { name: 'Save project', exact: true })).toBeDisabled();
  await editor.getByLabel('Local URL (optional)', { exact: true }).fill('http://localhost:3000/');
  await editor.getByRole('button', { name: 'Save project', exact: true }).click();
  const renamed = page.getByRole('listitem', { name: 'Renamed', exact: true });
  await expect(renamed).toContainText('v24.1.0'); await expect(renamed).toContainText('build');
  await renamed.getByRole('button', { name: 'Remove Renamed', exact: true }).click();
  await page.keyboard.press('Escape');
  await expect(renamed.getByRole('button', { name: 'Remove Renamed', exact: true })).toBeFocused();
  await renamed.getByRole('button', { name: 'Remove Renamed', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Remove registration', exact: true }).click();
  await expect(renamed).toHaveCount(0); expect((await runs(page)).runs).toHaveLength(0);
});
test('runs concurrent versions, restarts with a new ID, keeps native ownership across navigation and isolates logs', async ({ page }) => {
  await importProject(page); await importProject(page, 'Second app', '/controlled/second', 'dev', 'node-24');
  await start(page, 'First app');
  await expect(page.getByRole('region', { name: 'Output for First app' })).toContainText('Node v22.1.0 café 日本');
  await expect(page.getByRole('region', { name: 'Output for First app' })).toContainText('<script>plain text</script>');
  await start(page, 'Second app');
  await expect(page.getByRole('region', { name: 'Output for Second app' })).toContainText('v24.1.0');
  expect((await runs(page)).runs.filter(run => run.state === 'running')).toHaveLength(2);
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'Start all', exact: true }).click();
  await page.getByRole('button', { name: 'Stop all', exact: true }).click();
  expect((await runs(page)).runs.filter(run => run.state === 'running')).toHaveLength(2);
  await page.getByRole('button', { name: 'Node Projects', exact: true }).click();
  const old = (await runs(page)).runs[0].runId;
  await page.getByRole('button', { name: 'Restart First app', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Restart project', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Output for First app' })).toContainText('v22.1.0');
  expect((await runs(page)).runs[0].runId).not.toBe(old);
  await page.getByRole('button', { name: 'Stop First app', exact: true }).click();
  await expect(page.getByRole('listitem', { name: 'First app', exact: true }).getByRole('status')).toHaveText('Stopped');
  expect((await runs(page)).runs.find(run => run.runtime.version === 'v24.1.0')?.state).toBe('running');
});
test('handles duplicate roots, malformed manifests, unsupported managers and missing runtimes', async ({ page }) => {
  await importProject(page);
  await fixture(page, { root: '/controlled/first 日本' });
  await page.getByRole('button', { name: 'Import Node project', exact: true }).click();
  let dialog = page.getByRole('dialog'); await dialog.getByRole('button', { name: 'Choose folder…' }).click();
  await dialog.getByRole('button', { name: 'Save project', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('already registered'); await page.keyboard.press('Escape');
  await fixture(page, { inspectionError: true });
  await page.getByRole('button', { name: 'Import Node project', exact: true }).click();
  dialog = page.getByRole('dialog'); await dialog.getByRole('button', { name: 'Choose folder…' }).click();
  await expect(dialog.getByRole('alert')).toContainText('valid manifest'); await page.keyboard.press('Escape');
  await fixture(page, { inspectionError: false, manager: 'pnpm' });
  await importProject(page, 'Other manager', '/controlled/other');
  await page.getByRole('button', { name: 'Start Other manager', exact: true }).click();
  await expect(page.getByRole('alert').first()).toContainText('supports npm only');
  expect((await runs(page)).runs).toHaveLength(0);
  await fixture(page, { missingRuntime: true });
  await expect(page.getByRole('button', { name: 'Start First app', exact: true })).toBeDisabled();
});
test('recovers from stale IDs and conflicts without automatically executing another run', async ({ page }) => {
  await importProject(page); await start(page, 'First app');
  await expect(page.getByRole('button', { name: 'Restart First app', exact: true })).toBeEnabled();
  const old = (await runs(page)).runs[0];
  await fixture(page, { staleStop: true }); await page.getByRole('button', { name: 'Stop First app', exact: true }).click();
  await expect(page.getByRole('alert').first()).toContainText('older run');
  expect((await runs(page)).runs[0].state).toBe('running');
  await page.evaluate(async run => { const path = '/src/ui-test/node-projects.ts'; (await import(path)).publish({ ...run, revision: 1, state: 'starting' }); }, old);
  await expect(page.getByRole('listitem', { name: 'First app', exact: true }).getByRole('status')).toHaveText('Running');
  await page.getByRole('button', { name: 'Stop First app', exact: true }).click();
  await page.getByRole('button', { name: 'Start First app', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.evaluate(async () => { const path = '/src/ui-test/node-projects.ts'; (await import(path)).conflict(); });
  await page.getByRole('dialog').getByRole('button', { name: 'Start project', exact: true }).click();
  await expect(page.getByRole('alert').first()).toContainText('registry changed');
  expect((await runs(page)).runs[0].state).toBe('stopped');
});
test('reports failed scripts, log truncation, clear view and retry without HTML execution', async ({ page }) => {
  await importProject(page, 'Failure app', '/controlled/failure', 'fail');
  await fixture(page, { truncated: true }); await start(page, 'Failure app');
  await expect(page.getByRole('listitem', { name: 'Failure app', exact: true }).getByRole('status')).toHaveText('Failed');
  await expect(page.getByText('Process exited with code 7.', { exact: true })).toBeVisible();
  await expect(page.getByText('Earlier output is no longer in this view. Only recent output is retained.', { exact: true })).toBeVisible();
  const output = page.getByRole('region', { name: 'Output for Failure app' }); await expect(output).toContainText('日本');
  expect(await output.locator('script').count()).toBe(0);
  await page.getByRole('button', { name: 'Clear view', exact: true }).click(); await expect(output).not.toContainText('日本');
  await expect(page.getByLabel('Follow output', { exact: true })).toBeChecked();
  await page.getByRole('button', { name: 'Maximize Project logs', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Project logs maximized', exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Restart Failure app', exact: true }).click();
  await fixture(page, { logsError: true });
  await page.getByRole('dialog').getByRole('button', { name: 'Restart project', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Retry logs', exact: true })).toBeVisible();
  await fixture(page, { logsError: false }); await page.getByRole('button', { name: 'Retry logs', exact: true }).click();
  await expect(output).toContainText('日本');
});
test('registry corruption preserves stop access and unsupported platforms show no execution controls', async ({ page }) => {
  await importProject(page); await start(page, 'First app');
  await fixture(page, { registryError: true });
  const recovery = page.getByRole('region', { name: 'Unlisted project runs', exact: true });
  await recovery.getByRole('button', { name: /^Stop unlisted / }).click();
  await expect.poll(async () => (await runs(page)).runs[0].state).toBe('stopped');
  await page.getByLabel('Example paths', { exact: true }).selectOption('Windows');
  await expect(page.getByText('Node Projects is available on macOS first. Your PHP services remain available.', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Import Node project', exact: true })).toBeDisabled();
});

test('log polling pauses when hidden, manual scrolling stays put and missing roots have actionable controls', async ({ page }) => {
  await importProject(page); await start(page, 'First app');
  await page.evaluate(async () => { const path = '/src/ui-test/node-projects.ts'; (await import(path)).flood(0); });
  const output = page.getByRole('region', { name: 'Output for First app' });
  await expect(output).toContainText('burst-499');
  await expect(page.getByText('Earlier output is no longer in this view. Only recent output is retained.', { exact: true })).toHaveCount(0);
  await output.evaluate(element => { element.scrollTop = 0; element.dispatchEvent(new Event('scroll')); });
  await expect(page.getByLabel('Follow output', { exact: true })).not.toBeChecked();
  await page.evaluate(async () => { const path = '/src/ui-test/node-projects.ts'; (await import(path)).flood(500); });
  await expect(output).toContainText('burst-999');
  expect(await output.evaluate(element => element.scrollTop)).toBe(0);
  await page.evaluate(async () => { const path = '/src/ui-test/node-projects.ts'; (await import(path)).flood(1000); });
  await expect(output).toContainText('burst-1499');
  await expect(page.getByText('Earlier output is no longer in this view. Only recent output is retained.', { exact: true })).toBeVisible();
  expect(await output.locator('.log-entry').count()).toBeLessThanOrEqual(1024);
  await page.evaluate(() => {
    const root = window as unknown as { nodeLogReads: number };
    root.nodeLogReads = 0;
    window.addEventListener('fairway-ui-command', event => { if ((event as CustomEvent).detail === 'node_project_logs') root.nodeLogReads++; });
    Object.defineProperty(document, 'hidden', { configurable: true, value: true }); document.dispatchEvent(new Event('visibilitychange'));
  });
  await page.waitForTimeout(1100); // Verify absence across two scheduled polling periods.
  expect(await page.evaluate(() => (window as unknown as { nodeLogReads: number }).nodeLogReads)).toBe(0);
  expect((await runs(page)).runs[0].state).toBe('running');
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: false }); document.dispatchEvent(new Event('visibilitychange')); });
  await expect.poll(() => page.evaluate(() => (window as unknown as { nodeLogReads: number }).nodeLogReads)).toBeGreaterThan(0);
  await page.getByRole('button', { name: 'Stop First app', exact: true }).click();
  await fixture(page, { missingRoot: true });
  await expect(page.getByRole('button', { name: 'Start First app', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Edit First app', exact: true })).toBeEnabled();
});

test('missing inventory and incompatible requirements never start a project or silently select another runtime', async ({ page }) => {
  await fixture(page, { noRuntimes: true });
  await page.getByRole('button', { name: 'Import Node project', exact: true }).click();
  let dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'Choose folder…' }).click();
  await expect(dialog.getByText('No installed Node runtimes found. Install a version in Settings, then refresh inspection.', { exact: true })).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Save project', exact: true })).toBeDisabled();
  await page.keyboard.press('Escape');
  await fixture(page, { noRuntimes: false, requirementConflict: true });
  await importProject(page);
  await expect(page.getByRole('alert').first()).toContainText('Runtime selection needs attention');
  expect((await runs(page)).runs).toHaveLength(0);
  await page.getByRole('button', { name: 'Start First app', exact: true }).click();
  dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'Start project', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('does not satisfy');
  expect((await runs(page)).runs).toHaveLength(0);
  expect((await snapshot(page)).projects[0].config.runtime.version).toBe('v22.1.0');
});

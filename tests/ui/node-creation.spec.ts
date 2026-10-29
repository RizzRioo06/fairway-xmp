import { expect, test, type Page } from '@playwright/test';
import type { CreationJobsSnapshot } from '../../src/node-project-creation/types';
async function configure(page: Page, options: Record<string, unknown>) { await page.evaluate(async value => { const path = '/src/ui-test/node-creation.ts'; (await import(path)).configure(value); }, options); }
async function stats(page: Page) { return page.evaluate(async () => { const path = '/src/ui-test/node-creation.ts'; return (await import(path)).stats() as { starts: number; plans: number; validations: number; cancels: number; recoveries: number }; }); }
async function jobs(page: Page): Promise<CreationJobsSnapshot> { return page.evaluate(async () => { const path = '/src/ui-test/backend.ts'; return (await import(path)).invoke('node_project_creation_jobs'); }); }
const dialog = (page: Page) => page.getByRole('dialog', { name: 'Create Node project', exact: true });
async function choose(page: Page, template = 'Plain Node.js JavaScript') {
  await page.getByRole('button', { name: 'Create New Project', exact: true }).click();
  await dialog(page).getByRole('radio', { name: new RegExp(template.replaceAll('+', '\\+')) }).check();
  await dialog(page).getByRole('button', { name: 'Configure project', exact: true }).click();
}
async function configureProject(page: Page, name = 'my-node-app', runtime = 'creation-node-22') {
  await dialog(page).getByLabel('Project name', { exact: true }).fill(name);
  await dialog(page).getByRole('button', { name: 'Choose parent folder…' }).click();
  await dialog(page).getByLabel('Node.js runtime', { exact: true }).selectOption(runtime);
}
async function review(page: Page) {
  await dialog(page).getByRole('button', { name: 'Check setup', exact: true }).click();
  await expect(dialog(page)).toContainText('Verified Node');
  await dialog(page).getByRole('button', { name: 'Review setup', exact: true }).click();
}
async function submit(page: Page) {
  await dialog(page).getByRole('checkbox', { name: /^I approve/ }).check();
  await dialog(page).getByRole('button', { name: 'Create and install', exact: true }).click();
}
test.beforeEach(async ({ page }) => {
  await page.goto('/'); await page.getByLabel('Example paths', { exact: true }).selectOption('macOS');
  await page.getByRole('button', { name: 'Node Projects', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Create New Project', exact: true })).toBeEnabled();
});
test('catalog selection, native validation, review consent and project-card handoff never auto-start', async ({ page }) => {
  await choose(page); await configureProject(page); await review(page);
  const modal = dialog(page);
  await expect(modal).toContainText('/controlled/Node café 日本 projects/my-node-app');
  await expect(modal).toContainText('Installation lifecycle scripts are disabled');
  await expect(modal).toContainText('package-lock.json');
  await expect(modal.getByRole('note')).toContainText('Dry run only.');
  await expect(modal.getByRole('button', { name: 'Create and install', exact: true })).toBeDisabled();
  expect((await stats(page)).starts).toBe(0);
  await submit(page);
  await expect(modal.getByRole('region', { name: 'Setup result' })).toContainText('my-node-app is registered and ready');
  await expect(modal.getByRole('region', { name: 'Installation output' })).toContainText('café 日本');
  await expect(modal.getByRole('region', { name: 'Installation output' })).toContainText('<script>plain text</script>');
  expect((await stats(page)).starts).toBe(1);
  await modal.getByRole('button', { name: 'View project', exact: true }).click();
  const card = page.getByRole('listitem', { name: 'my-node-app', exact: true });
  await expect(card).toBeFocused(); await expect(card.getByRole('status')).toHaveText('Stopped');
  await card.getByRole('button', { name: 'Start my-node-app', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Start Node project?', exact: true })).toBeVisible();
  await page.keyboard.press('Escape');
});
test('invalid names, occupied destinations, unsupported runtimes and native folder errors stay read-only', async ({ page }) => {
  await choose(page); await configureProject(page, 'Bad Name');
  await expect(dialog(page).getByRole('button', { name: 'Check setup', exact: true })).toBeDisabled();
  await dialog(page).getByLabel('Project name', { exact: true }).fill('already-exists');
  await dialog(page).getByRole('button', { name: 'Check setup', exact: true }).click(); await expect(dialog(page).getByRole('alert')).toContainText('destination already exists');
  await dialog(page).getByLabel('Project name', { exact: true }).fill('valid-name');
  await dialog(page).getByLabel('Node.js runtime', { exact: true }).selectOption('creation-node-20');
  await dialog(page).getByRole('button', { name: 'Check setup', exact: true }).click(); await expect(dialog(page).getByRole('alert')).toContainText('template engine range');
  await configure(page, { parent: '../unsafe' }); await configureProject(page, 'valid-name');
  await dialog(page).getByRole('button', { name: 'Check setup', exact: true }).click(); await expect(dialog(page).getByRole('alert')).toContainText('absolute parent folder');
  expect((await stats(page)).starts).toBe(0); expect((await jobs(page)).jobs).toHaveLength(0);
});
test('expired or stale review is regenerated and requires fresh confirmation', async ({ page }) => {
  await choose(page); await configureProject(page); await review(page); await configure(page, { expireValidate: true });
  await submit(page); await expect(dialog(page).getByRole('alert')).toContainText('new native review is ready');
  await expect(dialog(page).getByRole('checkbox', { name: /^I approve/ })).not.toBeChecked();
  await expect(dialog(page).getByRole('button', { name: 'Create and install', exact: true })).toBeDisabled();
  expect((await stats(page)).starts).toBe(0); expect((await stats(page)).plans).toBe(2);
  await submit(page); await expect(dialog(page)).toContainText('my-node-app is registered and ready');
});
test('cancellation preserves staging and hides unsupported recovery actions', async ({ page }) => {
  await configure(page, { duration: 15000 }); await choose(page); await configureProject(page); await review(page); await submit(page);
  await dialog(page).getByRole('button', { name: 'Cancel setup', exact: true }).click();
  await expect(dialog(page).getByRole('status').first()).toContainText('Setup cancelled');
  await expect(dialog(page)).toContainText('Any staging files are preserved');
  await expect(dialog(page).getByRole('button', { name: 'Retry Registration', exact: true })).toHaveCount(0);
  await expect(dialog(page).getByRole('button', { name: /Discard|Delete/ })).toHaveCount(0);
  expect((await stats(page)).cancels).toBe(1); await expect(page.getByRole('listitem', { name: 'my-node-app', exact: true })).toHaveCount(0);
});
for (const failAt of ['network', 'integrity', 'lock', 'registration'] as const) test(`handles ${failAt} failure and offers only supported recovery`, async ({ page }) => {
  await configure(page, { failAt }); await choose(page); await configureProject(page); await review(page); await submit(page);
  await expect(dialog(page)).toContainText('Setup failed');
  if (failAt === 'registration') {
    await expect(dialog(page)).toContainText('published project folder is preserved');
    await dialog(page).getByRole('button', { name: 'Retry Registration', exact: true }).click();
    await expect(dialog(page)).toContainText('my-node-app is registered and ready'); expect((await stats(page)).recoveries).toBe(1);
    await dialog(page).getByRole('button', { name: 'View project', exact: true }).click();
    await expect(page.getByRole('listitem', { name: 'my-node-app', exact: true })).toHaveCount(1);
  } else {
    await expect(dialog(page)).toContainText(failAt === 'integrity' ? 'EINTEGRITY' : failAt === 'lock' ? 'lockfile' : 'registry could not be reached');
    await expect(dialog(page).getByRole('button', { name: 'Retry Registration', exact: true })).toHaveCount(0);
    await expect(dialog(page).getByRole('button', { name: 'Review a new setup', exact: true })).toBeVisible();
  }
});
test('hide, navigation and WebView reload reconnect to the same job without submitting again', async ({ page }) => {
  await configure(page, { duration: 12000 }); await choose(page); await configureProject(page); await review(page); await submit(page);
  const original = (await jobs(page)).jobs[0].jobId;
  await dialog(page).getByRole('button', { name: 'Hide setup', exact: true }).click();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'Start all', exact: true }).click(); await page.getByRole('button', { name: 'Stop all', exact: true }).click();
  expect((await jobs(page)).jobs[0].active).toBe(true);
  await page.reload(); await page.getByRole('button', { name: 'Node Projects', exact: true }).click();
  await page.getByRole('button', { name: 'View setup for my-node-app', exact: true }).click();
  expect((await jobs(page)).jobs[0].jobId).toBe(original); expect((await stats(page)).starts).toBe(1);
  await expect(dialog(page)).toContainText('my-node-app is registered and ready', { timeout: 16000 });
});
test('interrupted jobs remain explicit and are never automatically restarted', async ({ page }) => {
  await configure(page, { duration: 30000 }); await choose(page); await configureProject(page); await review(page); await submit(page);
  await page.evaluate(async () => { const path = '/src/ui-test/node-creation.ts'; (await import(path)).interrupt(); });
  await dialog(page).getByRole('button', { name: 'Refresh setup status', exact: true }).click();
  await expect(dialog(page)).toContainText('Setup interrupted'); await expect(dialog(page)).toContainText('not restarted the setup or any server');
  expect((await stats(page)).starts).toBe(1); await expect(dialog(page).getByRole('button', { name: 'Retry Registration', exact: true })).toHaveCount(0);
});
test('keyboard focus, empty runtime list and minimum-width scrolling remain usable', async ({ page }) => {
  await configure(page, { noRuntimes: true }); await choose(page);
  await expect(dialog(page)).toContainText('No installed runtimes found');
  await expect(dialog(page).getByRole('button', { name: 'Check setup', exact: true })).toBeDisabled();
  await expect.poll(() => dialog(page).evaluate(root => root.scrollWidth <= root.clientWidth)).toBe(true);
  await page.keyboard.press('Escape'); await expect(page.getByRole('button', { name: 'Create New Project', exact: true })).toBeFocused();
});

test('all six catalog variants keep their own metadata and can be created with either compatible runtime', async ({ page }) => {
  test.setTimeout(60000);
  await configure(page, { duration: 500 });
  const catalog: { templates: { title: string; id: string }[] } = await page.evaluate(async () => { const path = '/src/ui-test/backend.ts'; const value = await (await import(path)).invoke('node_project_templates'); return value.catalog; });
  expect(catalog.templates).toHaveLength(6);
  for (const [index, template] of catalog.templates.entries()) {
    await choose(page, template.title); await configureProject(page, `created-${template.id}`, index % 2 ? 'creation-node-24' : 'creation-node-22'); await review(page);
    await expect(dialog(page)).toContainText(template.title); await expect(dialog(page)).toContainText(index % 2 ? 'npm 11.19.0' : 'npm 10.9.9');
    await submit(page); await expect(dialog(page)).toContainText(`created-${template.id} is registered and ready`);
    await dialog(page).getByRole('button', { name: 'View project', exact: true }).click();
    await expect(page.getByRole('listitem', { name: `created-${template.id}`, exact: true }).getByRole('status')).toHaveText('Stopped');
  }
  expect((await stats(page)).starts).toBe(6);
});
test('missing runtimes, invalid lockfile review and an empty catalog cannot create files', async ({ page }) => {
  await configure(page, { planError: 'runtimeUnavailable' }); await choose(page); await configureProject(page); await dialog(page).getByRole('button', { name: 'Check setup', exact: true }).click();
  await expect(dialog(page).getByRole('alert')).toContainText('runtimeUnavailable');
  await configure(page, { planError: 'invalidLockfile' }); await dialog(page).getByRole('button', { name: 'Check setup', exact: true }).click();
  await expect(dialog(page).getByRole('alert')).toContainText('No unpinned installation');
  await dialog(page).getByRole('button', { name: 'Close', exact: true }).click(); await configure(page, { emptyCatalog: true });
  await page.getByRole('button', { name: 'Create New Project', exact: true }).click(); await expect(dialog(page)).toContainText('No templates are available');
  await expect(dialog(page).getByRole('button', { name: 'Configure project', exact: true })).toBeDisabled(); expect((await stats(page)).starts).toBe(0);
});
test('large Unicode output is bounded and old job output cannot appear in a new setup', async ({ page }) => {
  await configure(page, { flood: true, duration: 500 }); await choose(page); await configureProject(page, 'flood-one'); await review(page); await submit(page);
  await expect(dialog(page)).toContainText('flood-one is registered and ready');
  await expect(dialog(page).getByRole('region', { name: 'Installation output' })).toContainText('burst-1299', { timeout: 12000 });
  expect(await dialog(page).locator('.creation-output>div').count()).toBeLessThanOrEqual(1024);
  expect(await dialog(page).locator('.creation-output').evaluate(root => new TextEncoder().encode(root.textContent ?? '').length)).toBeLessThan(300000);
  await dialog(page).getByRole('button', { name: 'View project', exact: true }).click();
  await configure(page, { flood: false, logsDelay: 150 }); await choose(page); await configureProject(page, 'new-job'); await review(page); await submit(page);
  await expect(dialog(page)).toContainText('new-job is registered and ready');
  await expect(dialog(page).getByRole('region', { name: 'Installation output' })).toContainText('new-job');
  await expect(dialog(page).getByRole('region', { name: 'Installation output' })).not.toContainText('flood-one');
});
test('editing checked inputs invalidates the plan and a duplicate click sends one native start', async ({ page }) => {
  await choose(page); await configureProject(page); await review(page);
  await dialog(page).getByRole('button', { name: 'Back', exact: true }).click();
  await dialog(page).getByLabel('Project name', { exact: true }).fill('changed-name');
  await expect(dialog(page).getByRole('button', { name: 'Review setup', exact: true })).toBeDisabled();
  await review(page); await dialog(page).getByRole('checkbox', { name: /^I approve/ }).check();
  await dialog(page).getByRole('button', { name: 'Create and install', exact: true }).evaluate(button => { (button as HTMLButtonElement).click(); (button as HTMLButtonElement).click(); });
  await expect(dialog(page)).toContainText('changed-name is registered and ready'); expect((await stats(page)).starts).toBe(1);
});

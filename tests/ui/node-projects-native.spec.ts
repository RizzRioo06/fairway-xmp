import { expect, test, type Page } from '@playwright/test';
import { join } from 'node:path';
import { readFile, readdir } from 'node:fs/promises';
import { nativeBridge } from './native-bridge';
test.use({ baseURL: 'http://127.0.0.1:1422' });
async function importNative(page: Page, name: string, major: string) {
  await page.getByRole('button', { name: 'Import Node project', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'Choose folder…' }).click();
  await expect(dialog.getByLabel('Project name', { exact: true })).toBeEnabled({ timeout: 30000 });
  await dialog.getByLabel('Project name', { exact: true }).fill(name);
  const runtime = await dialog.getByLabel('Node runtime', { exact: true }).locator('option').evaluateAll((options, major) => options.find(option => option.textContent?.includes(`v${major}.`))?.getAttribute('value'), major);
  expect(runtime).toBeTruthy();
  await dialog.getByLabel('Node runtime', { exact: true }).selectOption(runtime!);
  await dialog.getByRole('button', { name: 'Save project', exact: true }).click(); await expect(dialog).toBeHidden();
}
async function start(page: Page, name: string) {
  await page.getByRole('button', { name: `Start ${name}`, exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Start project', exact: true }).click();
  await expect(page.getByRole('region', { name: `Output for ${name}` })).toContainText('NATIVE_NODE', { timeout: 20000 });
}
test('real controlled Node/npm projects survive navigation and UI reload, restart, stop and keep imported files', async ({ page }, info) => {
  test.skip(process.platform !== 'darwin' || !process.env.FAIRWAY_TEST_RUNTIME || !process.env.FAIRWAY_TEST_NODE_RUNTIMES, 'Supply packaged runtime and two controlled Node versions on macOS.');
  test.setTimeout(120000);
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/'); await page.getByRole('button', { name: 'Node Projects', exact: true }).click();
    await expect(page.getByText('No Node projects yet', { exact: true })).toBeVisible();
    await importNative(page, 'Native 22', '22'); await importNative(page, 'Native 24', '24');
    await start(page, 'Native 22'); await start(page, 'Native 24');
    await expect(page.getByRole('region', { name: 'Output for Native 24' })).toContainText('v24.');
    await page.getByRole('button', { name: 'Environment', exact: true }).click();
    await page.reload(); await page.getByRole('button', { name: 'Node Projects', exact: true }).click();
    for (const name of ['Native 22', 'Native 24']) await expect(page.getByRole('listitem', { name, exact: true }).getByRole('status')).toHaveText('Running');
    await page.getByRole('button', { name: 'Restart Native 22', exact: true }).click();
    await page.getByRole('dialog').getByRole('button', { name: 'Restart project', exact: true }).click();
    await expect(page.getByRole('region', { name: 'Output for Native 22' })).toContainText('v22.');
    for (const name of ['Native 22', 'Native 24']) {
      await page.getByRole('button', { name: `Stop ${name}`, exact: true }).click();
      await expect(page.getByRole('listitem', { name, exact: true }).getByRole('status')).toHaveText('Stopped');
    }
    const root = join(bridge.root, 'node fixture 0 日本');
    expect((await readFile(join(root, 'package.json'), 'utf8'))).toContain('server.cjs');
    expect((await readdir(root)).sort()).toEqual(['failure.cjs', 'package.json', 'server.cjs']);
    await page.getByRole('button', { name: 'Remove Native 22', exact: true }).click();
    await page.getByRole('dialog').getByRole('button', { name: 'Remove registration', exact: true }).click();
    await expect(page.getByRole('region', { name: 'Unlisted project runs', exact: true })).toHaveCount(0);
    expect((await readFile(join(root, 'package.json'), 'utf8'))).toContain('server.cjs');
  } finally { await bridge.close(); }
});

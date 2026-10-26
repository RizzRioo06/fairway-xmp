import { expect, test } from '@playwright/test';
import type { Locator, Page } from '@playwright/test';
import type { NodeStatus } from '../../src/types';

async function status(page: Page): Promise<NodeStatus> {
  return page.evaluate(async () => {
    const path = '/src/ui-test/backend.ts';
    return (await import(path)).invoke('node_status');
  });
}

async function options(page: Page, value: { installDelay?: number; failActivation?: boolean }) {
  await page.evaluate(value => {
    (window as Window & { __fairwayNodeOptions?: typeof value }).__fairwayNodeOptions = value;
  }, value);
}

async function install(card: Locator, version: string, activate: boolean) {
  if (!(await card.locator('.node-manager').evaluate(element => (element as HTMLDetailsElement).open))) {
    await card.locator('.node-manager > summary').click();
  }
  await card.getByLabel('Install a version', { exact: true }).selectOption(version);
  await card.getByRole('button', { name: activate ? 'Install & use' : 'Install', exact: true }).click();
  await expect(card.getByRole('button', { name: 'Refresh Node detection' })).toBeEnabled();
}

for (const platform of ['Windows', 'macOS']) {
  test.describe(platform, () => {
    test.beforeEach(async ({ page }) => {
      await page.goto('/');
      await page.getByLabel('Example paths', { exact: true }).selectOption(platform);
    });

    test('Install preserves external Node and Use explicitly selects the installed version', async ({ page }) => {
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      const card = page.getByRole('region', { name: 'Node.js settings' });
      await expect(card.locator('.node-current')).toContainText('v24.14.1');
      const original = (await status(page)).detected;
      await install(card, 'v22.1.0', false);
      expect((await status(page)).enabled).toBe(false);
      expect((await status(page)).detected).toEqual(original);
      await expect(card.locator('.node-current')).toContainText('v24.14.1');
      await card.getByRole('button', { name: 'Use Node v22.1.0 in new terminals' }).click();
      await expect(card.locator('.node-current')).toContainText('v22.1.0');
      expect((await status(page)).enabled).toBe(true);
    });

    test('Install & use activates Node and uninstall requires confirmation', async ({ page }) => {
      await page.getByLabel('Scenario', { exact: true }).selectOption('Node not detected');
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      const card = page.getByRole('region', { name: 'Node.js settings' });
      await install(card, 'v24.14.1', true);
      await expect(card.locator('.node-current')).toContainText('v24.14.1');
      const uninstall = card.getByRole('button', { name: 'Uninstall Node v24.14.1', exact: true });
      await uninstall.click();
      const dialog = page.getByRole('dialog', { name: 'Uninstall Node v24.14.1?' });
      await expect(dialog).toContainText('global npm packages');
      await expect(dialog).toContainText('active fnm version');
      await page.keyboard.press('Escape');
      await expect(dialog).toHaveCount(0);
      expect((await status(page)).installed).toEqual(['v24.14.1']);
      await uninstall.click();
      await dialog.getByRole('button', { name: 'Uninstall Node', exact: true }).click();
      await expect(dialog).toHaveCount(0);
      await expect(card.getByRole('heading', { name: 'Node.js', exact: true })).toBeFocused();
      const state = await status(page);
      expect(state.installed).toEqual([]);
      expect(state.enabled).toBe(false);
    });

    test('existing fnm and unowned stores stay read-only and handover preserves versions', async ({ page }) => {
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      const card = page.getByRole('region', { name: 'Node.js settings' });
      for (const scenario of ['Existing fnm', 'Existing fnm store', 'fnm handover']) {
        await page.getByLabel('Scenario', { exact: true }).selectOption(scenario);
        await expect(card.getByText('Read-only', { exact: true })).toBeVisible();
        await expect(card.getByRole('button', { name: /^Install|^Use Node|^Uninstall/ })).toHaveCount(0);
        if (scenario === 'fnm handover') {
          const before = await status(page);
          await card.getByRole('button', { name: 'Turn off Fairway activation' }).click();
          await expect(card.getByRole('button', { name: 'Turn off Fairway activation' })).toHaveCount(0);
          const after = await status(page);
          expect(after.enabled).toBe(false);
          expect(after.installed).toEqual(before.installed);
          expect(after.defaultVersion).toBe(before.defaultVersion);
        }
      }
    });

    test('failed Use leaves the previous selection active and can be retried', async ({ page }) => {
      await page.getByLabel('Scenario', { exact: true }).selectOption('Node not detected');
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      const card = page.getByRole('region', { name: 'Node.js settings' });
      await install(card, 'v24.14.1', true);
      await install(card, 'v22.1.0', false);
      await options(page, { failActivation: true });
      const use = card.getByRole('button', { name: 'Use Node v22.1.0 in new terminals' });
      await use.click();
      await expect(page.getByRole('alert')).toContainText('Previous Node selection restored');
      await expect(card.locator('.node-current')).toContainText('v24.14.1');
      expect((await status(page)).defaultVersion).toBe('v24.14.1');
      await options(page, {});
      await use.click();
      await expect(card.locator('.node-current')).toContainText('v22.1.0');
    });

    test('download progress survives navigation and cancellation permits retry', async ({ page }) => {
      await page.getByLabel('Scenario', { exact: true }).selectOption('Node not detected');
      await options(page, { installDelay: 30_000 });
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      const card = page.getByRole('region', { name: 'Node.js settings' });
      await card.getByLabel('Install a version', { exact: true }).selectOption('v24.14.1');
      await card.getByRole('button', { name: 'Install & use', exact: true }).click();
      await expect(card.getByRole('progressbar', { name: 'Node installation progress' })).toBeVisible();
      const id = (await status(page)).installation!.id;
      // A delayed cancel from another request must not stop this download.
      await page.evaluate(async () => {
        const path = '/src/ui-test/backend.ts';
        await (await import(path)).invoke('cancel_node_installation', { id: 'old-request' });
      });
      expect((await status(page)).installation?.id).toBe(id);
      await page.getByRole('button', { name: 'Environment', exact: true }).click();
      await page.getByRole('button', { name: 'Settings', exact: true }).click();
      await expect(card.getByRole('progressbar', { name: 'Node installation progress' })).toBeVisible();
      await card.getByRole('button', { name: 'Cancel download', exact: true }).click();
      await expect(card.getByRole('progressbar', { name: 'Node installation progress' })).toHaveCount(0);
      await expect(card.getByRole('button', { name: 'Refresh Node detection' })).toBeEnabled();
      const cancelled = await status(page);
      expect(cancelled.installed).toEqual([]);
      expect(cancelled.enabled).toBe(false);
      await options(page, {});
      await install(card, 'v24.14.1', true);
      await expect(card.locator('.node-current')).toContainText('v24.14.1');
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    });
  });
}

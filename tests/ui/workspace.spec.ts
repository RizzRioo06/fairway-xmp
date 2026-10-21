import { expect, test, type Page } from '@playwright/test';

async function open(page: Page, navigation: string) {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  await page.getByRole('button', { name: navigation, exact: true }).click();
}

async function maximize(page: Page, title: string) {
  await page.getByRole('button', { name: `Maximize ${title}`, exact: true }).click();
  const workspace = page.getByRole('dialog', { name: `${title} maximized`, exact: true });
  await expect(workspace).toBeVisible();
  const bounds = (await workspace.boundingBox())!;
  expect(bounds.x).toBe(0); expect(bounds.y).toBe(0);
  expect(bounds.width).toBe(page.viewportSize()!.width);
  expect(bounds.height).toBe(page.viewportSize()!.height);
  expect(await page.locator('.sidebar').evaluate(element => element.hasAttribute('inert'))).toBe(true);
  return workspace;
}

test('logs preserve filters, pause state, page scroll and focus across maximize and nested menus', async ({ page }) => {
  await open(page, 'Logs');
  await page.getByRole('searchbox', { name: 'Search logs' }).fill('php');
  await page.getByRole('button', { name: 'Log options' }).click();
  await page.getByRole('menuitem', { name: 'Pause', exact: true }).click();
  const position = await page.evaluate(() => window.scrollY);
  const trigger = page.getByRole('button', { name: 'Maximize Logs', exact: true });
  await trigger.focus();
  const workspace = await maximize(page, 'Logs');
  await expect(workspace.getByRole('button', { name: 'Open log folder' })).toBeVisible();
  await page.keyboard.press('Control+Backquote');
  await expect(workspace).toBeVisible();
  await workspace.getByRole('button', { name: 'Log options' }).click();
  await page.keyboard.press('Escape');
  await expect(workspace.getByRole('menu')).toHaveCount(0);
  await expect(workspace).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(workspace).toHaveCount(0);
  await expect(trigger).toBeFocused();
  await expect(page.getByRole('searchbox', { name: 'Search logs' })).toHaveValue('php');
  await expect(page.getByRole('status').filter({ hasText: /^Paused$/ })).toBeVisible();
  expect(await page.evaluate(() => window.scrollY)).toBe(position);
  expect(await page.locator('.sidebar').evaluate(element => element.hasAttribute('inert'))).toBe(false);
});

test('terminal keeps its live instance, shell Escape and search behavior while maximized', async ({ page }) => {
  await open(page, 'Terminal');
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  const input = page.getByRole('textbox', { name: 'Terminal input' });
  await input.focus(); await input.pressSequentially('echo retained-workspace'); await input.press('Enter');
  const output = page.locator('.terminal-tab-panel:not([hidden]) .xterm-rows');
  await expect(output).toContainText('retained-workspace');
  const instance = await input.elementHandle();
  const workspace = await maximize(page, 'Terminal');
  await input.focus(); await input.press('Escape');
  await expect(workspace).toBeVisible();
  await input.press(process.platform === 'darwin' ? 'Meta+f' : 'Control+Shift+f');
  await expect(page.getByRole('textbox', { name: 'Find in terminal' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(workspace).toBeVisible();
  await input.press('Control+Shift+m');
  await expect(workspace).toHaveCount(0);
  expect(await input.evaluate((element, original) => element === original, instance)).toBe(true);
  await expect(output).toContainText('retained-workspace');
});

test('mail dialogs, safe HTML preview and restore preserve the selected message', async ({ page, browserName }) => {
  await open(page, 'View Mail');
  await page.getByRole('list', { name: 'Emails' }).getByRole('button', { name: /Welcome to My Shop/ }).click();
  const workspace = await maximize(page, 'Mail');
  await workspace.getByRole('button', { name: 'SMTP settings' }).click();
  const connection = page.getByRole('dialog', { name: 'Mail connection', exact: true });
  await expect(connection).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(connection).toHaveCount(0);
  await expect(workspace).toBeVisible();
  const iframe = workspace.locator('iframe[title="Email content"]');
  await expect(iframe).toHaveAttribute('sandbox', 'allow-same-origin');
  const body = page.frameLocator('iframe[title="Email content"]').locator('body');
  await expect(body).not.toContainText('scripts must not run');
  await body.click(); await body.press('Escape');
  if (browserName === 'webkit') {
    // WebKit blocks even parent-owned listeners in a script-disabled frame.
    // Preserve the sandbox and use the always-visible restore control.
    await expect(workspace).toBeVisible();
    await workspace.getByRole('button', { name: 'Restore Mail', exact: true }).click();
  }
  await expect(workspace).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Welcome to My Shop', exact: true })).toBeVisible();
});

test('database source controls, editor state and nested row dialogs survive maximize', async ({ page }) => {
  await open(page, 'Browse Data');
  await expect(page.getByRole('button', { name: 'Insert row', exact: true })).toBeEnabled();
  const workspace = await maximize(page, 'Data browser');
  await expect(workspace.getByRole('tab', { name: 'SQLite', exact: true })).toBeVisible();
  await workspace.getByRole('button', { name: 'Insert row', exact: true }).click();
  const edit = page.getByRole('dialog', { name: 'Insert row', exact: true });
  await expect(edit).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(edit).toHaveCount(0);
  await expect(workspace).toBeVisible();
  await workspace.getByRole('tab', { name: 'SQL query', exact: true }).click();
  await page.locator('#db-sql').fill('SELECT 42 AS preserved;');
  await workspace.getByRole('button', { name: 'Restore Data browser' }).click();
  await expect(page.locator('#db-sql')).toHaveValue('SELECT 42 AS preserved;');
});

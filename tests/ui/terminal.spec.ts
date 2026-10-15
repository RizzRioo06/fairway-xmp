import { expect, test, type Page } from '@playwright/test';

const output = (page: Page) => page.locator('.terminal-host .xterm-rows');
async function enter(page: Page, command: string) {
  const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
  await input.focus(); await input.pressSequentially(command); await input.press('Enter');
}
async function open(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Open terminal', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
}

test('terminal runs input, preserves the session across pages, fits the window, and exits', async ({ page }, info) => {
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await open(page);
  await expect(page.getByText('UI test — simulated terminal. Commands do not run on your computer.')).toBeVisible();
  await enter(page, 'php --version');
  await expect(output(page)).toContainText('PHP 8.5.');
  await enter(page, 'echo session-retained');
  await expect(output(page)).toContainText('session-retained');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(output(page)).toContainText('session-retained');
  await page.setViewportSize({ width: 820, height: 650 });
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const path = info.outputPath('embedded-terminal.png');
  await page.screenshot({ path, fullPage: true }); await info.attach('embedded-terminal', { path, contentType: 'image/png' });
  await enter(page, 'exit');
  await expect(page.getByRole('status').filter({ hasText: /^Session ended$/ })).toBeVisible();
  await expect(output(page)).toContainText('exit 0');
  await page.getByRole('button', { name: 'New session', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  expect(errors).toEqual([]);
});

test('interrupt, restart confirmation, end session, and missing runtime recovery', async ({ page }) => {
  await open(page);
  await page.getByRole('textbox', { name: 'Terminal input' }).press('Control+c');
  await expect(output(page)).toContainText('^C');
  await page.getByRole('button', { name: 'Restart', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await page.getByRole('button', { name: 'Restart', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Restart terminal' }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await page.getByRole('button', { name: 'End session', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'End session', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Session ended$/ })).toBeVisible();
  await page.getByLabel('Scenario', { exact: true }).selectOption('Missing runtime');
  await page.getByRole('button', { name: 'New session' }).click();
  await expect(page.getByRole('alert')).toContainText('Runtime files are missing');
  await page.getByLabel('Scenario', { exact: true }).selectOption('CLI disabled');
  await page.getByRole('button', { name: 'New session' }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await enter(page, 'composer --version');
  await expect(output(page)).toContainText('Composer version');
});

test('production preview cannot create a shell', async ({ page }) => {
  await page.goto('http://127.0.0.1:1422/');
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(page.getByText('Open the Fairway XMP desktop app to use the terminal.')).toBeVisible();
  await expect(page.locator('.terminal-host')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'New session' })).toBeDisabled();
});

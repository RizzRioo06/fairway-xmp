import { test, expect } from '@playwright/test';

test('slow mail searches and message opens cannot replace a newer selection', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Slow mail');
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  const emails = page.getByRole('list', { name: 'Emails', exact: true });
  await expect(emails.getByRole('button')).toHaveCount(3);
  await page.getByLabel('Search emails').fill('welcome');
  await page.waitForTimeout(400); // The debounced slow request is now pending.
  await page.getByLabel('Search emails').fill('reset');
  await expect(emails.getByRole('button')).toHaveCount(1);
  await expect(emails).toContainText('Reset your password');
  await page.waitForTimeout(1500);
  await expect(emails).toContainText('Reset your password');
  await expect(emails).not.toContainText('Welcome to My Shop');
  await page.getByLabel('Search emails').fill('');
  await expect(emails.getByRole('button')).toHaveCount(3);
  await emails.getByRole('button', { name: /Welcome to My Shop/ }).click();
  await emails.getByRole('button', { name: /Reset your password/ }).click();
  await expect(page.getByRole('heading', { name: 'Reset your password', exact: true })).toBeVisible();
  await page.waitForTimeout(1600);
  await expect(page.getByRole('heading', { name: 'Reset your password', exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Welcome to My Shop', exact: true })).toHaveCount(0);
});


test('missing optional mail leaves core readiness intact and explains disabled controls', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Mail unavailable');
  await expect(page.getByText('All services ready', { exact: true })).toBeVisible();
  const card = page.locator('.service-card').filter({ has: page.getByRole('heading', { name: 'Mail', exact: true }) });
  await expect(card).toContainText('Mail catcher files are missing');
  await expect(card.getByRole('button', { name: 'Start service', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Mail is unavailable.', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Start mail', exact: true })).toBeDisabled();
});

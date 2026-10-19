import { test, expect, type Page } from '@playwright/test';

async function expectStill(page: Page, position: number) {
  const samples = await page.evaluate(async () => {
    const positions: number[] = [];
    for (let i = 0; i < 45; i++) {
      await new Promise(requestAnimationFrame);
      positions.push(window.scrollY);
    }
    return positions;
  });
  expect(samples.every(value => Math.abs(value - position) <= 1)).toBe(true);
}

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

test('attachment results stay on screen without moving the page or focus', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await page.getByRole('list', { name: 'Emails' }).getByRole('button', { name: /Invoice #1042/ }).click();
  const save = page.getByRole('list', { name: 'Attachments' }).getByRole('button', { name: /invoice-1042\.pdf/ });
  const path = `/Users/Student May/Downloads/${'invoice-1042-'.repeat(160)}pdf`;
  await page.evaluate(saved => { (window as unknown as { __fairwayMailSave: { path: string } }).__fairwayMailSave = { path: saved }; }, path);
  await page.evaluate(() => { document.querySelector('main')!.style.minHeight = '2400px'; window.scrollTo(0, 700); });
  await save.evaluate(element => (element as HTMLElement).focus({ preventScroll: true }));
  const position = await page.evaluate(() => window.scrollY);
  expect(position).toBeGreaterThan(100);
  await page.keyboard.press('Enter');
  const status = page.getByRole('status').filter({ hasText: 'Saved to' });
  await expect(status).toContainText(path);
  await expect(page.locator('.mail-notice')).toBeInViewport({ ratio: 1 });
  await expect(page.getByRole('button', { name: 'Dismiss mail message', exact: true })).toBeInViewport({ ratio: 1 });
  expect(await page.locator('.mail-notice').evaluate(element => element.getBoundingClientRect().height)).toBeLessThan(270);
  const dimensions = await page.locator('.mail-notice-message').evaluate(element => ({ height: element.clientHeight, content: element.scrollHeight }));
  expect(dimensions.content).toBeGreaterThan(dimensions.height);
  await expectStill(page, position);
  await expect(save).toBeFocused();
  await page.evaluate(() => { (window as unknown as { __fairwayMailSave: { error: string } }).__fairwayMailSave = { error: `Could not save the attachment. ${'Choose another folder. '.repeat(80)}` }; });
  await page.keyboard.press('Enter');
  const alert = page.getByRole('alert').filter({ hasText: 'Could not save the attachment.' });
  await expect(alert).toContainText('Choose another folder.');
  await expect(page.locator('.mail-notice')).toBeInViewport({ ratio: 1 });
  await expectStill(page, position);
  await expect(save).toBeFocused();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Could not save the attachment.' })).toHaveCount(0);
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Could not save the attachment.' })).toHaveCount(0);
});

test('a late attachment failure does not return after leaving mail', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await page.getByRole('list', { name: 'Emails' }).getByRole('button', { name: /Invoice #1042/ }).click();
  await page.evaluate(() => { (window as unknown as { __fairwayMailSave: { delay: number; error: string } }).__fairwayMailSave = { delay: 700, error: 'Could not save the attachment.' }; });
  await page.getByRole('list', { name: 'Attachments' }).getByRole('button', { name: /invoice-1042\.pdf/ }).click();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.waitForTimeout(900);
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Could not save the attachment.' })).toHaveCount(0);
});

test('inbox load failures stay in the message list', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  const detail = `Could not load the inbox. ${'Mailpit did not respond. '.repeat(40)}`;
  await page.evaluate(text => { (window as unknown as { __fairwayMailListError: string }).__fairwayMailListError = text; }, detail);
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  const alert = page.locator('.mail-error');
  await expect(alert).toContainText('Could not load the inbox.');
  await expect(page.locator('.mail-notice')).toHaveText('');
  const dimensions = await alert.evaluate(element => ({ height: element.clientHeight, content: element.scrollHeight }));
  expect(dimensions.content).toBeGreaterThan(dimensions.height);
  expect(dimensions.height).toBeLessThan(270);
});

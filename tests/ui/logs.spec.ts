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

test('global logs focus on problems with source/search filters and plain text', async ({ page }, info) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  const output = page.getByRole('region', { name: 'Log entries', exact: true });
  await expect(page.getByRole('checkbox', { name: 'Problems only' })).toBeChecked();
  await expect(output).toContainText('PHP Fatal error');
  await expect(output).toContainText('→ 500');
  await expect(output).toContainText('custom diagnostic <script>safe text</script>');
  await expect(output.locator('script')).toHaveCount(0);
  await expect(output).not.toContainText('→ 200');
  await page.getByLabel('Log source').selectOption('php');
  await expect(output).toContainText('Stack trace:');
  await expect(output).not.toContainText('→ 500');
  await page.getByLabel('Log source').selectOption('all');
  await page.getByLabel('Search logs', { exact: true }).fill('shop.test');
  await expect(output).toContainText('No matching entries');
  await page.getByRole('checkbox', { name: 'Problems only' }).uncheck();
  await expect(output).toContainText('https://shop.test:8843');
  await expect(output).not.toContainText('PHP Fatal error');
  await page.getByLabel('Search logs', { exact: true }).fill('');
  await page.getByLabel('Log source').selectOption('mysql');
  await expect(output).toContainText('Ready for local connections');
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await expect(page.getByLabel('Log source')).toHaveValue('mysql');
  await expect(page.getByRole('checkbox', { name: 'Problems only' })).not.toBeChecked();
  await page.getByLabel('Log source').selectOption('all');
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const screenshot = info.outputPath('global-logs.png');
  await page.screenshot({ path: screenshot, fullPage: true });
  await info.attach('global-logs', { path: screenshot, contentType: 'image/png' });
});

test('logs polling pauses, stays off other screens, and copies only the shown entries', async ({ page }) => {
  let reads = 0;
  let clipboard = '';
  await page.exposeFunction('recordLogRead', () => { reads++; });
  await page.exposeFunction('recordLogCopy', (value: string) => { clipboard = value; });
  await page.addInitScript(() => {
    const bridge = window as unknown as { recordLogRead: () => Promise<void>; recordLogCopy: (value: string) => Promise<void> };
    // Track the actual simulated command without replacing its response.
    window.addEventListener('fairway-ui-command', event => {
      if ((event as CustomEvent).detail === 'read_logs') void bridge.recordLogRead();
    });
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: bridge.recordLogCopy }, configurable: true });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await expect.poll(() => reads).toBeGreaterThan(0);
  await page.getByRole('button', { name: 'Pause', exact: true }).click();
  const pausedReads = reads;
  await page.waitForTimeout(1800);
  expect(reads).toBe(pausedReads);
  await page.getByLabel('Log source').selectOption('php');
  await page.getByRole('button', { name: 'Copy shown', exact: true }).click();
  await expect.poll(() => clipboard).toContain('PHP Fatal error');
  expect(clipboard).not.toContain('custom diagnostic');
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect.poll(() => reads).toBeGreaterThan(pausedReads);
  await page.getByRole('button', { name: 'Resume', exact: true }).click();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.waitForTimeout(200);
  const hiddenReads = reads;
  await page.waitForTimeout(1800);
  expect(reads).toBe(hiddenReads);
});

test('service shortcuts share the searchable viewer and Logs follows Terminal in Core', async ({ page }) => {
  await page.goto('/');
  const core = page.getByRole('group', { name: 'Core', exact: true });
  await expect(core.getByRole('button')).toHaveText(['Environment', 'Terminal', 'Logs']);
  await expect(page.getByRole('group', { name: 'Tools', exact: true }).getByRole('button')).toHaveText(['Manage Sites', 'View Mail', 'Browse Data']);
  for (const [service, source] of [['PHP', 'php'], ['Nginx', 'nginx'], ['MySQL', 'mysql'], ['phpMyAdmin', 'nginx'], ['Mail', 'mail']]) {
    await page.getByRole('button', { name: 'Environment', exact: true }).click();
    await page.getByRole('button', { name: `View ${service} logs`, exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Logs', exact: true })).toBeVisible();
    await expect(page.getByLabel('Log source')).toHaveValue(source);
    await expect(page.getByRole('dialog')).toHaveCount(0);
    await page.getByLabel('Search logs', { exact: true }).fill('old search');
  }
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await page.getByRole('button', { name: 'View logs', exact: true }).click();
  await expect(page.getByLabel('Log source')).toHaveValue('mail');
  await expect(page.getByLabel('Search logs', { exact: true })).toHaveValue('');
});

test('a burst of logs has a bounded display and remains searchable and responsive', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Logs under load');
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  const output = page.getByRole('region', { name: 'Log entries', exact: true });
  await expect(output.locator('.log-entry')).toHaveCount(240);
  await expect(page.getByText('240 of 900 entries · Live', { exact: true })).toBeVisible();
  await expect(output).toContainText('burst-log-0 '); // A dated failure survives the undated flood.
  await page.getByLabel('Search logs', { exact: true }).fill('burst-log-0 ');
  await expect(output.locator('.log-entry')).toHaveCount(1);
  await expect(output).toContainText('日本 မြန်မာ');
  await page.getByRole('button', { name: 'Pause', exact: true }).click();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Ready when you are.' })).toBeVisible();
});

test('site scope shows its requests alongside explicitly shared PHP and shortcuts restore global scope', async ({ page }, info) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  const output = page.getByRole('region', { name: 'Log entries', exact: true });
  await expect(output).toContainText('PHP Fatal error');
  await page.getByLabel('Log site').selectOption('shop.test');
  await page.getByRole('checkbox', { name: 'Problems only' }).uncheck();
  await expect(output).toContainText('https://shop.test:8843');
  // This PHP error mentions another project's path; it must remain shared.
  await expect(output).toContainText('lesson/index.php');
  await expect(output).toContainText('Shared PHP');
  await expect(page.locator('.logs-scope')).toContainText('may belong to another site');
  await expect(output).not.toContainText('http://lesson.test:8800');
  await expect(output).not.toContainText('custom diagnostic');
  await expect(output).not.toContainText('Ready for local connections');
  await page.getByLabel('Log source').selectOption('requests');
  await expect(output).not.toContainText('PHP Fatal error');
  await page.getByLabel('Log site').selectOption('lesson.test');
  await expect(output).toContainText('http://lesson.test:8800');
  await expect(output).not.toContainText('https://shop.test');
  await page.getByLabel('Log source').selectOption('all');
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await expect(page.getByLabel('Log site')).toHaveValue('lesson.test');
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const screenshot = info.outputPath('site-requests-shared-php.png');
  await page.screenshot({ path: screenshot, fullPage: true });
  await info.attach('site-requests-shared-php', { path: screenshot, contentType: 'image/png' });
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'View MySQL logs', exact: true }).click();
  await expect(page.getByLabel('Log site')).toHaveValue('');
  await expect(page.getByLabel('Log source')).toHaveValue('mysql');
  await expect(output).toContainText('Ready for local connections');
  // Registered sites are offered before they have any request history.
  await page.getByLabel('Log site').selectOption('blog.test');
  await expect(page.getByLabel('Log source')).toHaveValue('all');
  await expect(output).toContainText('Shared PHP');
  await expect(output).not.toContainText('https://shop.test');
  await page.getByLabel('Log site').selectOption('');
  await expect(output).toContainText('custom diagnostic');
});


test('pending log reads are shared across shortcuts and quick navigation', async ({ page }) => {
  await page.addInitScript(() => {
    let active = 0;
    Object.assign(window, { maxLogReads: 0 });
    window.addEventListener('fairway-ui-command', event => {
      if ((event as CustomEvent).detail !== 'read_logs') return;
      const state = window as unknown as { maxLogReads: number };
      state.maxLogReads = Math.max(state.maxLogReads, ++active);
      window.setTimeout(() => active--, 600);
    });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'View PHP logs', exact: true }).click();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Log entries', exact: true })).toContainText('PHP Fatal error');
  expect(await page.evaluate(() => (window as unknown as { maxLogReads: number }).maxLogReads)).toBe(1);
});

test('clipboard failures survive polling and retry separately from log reads', async ({ page }) => {
  await page.addInitScript(() => {
    let attempts = 0;
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: async () => {
      if (++attempts === 1) throw new Error('Clipboard denied');
    } }, configurable: true });
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await page.getByRole('button', { name: 'Copy shown', exact: true }).click();
  const alert = page.getByRole('alert').filter({ hasText: 'Could not copy logs' });
  await expect(alert).toBeVisible();
  await page.waitForTimeout(2400);
  await expect(alert).toBeVisible();
  await page.getByRole('button', { name: 'Retry copy', exact: true }).click();
  await expect(alert).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Copied', exact: true })).toBeVisible();
});

test('empty history and unreadable logs have distinct explanations', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Logs empty');
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'No log entries yet', exact: true })).toBeVisible();
  await page.getByLabel('Scenario', { exact: true }).selectOption('Logs unreadable');
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Logs unavailable', exact: true })).toBeVisible();
  await expect(page.getByRole('alert')).toContainText('php.log: permission denied');
  await expect(page.locator('.logs-notice')).toHaveText('');
});

test('copy failures stay on screen and do not return after navigation', async ({ page }) => {
  const detail = `Clipboard denied. ${'Grant clipboard permission. '.repeat(40)}`;
  await page.addInitScript(message => {
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: async () => { throw new Error(message); } }, configurable: true });
  }, detail);
  await page.goto('/');
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  const output = page.getByRole('region', { name: 'Log entries', exact: true });
  await expect(output).toContainText('PHP Fatal error');
  await output.evaluate(element => { element.scrollTop = 80; });
  const copy = page.getByRole('button', { name: 'Copy shown', exact: true });
  await page.evaluate(() => { document.querySelector('main')!.style.minHeight = '2400px'; window.scrollTo(0, 700); });
  await copy.evaluate(element => (element as HTMLElement).focus({ preventScroll: true }));
  const position = await page.evaluate(() => window.scrollY);
  expect(position).toBeGreaterThan(100);
  const listPosition = await output.evaluate(element => element.scrollTop);
  await page.keyboard.press('Enter');
  const alert = page.getByRole('alert').filter({ hasText: 'Could not copy logs' });
  await expect(alert).toContainText(detail);
  await expect(page.locator('.logs-notice')).toBeInViewport({ ratio: 1 });
  await expect(page.getByRole('button', { name: 'Dismiss logs message', exact: true })).toBeInViewport({ ratio: 1 });
  expect(await page.locator('.logs-notice').evaluate(element => element.getBoundingClientRect().height)).toBeLessThan(270);
  const dimensions = await page.locator('.logs-notice-message').evaluate(element => ({ height: element.clientHeight, content: element.scrollHeight }));
  expect(dimensions.content).toBeGreaterThan(dimensions.height);
  await expectStill(page, position);
  await expect(copy).toBeFocused();
  expect(await output.evaluate(element => element.scrollTop)).toBe(listPosition);
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(alert).toHaveCount(0);
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Could not copy logs' })).toHaveCount(0);
  await expect(page.locator('.logs-error')).toHaveCount(0);
});

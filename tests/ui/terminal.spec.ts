import { expect, test, type Page } from '@playwright/test';

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

const output = (page: Page) => page.locator('.terminal-tab-panel:not([hidden]) .terminal-host .xterm-rows');
async function enter(page: Page, command: string) {
  const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
  await input.focus(); await input.pressSequentially(command); await input.press('Enter');
}
async function options(page: Page) {
  const menu = page.locator('.terminal-tab-panel:not([hidden]) .terminal-tools');
  if (!await menu.evaluate(element => (element as HTMLDetailsElement).open)) await menu.locator('summary').click();
}
async function newTerminal(page: Page, folder?: string) {
  await page.getByLabel('New terminal', { exact: true }).click();
  if (folder) await page.locator('.terminal-folder-menu').getByRole('button', { name: folder, exact: true }).click();
  else await page.locator('.terminal-default-folder').click();
}
async function open(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
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
  await expect(output(page)).not.toContainText('session-retained');
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  expect(errors).toEqual([]);
});

test('a full terminal keeps the last input row inside its padded card after resize and maximize', async ({ page }) => {
  await open(page);
  await enter(page, 'seq 1 120');
  await expect(output(page)).toContainText('120');
  const host = page.locator('.terminal-tab-panel:not([hidden]) .terminal-host');
  async function expectInputFits() {
    await expect.poll(() => host.evaluate(element => {
      const screen = element.querySelector('.xterm-screen')?.getBoundingClientRect();
      const bounds = element.getBoundingClientRect();
      const padding = getComputedStyle(element);
      return !!screen && screen.height > 0 &&
        screen.bottom <= bounds.bottom - parseFloat(padding.paddingBottom) + .5 &&
        screen.right <= bounds.right - parseFloat(padding.paddingRight) + .5;
    })).toBe(true);
    await expect(host.locator('.xterm-rows > div').last()).toBeInViewport({ ratio: 1 });
  }
  await expectInputFits();
  await page.getByRole('button', { name: 'Maximize Terminal', exact: true }).click();
  await expectInputFits();
  await page.setViewportSize({ width: 820, height: 650 });
  await expectInputFits();
  await page.getByRole('button', { name: 'Restore Terminal', exact: true }).click();
  await expectInputFits();
  await options(page);
  await page.getByRole('button', { name: 'Increase terminal text size', exact: true }).click();
  await enter(page, 'echo current-input-visible');
  await expect(output(page)).toContainText('current-input-visible');
  await expectInputFits();
});

test('interrupt, restart confirmation, and missing runtime recovery', async ({ page }) => {
  await open(page);
  await page.getByRole('textbox', { name: 'Terminal input' }).press('Control+c');
  await expect(output(page)).toContainText('^C');
  await options(page);
  await page.getByRole('button', { name: 'Restart terminal', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Cancel' }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await page.getByLabel('Scenario', { exact: true }).selectOption('Missing runtime');
  await options(page);
  await page.getByRole('button', { name: 'Restart terminal', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Restart terminal' }).click();
  await expect(page.getByRole('alert')).toContainText('Runtime files are missing');
  await page.getByLabel('Scenario', { exact: true }).selectOption('CLI disabled');
  await page.locator('.terminal-recovery').getByRole('button', { name: 'Retry terminal' }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await enter(page, 'composer --version');
  await expect(output(page)).toContainText('Composer version');
});

test('session failures stay visible without moving the page or the shell focus', async ({ page }) => {
  const detail = `Could not end the session. ${'The shell is still running. '.repeat(40)}`;
  await page.route('**/src/ui-test/terminal.ts*', async route => {
    const url = new URL(route.request().url());
    if (url.searchParams.has('original')) { await route.continue(); return; }
    await route.fulfill({ contentType: 'text/javascript', body: `
      import { terminalRequest as original } from '/src/ui-test/terminal.ts?original=true';
      export function terminalRequest(command, args, ...rest) {
        if (command === 'terminal_close') throw new Error(${JSON.stringify(detail)});
        return original(command, args, ...rest);
      }
    ` });
  });
  await open(page);
  await enter(page, 'echo keep-focus');
  await expect(output(page)).toContainText('keep-focus');
  await options(page);
  const paste = page.getByRole('button', { name: 'Paste', exact: true });
  const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
  await page.evaluate(() => {
    document.querySelector('main')!.style.minHeight = '2400px';
    window.scrollTo(0, 700);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: async () => { throw new Error('Clipboard denied'); }, readText: async () => { throw new Error('Clipboard denied'); } }, configurable: true });
  });
  await input.evaluate(element => (element as HTMLElement).focus({ preventScroll: true }));
  const position = await page.evaluate(() => window.scrollY);
  expect(position).toBeGreaterThan(100);
  await paste.dispatchEvent('click');
  await expect(page.getByRole('alert')).toContainText('Could not paste.');
  await expect(page.locator('.terminal-notice')).toBeInViewport({ ratio: 1 });
  await expectStill(page, position);
  await expect(input).toBeFocused();
  await page.locator('.terminal-tools > summary').dispatchEvent('click');
  await page.getByRole('button', { name: 'Restart terminal', exact: true }).dispatchEvent('click');
  await page.getByRole('dialog').getByRole('button', { name: 'Restart terminal', exact: true }).click();
  const alert = page.getByRole('alert');
  await expect(alert).toContainText('Could not end the session.');
  await expect(page.locator('.terminal-notice')).toBeInViewport({ ratio: 1 });
  await expect(page.getByRole('button', { name: 'Dismiss terminal message', exact: true })).toBeInViewport({ ratio: 1 });
  expect(await page.locator('.terminal-notice').evaluate(element => element.getBoundingClientRect().height)).toBeLessThan(270);
  const dimensions = await page.locator('.terminal-notice-message').evaluate(element => ({ height: element.clientHeight, content: element.scrollHeight }));
  expect(dimensions.content).toBeGreaterThan(dimensions.height);
  await expectStill(page, position);
  const owner = await page.evaluate(() => document.activeElement?.closest('.sidebar') ? 'sidebar' : 'page');
  expect(owner).toBe('page');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Could not end the session.');
});

test('production preview cannot create a shell', async ({ page }) => {
  await page.goto('http://127.0.0.1:1422/');
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(page.getByText('Open the Fairway XMP desktop app to use the terminal.')).toBeVisible();
  await expect(page.locator('.terminal-host')).toHaveCount(0);
  await expect(page.getByLabel('New terminal', { exact: true })).toHaveCount(0);
});

test('resize failures are recoverable and wide windows preserve the session', async ({ page }, info) => {
  // Replace only the simulated IPC boundary: fail one resize as a native PTY
  // could, then record dimensions using the same limits as the Rust API.
  await page.route('**/src/ui-test/terminal.ts*', async route => {
    const url = new URL(route.request().url());
    if (url.searchParams.has('original')) { await route.continue(); return; }
    await route.fulfill({ contentType: 'text/javascript', body: `
      import { terminalRequest as original } from '/src/ui-test/terminal.ts?original=true';
      let rejectResize = true;
      export function terminalRequest(command, args, ...rest) {
        if (command === 'terminal_write' && args.data.includes(String.fromCharCode(27))) document.documentElement.dataset.terminalEscape = 'delivered';
        if (command === 'terminal_resize') {
          document.documentElement.dataset.terminalColumns = String(args.cols);
          if (args.cols < 2 || args.cols > 500 || args.rows < 1 || args.rows > 300) throw new Error('Terminal size is out of range.');
          if (rejectResize) { rejectResize = false; throw new Error('Temporary PTY resize failure.'); }
        }
        return original(command, args, ...rest);
      }
    ` });
  });
  await open(page);
  await expect(page.getByText('Terminal size could not be updated. Your commands are still running.')).toBeVisible();
  await enter(page, 'echo resize-survivor');
  await expect(output(page)).toContainText('resize-survivor');
  await page.getByRole('button', { name: 'Retry resize' }).click();
  await expect(page.getByRole('button', { name: 'Retry resize' })).toHaveCount(0);
  await page.setViewportSize({ width: 6000, height: 900 });
  await expect(page.locator('html')).toHaveAttribute('data-terminal-columns', '500');
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await enter(page, 'echo wide-window-survivor');
  await expect(output(page)).toContainText('wide-window-survivor');
  await page.getByRole('textbox', { name: 'Terminal input', exact: true }).press('Escape');
  await expect(page.locator('html')).toHaveAttribute('data-terminal-escape', 'delivered');
  await page.setViewportSize({ width: 820, height: 650 });
  await expect(output(page)).toContainText('resize-survivor');
  const path = info.outputPath('terminal-resize-recovery.png');
  await page.screenshot({ path, fullPage: true }); await info.attach('terminal-resize-recovery', { path, contentType: 'image/png' });
});

test('find, text size, and resizing preserve output and keyboard input', async ({ page }, info) => {
  await open(page);
  await enter(page, 'echo search-needle search-needle');
  await expect(output(page)).toContainText('search-needle');
  const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
  const shortcut = process.platform === 'darwin' ? 'Meta' : 'Control';
  await input.press(`${shortcut}+f`);
  const find = page.getByRole('textbox', { name: 'Find in terminal', exact: true });
  await expect(find).toBeFocused();
  await find.fill('search-needle');
  const search = page.getByRole('search', { name: 'Terminal output', exact: true });
  await expect(search.getByRole('status')).toHaveText('Match found');
  await options(page);
  await expect(page.getByRole('button', { name: 'Copy', exact: true })).toBeEnabled();
  await page.locator('.terminal-tools > summary').press('Escape');
  await find.focus();
  const selection = page.locator('.terminal-host .xterm-selection div').first();
  const before = await selection.boundingBox();
  await find.press('Enter');
  await expect.poll(async () => JSON.stringify(await selection.boundingBox())).not.toBe(JSON.stringify(before));
  await find.press('Shift+Enter');
  await expect.poll(async () => JSON.stringify(await selection.boundingBox())).toBe(JSON.stringify(before));
  await find.fill('missing-search-term');
  await expect(search.getByRole('status')).toHaveText('No matches');
  await find.press('Escape');
  await expect(search).toHaveCount(0);
  await expect(input).toBeFocused();
  await options(page);
  await page.getByRole('button', { name: 'Increase terminal text size', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Reset terminal text size (14 pixels)', exact: true })).toBeVisible();
  await input.press(`${shortcut}+-`);
  await expect(page.getByRole('button', { name: 'Reset terminal text size (13 pixels)', exact: true })).toBeVisible();
  await input.press(`${shortcut}+=`);
  await page.getByRole('button', { name: 'Fold sidebar', exact: true }).click();
  await page.getByRole('button', { name: 'Unfold sidebar', exact: true }).click();
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await enter(page, 'echo resized-input-works');
  await expect(output(page)).toContainText('resized-input-works');
  const path = info.outputPath('terminal-controls.png');
  await page.screenshot({ path, fullPage: true }); await info.attach('terminal-controls', { path, contentType: 'image/png' });
  await page.reload();
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await options(page);
  await expect(page.getByRole('button', { name: 'Reset terminal text size (14 pixels)', exact: true })).toBeVisible();
  await page.getByRole('textbox', { name: 'Terminal input', exact: true }).press(`${shortcut}+0`);
  await expect(page.getByRole('button', { name: 'Reset terminal text size (13 pixels)', exact: true })).toBeVisible();
});

test('an existing terminal stays accessible during a service operation', async ({ page }) => {
  await open(page);
  await enter(page, 'echo existing-session');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'Start all', exact: true }).click();
  await expect(page.locator('.operation-status')).toBeVisible();
  const terminal = page.getByRole('button', { name: 'Terminal', exact: true });
  expect(await terminal.isEnabled()).toBe(true);
  await terminal.click();
  await expect(output(page)).toContainText('existing-session');
  await page.getByRole('textbox', { name: 'Terminal input', exact: true }).press('Control+c');
  await expect(output(page)).toContainText('^C');
});

test('two terminal tabs keep independent sessions and start in the selected project folder', async ({ page }, info) => {
  await open(page);
  await enter(page, 'echo first-session-kept');
  await newTerminal(page, 'Student project');
  await expect(page.getByRole('tab', { name: 'Terminal 2 · Student project', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await expect(page.getByLabel('New terminal', { exact: true })).toBeDisabled();
  await enter(page, 'pwd');
  await expect(output(page)).toContainText('Student project');
  await enter(page, 'echo second-session-only');
  await page.getByRole('tab', { name: 'Terminal 1', exact: true }).click();
  await expect(output(page)).toContainText('first-session-kept');
  await expect(output(page)).not.toContainText('second-session-only');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.keyboard.press('Control+Backquote');
  await expect(page.getByRole('textbox', { name: 'Terminal input', exact: true })).toBeFocused();
  await expect(output(page)).toContainText('first-session-kept');
  await page.getByRole('tab', { name: 'Terminal 1', exact: true }).focus();
  await page.keyboard.press('ArrowRight');
  const second = page.getByRole('tab', { name: 'Terminal 2 · Student project', exact: true });
  await expect(second).toBeFocused();
  await expect(second).toHaveAttribute('aria-selected', 'true');
  await expect(output(page)).toContainText('second-session-only');
  await page.keyboard.press('Control+Backquote');
  await expect(page.getByRole('textbox', { name: 'Terminal input', exact: true })).toBeFocused();
  await expect(second).toBeVisible();
  const path = info.outputPath('terminal-two-tabs.png');
  await page.screenshot({ path, fullPage: true }); await info.attach('terminal-two-tabs', { path, contentType: 'image/png' });
  await page.getByRole('button', { name: 'Close terminal 2', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(second).toBeVisible();
  await page.getByRole('button', { name: 'Close terminal 2', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Close tab', exact: true }).click();
  await expect(second).toHaveCount(0);
  await expect(output(page)).toContainText('first-session-kept');
  await expect(page.getByLabel('New terminal', { exact: true })).toBeEnabled();
  await enter(page, 'echo first-still-accepts-input');
  await expect(output(page)).toContainText('first-still-accepts-input');
});

test('history preference applies to new sessions without interrupting existing commands', async ({ page }) => {
  await page.route('**/src/ui-test/terminal.ts*', async route => {
    if (new URL(route.request().url()).searchParams.has('original')) { await route.continue(); return; }
    await route.fulfill({ contentType: 'text/javascript', body: `
      import { terminalRequest as original } from '/src/ui-test/terminal.ts?original=true';
      export function terminalRequest(command, args, ...rest) {
        if (command === 'terminal_start') document.documentElement.dataset.historyPreference = String(args.request.rememberHistory);
        return original(command, args, ...rest);
      }
    ` });
  });
  await page.goto('/');
  await page.getByLabel('Example paths').selectOption('macOS');
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-history-preference', 'true');
  await enter(page, 'echo history-setting-keeps-session');
  await options(page);
  const history = page.getByRole('checkbox', { name: /Remember commands/ });
  await history.uncheck();
  await expect(page.locator('html')).toHaveAttribute('data-history-preference', 'true');
  await expect(output(page)).toContainText('history-setting-keeps-session');
  await page.getByRole('button', { name: 'Restart terminal', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Restart terminal', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-history-preference', 'false');
  await page.reload();
  await page.getByLabel('Example paths').selectOption('macOS');
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await options(page);
  await expect(page.getByRole('checkbox', { name: /Remember commands/ })).not.toBeChecked();
  await expect(page.locator('html')).toHaveAttribute('data-history-preference', 'false');
});

test('exit closes the extra terminal and preserves the first terminal output', async ({ page }) => {
  await open(page);
  await enter(page, 'echo first-terminal-preserved');
  await newTerminal(page);
  await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
  await enter(page, 'exit');
  await expect(page.getByRole('tab')).toHaveCount(1);
  await expect(page.getByRole('tab', { name: 'Terminal 1', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(output(page)).toContainText('first-terminal-preserved');
  await expect(page.getByRole('textbox', { name: 'Terminal input', exact: true })).toBeFocused();
});

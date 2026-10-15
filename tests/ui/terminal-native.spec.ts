import { test, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { join, resolve } from 'node:path';

// Opt-in after building the stdio example; ordinary browser CI uses the fixture.
// No listener is opened and the packaged runtime is read only.
test('React input reaches the real native PTY and packaged PHP/Composer', async ({ page }, info) => {
  test.skip(!process.env.FAIRWAY_TEST_RUNTIME, 'Set FAIRWAY_TEST_RUNTIME and build the terminal_bridge example.');
  const binary = resolve('src-tauri/target/debug/examples/terminal_bridge' + (process.platform === 'win32' ? '.exe' : ''));
  const root = info.outputPath('native terminal isolated');
  const child = spawn(binary, [root, process.env.FAIRWAY_TEST_RUNTIME!], { stdio: ['pipe', 'pipe', 'pipe'] });
  let next = 0;
  let stderr = '';
  const pending = new Map<number, { resolve: (value: unknown) => void; reject: (reason: Error) => void }>();
  const lines = createInterface({ input: child.stdout });
  child.stderr.on('data', chunk => { stderr += chunk; });
  lines.on('line', line => {
    const response = JSON.parse(line);
    const item = pending.get(response.requestId); pending.delete(response.requestId);
    if (response.error) item?.reject(new Error(response.error)); else item?.resolve(response.value);
  });
  child.on('exit', code => {
    pending.forEach(item => item.reject(new Error(`Native bridge exited ${code}: ${stderr}`))); pending.clear();
  });
  await page.exposeFunction('nativeTerminalInvoke', (command: string, args: Record<string, unknown> = {}) => new Promise((resolve, reject) => {
    const requestId = ++next; pending.set(requestId, { resolve, reject });
    child.stdin.write(JSON.stringify({ requestId, command, args }) + '\n');
  }));
  await page.addInitScript(() => {
    const bridge = window as unknown as { nativeTerminalInvoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>; __TAURI_INTERNALS__: unknown };
    bridge.__TAURI_INTERNALS__ = { invoke: bridge.nativeTerminalInvoke, transformCallback: () => 1, unregisterCallback: () => {} };
  });
  try {
    await page.goto('/');
    await page.getByRole('button', { name: 'Open terminal', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
    const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
    const rows = page.locator('.terminal-host .xterm-rows');
    const enter = async (command: string) => { await input.focus(); await input.pressSequentially(command); await input.press('Enter'); };
    await enter('php --version'); await expect(rows).toContainText(/PHP 8\.5\./);
    await enter('composer --version'); await expect(rows).toContainText('Composer version');
    await enter(process.platform === 'win32' ? 'cd' : 'pwd');
    await expect(rows).toContainText(join('Student May\'s home 日本', 'fairway', 'www'));
    await page.getByRole('button', { name: 'Environment', exact: true }).click();
    await page.getByRole('button', { name: 'Terminal', exact: true }).click();
    await expect(rows).toContainText('Composer version');
    await enter('exit 0');
    await expect(page.getByRole('status').filter({ hasText: /^Session ended$/ })).toBeVisible();
    await expect(rows).toContainText('exit 0');
    const path = info.outputPath('native-terminal.png');
    await page.screenshot({ path, fullPage: true }); await info.attach('native-terminal', { path, contentType: 'image/png' });
    expect(stderr).toBe('');
  } finally {
    child.stdin.end();
    await new Promise<void>(resolve => {
      const timer = setTimeout(() => { child.kill(); resolve(); }, 2000);
      child.once('exit', () => { clearTimeout(timer); resolve(); });
    });
    lines.close();
  }
});

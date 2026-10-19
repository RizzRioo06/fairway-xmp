import { test, expect } from '@playwright/test';
import { writeFile, mkdir, readFile, appendFile } from 'node:fs/promises';
import { join } from 'node:path';
import { nativeBridge } from './native-bridge';

test.use({ baseURL: 'http://127.0.0.1:1422' });
test('global logs show real PHP failures and HTTP history from the bundled web server', async ({ page, request }, info) => {
  test.skip(!process.env.FAIRWAY_TEST_RUNTIME, 'Set FAIRWAY_TEST_RUNTIME and build terminal_bridge.');
  test.setTimeout(90000);
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/');
    const snapshot = await page.evaluate(async () => {
      const api = (window as unknown as { __TAURI_INTERNALS__: { invoke: (command: string) => Promise<{ ports: { website: number } }> } }).__TAURI_INTERNALS__;
      return api.invoke('snapshot');
    });
    await writeFile(join(bridge.root, "Student May's home 日本/fairway/www/broken.php"), '<?php missing_function_for_log_test();');
    const root = join(bridge.root, "Student May's home 日本/fairway/www");
    await mkdir(join(root, 'shop'));
    await writeFile(join(root, 'shop/index.php'), "<?php echo 'shop route';");
    await writeFile(join(bridge.root, 'data 日本/sites.json'), JSON.stringify([
      { name: 'Lesson', domain: 'lesson', root, https: false, indexFallback: false },
      { name: 'Shop', domain: 'shop', root: join(root, 'shop'), https: true, indexFallback: true },
    ]));
    await page.getByRole('button', { name: 'Start service', exact: true }).first().click();
    await expect(page.locator('.service-card').first()).toContainText('Running', { timeout: 20000 });
    // Reload and graceful stop use the same Unicode data path as startup.
    await page.evaluate(async root => {
      const api = (window as unknown as { __TAURI_INTERNALS__: { invoke: (command: string, args: unknown) => Promise<unknown> } }).__TAURI_INTERNALS__;
      await api.invoke('save_project', { previous: 'lesson', project: { name: 'Lesson renamed', domain: 'lesson', root, https: false, indexFallback: false } });
      await api.invoke('service_action', { service: 'nginx', action: 'stop' });
      await api.invoke('service_action', { service: 'nginx', action: 'start' });
    }, root);
    const config = await readFile(join(bridge.root, 'data 日本/config/nginx.conf'), 'utf8');
    const httpsPort = config.match(/listen 127\.0\.0\.1:(\d+) ssl/)![1];
    const base = `http://127.0.0.1:${snapshot.ports.website}`;
    await request.get(`${base}/broken.php?private_value=not-recorded`);
    await request.get(`${base}/missing-log-test`);
    await request.get(`${base}/မြန်မာ/日本`);
    await request.get(`${base}/broken.php`, { headers: { Host: 'lesson.test' } });
    await request.get(`https://127.0.0.1:${httpsPort}/pretty-route?private_value=not-recorded`, { headers: { Host: 'shop.test' }, ignoreHTTPSErrors: true });
    await page.getByRole('button', { name: 'Logs', exact: true }).click();
    const output = page.getByRole('region', { name: 'Log entries', exact: true });
    await expect(output).toContainText('missing_function_for_log_test', { timeout: 15000 });
    await expect(output).toContainText('→ 404');
    await expect(output).toContainText('/မြန်မာ/日本');
    await expect(output).not.toContainText('NOTICE: ready to handle connections');
    await page.getByLabel('Log site').selectOption('shop.test');
    await page.getByRole('checkbox', { name: 'Problems only' }).uncheck();
    await expect(output).toContainText(`https://shop.test:${httpsPort} /pretty-route`);
    await expect(output).toContainText('missing_function_for_log_test');
    await expect(output).toContainText('Shared PHP');
    await expect(output).not.toContainText('http://lesson.test:');
    await page.getByLabel('Log site').selectOption('lesson.test');
    await expect(output).toContainText('http://lesson.test:');
    await expect(output).not.toContainText('https://shop.test:');
    await page.getByLabel('Log site').selectOption('');
    await page.getByLabel('Log source').selectOption('php');
    await expect(output).toContainText('PHP Fatal error');
    await expect(output).toContainText('Stack trace:');
    await page.getByLabel('Log source').selectOption('requests');
    await expect(output).not.toContainText('private_value');
    await page.getByRole('checkbox', { name: 'Problems only' }).uncheck();
    await request.get(`${base}/`);
    await expect(output).toContainText('→ 200');
    await expect(output).toContainText(`https://shop.test:${httpsPort} /pretty-route`);
    await expect(output).toContainText('http://lesson.test:');
    await appendFile(join(bridge.root, 'data 日本/logs/access.log'), String.raw`127.0.0.1 - - [18/Oct/2026:09:35:34 +0630] "\x16\x03\x01\x05\xF7" 400 157 "-" "-"` + '\n');
    await expect(output).toContainText('HTTPS traffic reached an HTTP port');
    await expect(output).not.toContainText(String.raw`\x16\x03`);
    await page.getByRole('button', { name: 'Pause', exact: true }).click();
    await request.get(`${base}/paused-request`);
    await page.waitForTimeout(1800);
    await expect(output).not.toContainText('paused-request');
    await page.getByRole('button', { name: 'Resume', exact: true }).click();
    await expect(output).toContainText('paused-request');
    // Reading older output stays stable as fresh entries arrive.
    await page.getByLabel('Log source').selectOption('php');
    const phpLog = join(bridge.root, 'data 日本/logs/php.log');
    await writeFile(phpLog, Array.from({ length: 60 }, (_, i) => `[18-Oct-2026 12:00:${String(i).padStart(2, '0')} UTC] PHP Warning: history ${i}\n`).join(''));
    await expect(output).toContainText('history 59');
    await output.evaluate(element => { element.scrollTop = 0; element.dispatchEvent(new Event('scroll')); });
    await appendFile(phpLog, '[18-Oct-2026 12:01:00 UTC] PHP Warning: appended history\n');
    await expect(output).toContainText('appended history');
    expect(await output.evaluate(element => element.scrollTop)).toBeLessThan(4);
    await writeFile(phpLog, '[18-Oct-2026 12:02:00 UTC] PHP Warning: replacement history\n');
    await expect(output).toContainText('replacement history');
    await expect(output).not.toContainText('history 59');
    const screenshot = info.outputPath('native-global-logs.png');
    await page.screenshot({ path: screenshot, fullPage: true });
    await info.attach('native-global-logs', { path: screenshot, contentType: 'image/png' });
  } finally { await bridge.close(); }
});

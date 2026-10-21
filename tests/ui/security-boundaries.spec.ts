import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import type { Snapshot } from '../../src/types';
const externalRequests = new WeakMap<Page, string[]>();
const csp = JSON.parse(readFileSync(new URL('../../src-tauri/tauri.conf.json', import.meta.url), 'utf8')).app.security.csp as string;

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  const initial = await page.evaluate(async () => {
    const path = '/src/ui-test/backend.ts';
    return (await import(path)).invoke('snapshot') as Promise<Snapshot>;
  });
  await page.addInitScript((snapshot: Snapshot) => {
    const data = structuredClone(snapshot); data.httpsTrustUpdate = true;
    const callbacks = new Map<number, (event: { payload: unknown }) => void>(); let next = 0;
    let cancelled = true;
    window.addEventListener('test-approve-trust', () => { cancelled = false; });
    const email = { ID: 'boundary', Read: false, From: { Name: 'Local test', Address: 'local@shop.test' }, To: [], Cc: [], Subject: 'Boundary email', Date: new Date().toISOString(), Created: new Date().toISOString(), Snippet: 'Readable email', Attachments: [], Text: 'Readable text version', HTML: '<p id="readable" style="color:rgb(20,80,30)">Readable email</p><img alt="Embedded image" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"><script>parent.document.documentElement.dataset.emailScript="ran"</script><base href="https://blocked.invalid/"><object data="/?blocked-object"></object><iframe src="https://blocked.invalid/frame"></iframe><img src="https://blocked.invalid/tracker"><form action="https://blocked.invalid/submit" target="_top"><button>Submit email form</button></form><a href="https://blocked.invalid/navigate" target="_top">Email link</a>' };
    document.addEventListener('securitypolicyviolation', event => {
      const old: string[] = JSON.parse(document.documentElement.dataset.violations ?? '[]');
      document.documentElement.dataset.violations = JSON.stringify([...old, event.effectiveDirective]);
    });
    window.__TAURI_INTERNALS__ = {
      transformCallback(callback) { callbacks.set(++next, callback); return next; }, unregisterCallback(id) { callbacks.delete(id); },
      async invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
        if (command === 'plugin:event|listen') return ++next as T;
        if (command === 'plugin:event|unlisten') return undefined as T;
        if (command === 'snapshot') return structuredClone(data) as T;
        if (command === 'startup_status') return { blocking: false, errors: [], dataDir: data.dataDir } as T;
        if (command === 'set_project_https') {
          document.documentElement.dataset.httpsRequest = JSON.stringify(args);
          if (cancelled) throw new Error('Certificate approval was cancelled. Existing HTTPS sites were kept.');
          data.httpsTrustUpdate = false; return structuredClone(data) as T;
        }
        if (command === 'save_project') { document.documentElement.dataset.saved = 'yes'; return structuredClone(data) as T; }
        if (command === 'open_target') { document.documentElement.dataset.opened = String(args.target); return undefined as T; }
        if (command === 'mail_inbox') {
          const request = args.request as { action: string };
          if (request.action === 'list') return { messages: [{ ...email, Attachments: 0 }], messages_count: 1, messages_unread: 1 } as T;
          return email as T;
        }
        throw new Error(`Unexpected command: ${command}`);
      },
    };
  }, initial);
  // Exercise the exact configured production CSP, including inherited srcdoc
  // policy. Debug-protocol evaluation supplies only the controlled native bridge.
  await page.route('http://127.0.0.1:1422/', async route => {
    const response = await route.fetch();
    await route.fulfill({ response, headers: { ...response.headers(), 'content-security-policy': csp } });
  });
  externalRequests.set(page, []);
  await page.route('**/blocked.invalid/**', route => { externalRequests.get(page)!.push(route.request().url()); return route.abort(); });
  await page.goto('http://127.0.0.1:1422/');
  await expect(page.getByRole('button', { name: 'Stop all', exact: true })).toBeVisible();
});

test('certificate update cancellation keeps HTTPS selected and retry retires the pending update', async ({ page }) => {
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
  const update = page.getByRole('button', { name: 'Update certificate trust', exact: true });
  await expect(update).toBeVisible();
  const enabled = page.getByRole('switch', { name: 'HTTPS for My Shop', exact: true });
  await expect(enabled).toBeChecked();
  await update.click();
  await expect(page.getByRole('alert')).toContainText('approval was cancelled');
  await expect(enabled).toBeChecked(); await expect(update).toBeEnabled();
  expect(JSON.parse((await page.locator('html').getAttribute('data-https-request'))!)).toEqual({ domain: 'shop', enabled: true });
  await page.evaluate(() => window.dispatchEvent(new Event('test-approve-trust')));
  await update.click();
  await expect(update).toHaveCount(0); await expect(enabled).toBeChecked();
});

test('production CSP allows mail rendering, native external opens and React dialog submission', async ({ page }) => {
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await page.getByRole('list', { name: 'Emails', exact: true }).getByRole('button', { name: /Boundary email/ }).click();
  const frame = page.frameLocator('iframe[title="Email content"]');
  await expect(frame.locator('#readable')).toHaveText('Readable email');
  await expect(frame.locator('#readable')).toHaveCSS('color', 'rgb(20, 80, 30)');
  await expect(page.locator('iframe[title="Email content"]')).toHaveAttribute('sandbox', 'allow-same-origin');
  expect(await page.locator('html').getAttribute('data-email-script')).toBeNull();
  await expect.poll(() => frame.getByAltText('Embedded image').evaluate(img => (img as HTMLImageElement).naturalWidth)).toBe(1);
  expect(externalRequests.get(page)).toEqual([]);
  await frame.getByRole('link', { name: 'Email link', exact: true }).click();
  await expect(page).toHaveURL('http://127.0.0.1:1422/');
  await page.getByRole('tab', { name: 'Text', exact: true }).click();
  await expect(page.locator('.mail-text')).toHaveText('Readable text version');
  await page.getByRole('button', { name: 'Open in browser', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-opened', 'mail');
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
  await page.getByRole('button', { name: 'Actions for My Shop', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Edit site', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Edit site', exact: true });
  await dialog.getByLabel('Site name', { exact: true }).fill('Updated title');
  await dialog.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-saved', 'yes');
  await expect(dialog).toHaveCount(0);
});

test('production CSP blocks object loads, base changes and unhandled form navigation', async ({ page }) => {
  const before = page.url();
  await page.evaluate(() => {
    const base = document.createElement('base'); base.href = 'https://blocked.invalid/'; document.head.append(base);
    const object = document.createElement('object'); object.data = '/?blocked-object'; document.body.append(object);
  });
  await expect.poll(() => page.evaluate(() => JSON.parse(document.documentElement.dataset.violations ?? '[]'))).toEqual(expect.arrayContaining(['base-uri', 'object-src']));
  await page.evaluate(() => { const form = document.createElement('form'); form.action = 'https://blocked.invalid/submit'; document.body.append(form); form.submit(); });
  // Chromium reports a blank provisional URL for a CSP-blocked submission;
  // check the still-active document and UI instead of that provisional frame.
  await expect.poll(() => page.evaluate(() => location.href)).toBe(before);
  expect(externalRequests.get(page)).toEqual([]);
  expect(await page.evaluate(() => document.baseURI)).toBe(before);
  await expect.poll(() => page.evaluate(() => JSON.parse(document.documentElement.dataset.violations ?? '[]'))).toEqual(expect.arrayContaining(['base-uri', 'object-src', 'form-action']));
  await page.evaluate(() => { [...document.querySelectorAll('button')].find(button => button.textContent?.trim() === 'Manage Sites')!.click(); });
  await expect.poll(() => page.evaluate(() => [...document.querySelectorAll('h1')].find(heading => heading.getClientRects().length > 0)?.textContent)).toBe('Your sites');
});

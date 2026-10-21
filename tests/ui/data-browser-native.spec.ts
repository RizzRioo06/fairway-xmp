import { test, expect } from '@playwright/test';
import { nativeBridge } from './native-bridge';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';

// Use the same production frontend that is packaged in the desktop app.
test.use({ baseURL: 'http://127.0.0.1:1422' });

test('desktop data browser starts bundled MySQL and edits real rows', async ({ page }, info) => {
  test.skip(!process.env.FAIRWAY_TEST_RUNTIME, 'Set FAIRWAY_TEST_RUNTIME and build the terminal_bridge example.');
  test.setTimeout(120000);
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/');
    await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
    await page.getByRole('button', { name: 'Start MySQL', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Database', exact: true })).toBeEnabled({ timeout: 60000 });
    await page.getByRole('button', { name: 'Database', exact: true }).click();
    let dialog = page.getByRole('dialog', { name: 'Create database', exact: true });
    await dialog.getByLabel('Database name', { exact: true }).fill('native_classroom');
    await dialog.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(page.getByLabel('Database', { exact: true })).toHaveValue('native_classroom');
    await page.getByRole('button', { name: 'Create table', exact: true }).first().click();
    dialog = page.getByRole('dialog', { name: 'Create table', exact: true });
    await dialog.getByLabel('Table name', { exact: true }).fill('students');
    await dialog.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await page.getByRole('button', { name: 'Insert row', exact: true }).click();
    dialog = page.getByRole('dialog', { name: 'Insert row', exact: true });
    await expect(dialog.getByLabel('name value', { exact: true })).toBeEnabled();
    await dialog.getByLabel('name value', { exact: true }).fill("Student May's 日本");
    await dialog.getByRole('button', { name: 'Save', exact: true }).click();
    const rows = page.getByRole('region', { name: 'Table rows', exact: true });
    await expect(rows).toContainText("Student May's 日本");
    await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
    dialog = page.getByRole('dialog', { name: 'Edit row', exact: true });
    await dialog.getByLabel('name value', { exact: true }).fill('Edited through native PHP');
    await dialog.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(rows).toContainText('Edited through native PHP');
    await page.getByRole('tab', { name: 'SQL query', exact: true }).click();
    for (const [sql, summary] of [
      ["INSERT INTO students (name) VALUES ('SQL result label')", '1 row inserted'],
      ["UPDATE students SET name='SQL updated label' WHERE name='SQL result label'", '1 row updated'],
      ["UPDATE students SET name='SQL updated label' WHERE name='SQL updated label'", '0 rows updated'],
      ["DELETE FROM students WHERE name='SQL updated label'", '1 row deleted'],
    ]) {
      await page.locator('#db-sql').fill(sql);
      await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
      await expect(page.getByRole('heading', { name: summary, exact: true })).toBeVisible();
      await expect(page.locator('.db-browser')).toHaveAttribute('aria-busy', 'false');
    }
    await page.locator('#db-sql').fill('SELECT 9007199254740993 AS exact_number, name FROM students');
    await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
    await expect(page.getByRole('region', { name: 'Query result 1', exact: true })).toContainText('9007199254740993');
    await expect(page.locator('.db-browser')).toHaveAttribute('aria-busy', 'false');
    await page.getByRole('button', { name: 'Database actions', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Import SQL file…', exact: true }).click();
    const importing = page.getByRole('dialog', { name: 'Import SQL', exact: true });
    await importing.getByLabel('Type native_classroom to confirm', { exact: true }).fill('native_classroom');
    await importing.getByRole('button', { name: 'Import SQL', exact: true }).click();
    await expect(importing).toHaveCount(0);
    await expect(page.getByRole('status').filter({ hasText: /SQL imported into native_classroom/ })).toBeVisible();
    await page.getByRole('button', { name: 'imported_from_file', exact: true }).click();
    await page.getByRole('tab', { name: 'Rows', exact: true }).click();
    await expect(rows).toContainText('Imported through the desktop');
    await page.getByRole('tab', { name: 'SQL query', exact: true }).click();
    await page.locator('#db-sql').fill('SELECT 1;'.repeat(11));
    await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
    await expect(page.getByRole('region', { name: /^Query result \d+$/ })).toHaveCount(10);
    await expect(page.locator('.db-browser')).toHaveAttribute('aria-busy', 'false');
    // Exercise the real UI cancellation command and phase events, with a fresh
    // dump that cannot finish before Cancel import is clicked.
    await writeFile(join(bridge.root, "Student May's home 日本/fairway/www/Project with spaces 日本/school-import.sql"), 'SELECT SLEEP(10); CREATE TABLE must_not_import(id INT);');
    await page.getByRole('button', { name: 'Database actions', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Import SQL file…', exact: true }).click();
    const canceling = page.getByRole('dialog', { name: 'Import SQL', exact: true });
    const confirmation = canceling.getByLabel('Type native_classroom to confirm', { exact: true });
    await confirmation.fill('native_classroom');
    await canceling.getByRole('button', { name: 'Import SQL', exact: true }).click();
    await expect(canceling.getByRole('status')).toHaveText('Importing SQL…');
    await canceling.getByRole('button', { name: 'Cancel import', exact: true }).click();
    await expect(canceling.getByRole('alert')).toContainText('canceled');
    await expect(canceling.getByRole('alert')).toContainText('Backup saved:');
    await expect(confirmation).toHaveValue('');
    await expect(canceling.getByRole('button', { name: 'Import SQL', exact: true })).toBeDisabled();
    await canceling.getByRole('button', { name: 'Cancel', exact: true }).click();
    await expect(page.getByRole('button', { name: 'must_not_import', exact: true })).toHaveCount(0);
    const path = info.outputPath('native-data-browser.png');
    await page.screenshot({ path, fullPage: true });
    await info.attach('native-data-browser', { path, contentType: 'image/png' });
    expect(bridge.errors()).toBe('');
  } finally {
    await bridge.close();
  }
});

test('desktop row forms preserve large originals, stage large edits and reject concurrent changes', async ({ page }, info) => {
  test.skip(!process.env.FAIRWAY_TEST_RUNTIME, 'Set FAIRWAY_TEST_RUNTIME and build the terminal_bridge example.');
  test.setTimeout(120000);
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/');
    await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
    await page.getByRole('button', { name: 'Start MySQL', exact: true }).click();
    await page.getByRole('button', { name: 'Database', exact: true }).click({ timeout: 60000 });
    let dialog = page.getByRole('dialog', { name: 'Create database', exact: true });
    await dialog.getByLabel('Database name', { exact: true }).fill('large_row_forms');
    await dialog.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    async function query(sql: string) {
      await page.getByRole('tab', { name: 'SQL query', exact: true }).click();
      await page.locator('#db-sql').fill(sql);
      await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
      await expect(page.locator('.db-browser')).toHaveAttribute('aria-busy', 'false');
      await expect(page.getByLabel('Query results', { exact: true })).toBeVisible();
    }
    await query("CREATE TABLE values_to_edit(id BIGINT PRIMARY KEY, body LONGTEXT, note VARCHAR(100) CHARACTER SET latin1, b BIT(3), d DOUBLE); INSERT INTO values_to_edit VALUES(9007199254740993,REPEAT('x',4194304),'café',b'101',1.2345678901234567);");
    await page.getByRole('tab', { name: 'Rows', exact: true }).click();
    const rows = page.getByRole('region', { name: 'Table rows', exact: true });
    await expect(rows).toContainText('0x05'); await expect(rows).toContainText('1.2345678901234567');
    await page.getByRole('button', { name: 'View row 1', exact: true }).click();
    const view = page.getByRole('dialog', { name: 'View row', exact: true });
    await expect(view).toContainText('4,194,304 characters');
    await view.getByRole('button', { name: 'Read body in parts', exact: true }).click();
    await expect(view).toContainText('Characters 1–131072 of 4,194,304');
    await view.getByRole('button', { name: 'Edit', exact: true }).click();
    dialog = page.getByRole('dialog', { name: 'Edit row', exact: true });
    await expect(dialog.getByLabel('body value mode', { exact: true })).toHaveValue('unchanged');
    await dialog.getByLabel('note value', { exact: true }).fill('café edited');
    await dialog.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await query('SELECT OCTET_LENGTH(body) AS bytes, note FROM values_to_edit');
    const result = page.getByRole('region', { name: 'Query result 1', exact: true });
    await expect(result).toContainText('4194304'); await expect(result).toContainText('café edited');
    await page.getByRole('tab', { name: 'Rows', exact: true }).click();
    await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
    await dialog.getByLabel('body value mode', { exact: true }).selectOption('value');
    await dialog.getByLabel('body value', { exact: true }).fill('new🟢'.repeat(400000));
    await dialog.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await query('SELECT OCTET_LENGTH(body) AS bytes, RIGHT(body,4) AS tail FROM values_to_edit');
    await expect(result).toContainText('2800000'); await expect(result).toContainText('new🟢');
    await page.getByRole('tab', { name: 'Rows', exact: true }).click();
    await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
    await dialog.getByLabel('note value', { exact: true }).fill('must remain unsaved');
    await page.evaluate(async () => {
      await window.__TAURI_INTERNALS__!.invoke('data_browser', { id: crypto.randomUUID(), request: { op: 'query', database: 'large_row_forms', sql: "UPDATE values_to_edit SET body=CONCAT(body,'external')" } });
    });
    await dialog.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(dialog.getByRole('alert')).toContainText('row changed');
    await expect(dialog.getByLabel('note value', { exact: true })).toHaveValue('must remain unsaved');
    await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
    await query('SELECT note, RIGHT(body,8) AS tail FROM values_to_edit');
    await expect(result).toContainText('café edited'); await expect(result).toContainText('external');
    expect(bridge.errors()).toBe('');
  } finally { await bridge.close(); }
});

import { expect, test, type Page, type TestInfo } from '@playwright/test';

async function open(page: Page) {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Insert row', exact: true })).toBeEnabled();
}
async function action(page: Page, scope: 'Database' | 'Table', name: string) {
  await page.getByRole('button', { name: `${scope} actions`, exact: true }).click();
  await page.getByRole('menuitem', { name, exact: true }).click();
}
async function capture(page: Page, info: TestInfo, name: string) {
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  const path = info.outputPath(`${name}.png`);
  await page.screenshot({ path, fullPage: true, animations: 'disabled' });
  await info.attach(name, { path, contentType: 'image/png' });
}

test('SQL import confirms the database and backups are available outside the CLI disclosure', async ({ page }) => {
  await open(page);
  const backups = page.locator('.db-footer').getByRole('button', { name: 'Open backups', exact: true });
  await expect(backups).toBeVisible();
  expect(await page.locator('.db-cli').evaluate(element => (element as HTMLDetailsElement).open)).toBe(false);
  await action(page, 'Database', 'Import SQL file…');
  const dialog = page.getByRole('dialog', { name: 'Import SQL', exact: true });
  await expect(dialog).toContainText('school.sql');
  await expect(dialog).toContainText('only this target is backed up automatically');
  const submit = dialog.getByRole('button', { name: 'Import SQL', exact: true });
  await expect(submit).toBeDisabled();
  await dialog.getByLabel('Type fairway to confirm', { exact: true }).fill('other_database');
  await expect(submit).toBeDisabled();
  await dialog.getByLabel('Type fairway to confirm', { exact: true }).fill('fairway');
  await submit.click();
  await expect(dialog).toHaveCount(0);
  await expect(page.getByRole('status').filter({ hasText: /SQL imported into fairway/ })).toBeVisible();
  await expect(backups).toBeVisible();
});

test('controls align and the table sidebar resizes, collapses, and restores without losing data', async ({ page }, info) => {
  await open(page);
  const database = await page.getByLabel('Database', { exact: true }).boundingBox();
  const create = await page.getByRole('button', { name: 'Database', exact: true }).boundingBox();
  expect(database).not.toBeNull(); expect(create).not.toBeNull();
  expect(database!.height).toBe(36);
  expect(Math.abs(database!.height - create!.height)).toBeLessThanOrEqual(1);
  if (Math.abs(database!.y - create!.y) < 40) expect(Math.abs(database!.y + database!.height - create!.y - create!.height)).toBeLessThanOrEqual(1);
  const insert = (await page.getByRole('button', { name: 'Insert row', exact: true }).boundingBox())!;
  const actions = (await page.getByRole('button', { name: 'Table actions', exact: true }).boundingBox())!;
  expect(Math.abs(insert.y - actions.y)).toBeLessThanOrEqual(1);
  expect(actions.x - insert.x - insert.width).toBeGreaterThanOrEqual(0);
  expect(actions.x - insert.x - insert.width).toBeLessThanOrEqual(10);

  const sidebar = page.getByRole('complementary', { name: 'Tables', exact: true });
  const detail = page.getByRole('region', { name: 'Database workspace', exact: true });
  const divider = page.getByRole('separator', { name: 'Resize table sidebar', exact: true });
  const before = (await sidebar.boundingBox())!.width;
  const grip = divider.locator('span'); await grip.scrollIntoViewIfNeeded();
  const handle = (await grip.boundingBox())!;
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle.x + handle.width / 2 + 80, handle.y + handle.height / 2, { steps: 8 });
  await page.mouse.up();
  await expect.poll(async () => (await sidebar.boundingBox())!.width).toBeGreaterThan(before + 60);
  const expanded = (await sidebar.boundingBox())!.width;
  await divider.press('ArrowLeft');
  await expect.poll(async () => (await sidebar.boundingBox())!.width).toBe(expanded - 16);
  const remembered = (await sidebar.boundingBox())!.width;
  const detailBefore = (await detail.boundingBox())!.width;
  await capture(page, info, 'data-browser-resized');
  const hideTables = page.locator('.db-detail-title').getByRole('button', { name: 'Hide tables', exact: true });
  await expect(hideTables).toHaveText('');
  await hideTables.click();
  await expect(sidebar).toHaveCount(0);
  await expect(divider).toHaveCount(0);
  await expect.poll(async () => (await detail.boundingBox())!.width).toBeGreaterThan(detailBefore + remembered);
  await expect(page.getByRole('region', { name: 'Table rows', exact: true })).toContainText('Student May');
  await capture(page, info, 'data-browser-collapsed');
  const showTables = page.locator('.db-detail-title').getByRole('button', { name: 'Show tables', exact: true });
  await expect(showTables).toHaveText('');
  await showTables.click();
  await expect.poll(async () => (await sidebar.boundingBox())!.width).toBe(remembered);
  await divider.press('End');
  await page.setViewportSize({ width: 820, height: 650 });
  await expect.poll(async () => (await detail.boundingBox())!.width).toBeGreaterThanOrEqual(280);
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);

  await page.getByRole('button', { name: 'Create table', exact: true }).first().click();
  const dialog = page.getByRole('dialog', { name: 'Create table', exact: true });
  const columnName = await dialog.getByLabel('Column name', { exact: true }).first().boundingBox();
  const columnType = await dialog.getByRole('combobox', { name: 'Type', exact: true }).first().boundingBox();
  const defaultSelect = await dialog.getByRole('combobox', { name: 'Default', exact: true }).first().boundingBox();
  expect(columnName!.height).toBe(36); expect(columnType!.height).toBe(36); expect(defaultSelect!.height).toBe(36);
  await capture(page, info, 'data-browser-column-controls');
});

test('database switching publishes one consistent workspace and sidebar navigation preserves it', async ({ page }, info) => {
  await open(page);
  await page.getByRole('button', { name: 'Hide tables', exact: true }).click();
  const database = page.getByLabel('Database', { exact: true });
  await database.evaluate(element => {
    const browser = element.closest('.db-browser')!;
    const failures: string[] = [];
    function sample() {
      const db = (element as HTMLSelectElement).value;
      const title = browser.querySelector('.db-detail h2')?.textContent;
      if ((db === 'fairway' && title !== 'students') || (db === 'school' && title !== 'school')) failures.push(`${db}:${title}`);
      if (document.querySelector('.operation-status')) failures.push('global busy indicator');
      if (db === 'fairway' && browser.querySelector('.db-tabs button:disabled')) failures.push('dimmed tab');
      browser.setAttribute('data-switch-failures', JSON.stringify(failures));
    }
    const observer = new MutationObserver(sample);
    observer.observe(browser, { childList: true, subtree: true, attributes: true, attributeFilter: ['disabled', 'aria-busy'] });
    browser.addEventListener('stop-switch-observation', () => observer.disconnect(), { once: true });
  });
  await database.selectOption('school');
  await expect(page.getByRole('heading', { name: 'school', exact: true })).toBeVisible();
  await page.locator('.db-browser').evaluate(element => element.dispatchEvent(new Event('stop-switch-observation')));
  const failures = JSON.parse((await page.locator('.db-browser').getAttribute('data-switch-failures')) ?? '[]') as string[];
  expect(failures).toEqual([]);
  await database.selectOption('fairway');
  await expect(page.getByRole('button', { name: 'Insert row', exact: true })).toBeEnabled();
  await page.getByRole('tab', { name: 'Structure', exact: true }).click();
  const structure = page.getByRole('region', { name: 'Table structure', exact: true });
  await expect(structure).toBeVisible();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
  await expect(structure).toBeVisible();
  await expect(page.getByRole('button', { name: 'Show tables', exact: true })).toBeVisible();
  await expect(page.getByText('Loading data browser…', { exact: true })).toHaveCount(0);
  await expect(page.locator('.operation-status')).toHaveCount(0);
  await capture(page, info, 'data-browser-smooth-navigation');
});

test('returning during initial database loading recovers without an empty or stuck view', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Table rows', exact: true })).toContainText('Student May');
  await expect(page.locator('.db-browser')).toHaveAttribute('aria-busy', 'false');
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('database rows preserve exact values, pagination, and protect rows without keys and views', async ({ page }, info) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
  await page.getByRole('button', { name: 'Start MySQL', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Insert row', exact: true })).toBeEnabled();
  const rows = page.getByRole('region', { name: 'Table rows', exact: true });
  await expect(rows).toContainText('9007199254740993');
  await page.setViewportSize({ width: 1080, height: 1500 });
  await expect.poll(async () => (await rows.boundingBox())!.height).toBeGreaterThan(430);
  await expect.poll(async () => (await page.locator('.db-pagination').boundingBox())!.y).toBeLessThan(1500);
  await page.setViewportSize({ width: 1080, height: 900 });
  await expect.poll(async () => (await rows.boundingBox())!.height).toBeLessThan(430);
  await expect.poll(async () => {
    const pager = (await page.locator('.db-pagination').boundingBox())!;
    return pager.y + pager.height;
  }).toBeLessThanOrEqual(900);
  await expect(rows).toContainText('NULL');
  await page.getByRole('button', { name: 'View row 1', exact: true }).click();
  let view = page.getByRole('dialog', { name: 'View row', exact: true });
  await expect(view.locator('form, input, textarea, select')).toHaveCount(0);
  await expect(view.getByText('Student May', { exact: true })).toBeVisible();
  await expect(view.getByText('NULL', { exact: true })).toBeVisible();
  await view.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(view).toHaveCount(0);
  await page.getByRole('button', { name: 'View row 2', exact: true }).click();
  view = page.getByRole('dialog', { name: 'View row', exact: true });
  await expect(view.getByText('empty', { exact: true })).toBeVisible();
  await view.getByRole('button', { name: 'Edit', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Edit row', exact: true })).toBeVisible();
  await page.getByRole('dialog', { name: 'Edit row', exact: true }).getByRole('button', { name: 'Cancel', exact: true }).click();
  await capture(page, info, 'data-browser-rows');
  await page.getByRole('button', { name: 'Next page', exact: true }).click();
  await expect(page.getByText('51–55 of 55', { exact: true })).toBeVisible();
  await expect(page.getByRole('combobox', { name: 'Filter column', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Previous page', exact: true }).click();
  await expect(page.getByText('1–50 of 55', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'messages_without_key', exact: true }).click();
  await expect(page.getByText(/No non-null primary or unique key/)).toBeVisible();
  const shortTable = rows.locator('table');
  await expect.poll(async () => {
    const frame = (await rows.boundingBox())!;
    const grid = (await shortTable.boundingBox())!;
    return frame.height - grid.height;
  }).toBeLessThan(4);
  const actionCell = rows.locator('tbody td').first();
  await expect(actionCell).toHaveCSS('display', 'table-cell');
  await expect(actionCell).toHaveCSS('vertical-align', 'middle');
  const rowBox = (await rows.locator('tbody tr').boundingBox())!;
  const viewBox = (await rows.getByRole('button', { name: 'View row 1', exact: true }).boundingBox())!;
  expect(Math.abs(viewBox.y + viewBox.height / 2 - (rowBox.y + rowBox.height / 2))).toBeLessThanOrEqual(1);
  await expect(rows.getByRole('button', { name: 'View row 1', exact: true })).toBeEnabled();
  await expect(rows.getByRole('button', { name: /^Edit row/ })).toHaveCount(0);
  await page.getByRole('button', { name: 'student_names view', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Insert row', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'View row 1', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: /^Edit row/ })).toHaveCount(0);
  await expect(page.getByText('This view is read-only. Use SQL for advanced changes.')).toBeVisible();
  await page.getByRole('button', { name: 'View row 1', exact: true }).click();
  view = page.getByRole('dialog', { name: 'View row', exact: true });
  await expect(view.getByRole('button', { name: 'Edit', exact: true })).toHaveCount(0);
  await expect(view.getByRole('button', { name: 'Copy row JSON', exact: true })).toBeEnabled();
});

test('row viewer copies exact values and JSON, keeps full text readable, and handles clipboard errors', async ({ page }) => {
  const copied: string[] = [];
  await page.exposeFunction('recordRowCopy', (value: string) => { copied.push(value); });
  await page.addInitScript(() => {
    const bridge = window as unknown as { recordRowCopy: (value: string) => Promise<void> };
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: bridge.recordRowCopy } });
  });
  await open(page);
  await page.getByRole('button', { name: 'View row 1', exact: true }).click();
  let view = page.getByRole('dialog', { name: 'View row', exact: true });
  await expect(view.locator('form, input, textarea, select')).toHaveCount(0);
  await view.getByRole('button', { name: 'Copy id', exact: true }).click();
  await expect.poll(() => copied.at(-1)).toBe('9007199254740993');
  await view.getByRole('button', { name: 'Copy notes', exact: true }).click();
  await expect.poll(() => copied.at(-1)).toBe('NULL');
  await view.getByRole('button', { name: 'Copy row JSON', exact: true }).click();
  await expect.poll(() => copied.at(-1)).toBe(JSON.stringify({ id: '9007199254740993', name: 'Student May', notes: null }, null, 2));
  await page.evaluate(() => Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: () => Promise.reject(new Error('Denied')) } }));
  await view.getByRole('button', { name: 'Copy id', exact: true }).click();
  await expect(view.getByRole('alert')).toContainText('Could not copy');
  await expect(view.getByText('9007199254740993', { exact: true })).toBeVisible();
  await page.evaluate(() => {
    const bridge = window as unknown as { recordRowCopy: (value: string) => Promise<void> };
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: bridge.recordRowCopy } });
  });
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'View row 2', exact: true }).click();
  view = page.getByRole('dialog', { name: 'View row', exact: true });
  await expect(view.getByText('empty', { exact: true })).toBeVisible();
  await view.getByRole('button', { name: 'Copy notes', exact: true }).click();
  await expect.poll(() => copied.at(-1)).toBe('');
  await view.getByRole('button', { name: 'Close', exact: true }).click();

  const fullText = 'A multiline value\n' + 'unbroken-long-content'.repeat(160) + '\n<script>plain text</script>\n';
  await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
  const edit = page.getByRole('dialog', { name: 'Edit row', exact: true });
  await edit.getByLabel('notes value', { exact: true }).fill(fullText);
  await edit.getByRole('button', { name: 'Save', exact: true }).click();
  await page.getByRole('button', { name: 'View row 1', exact: true }).click();
  view = page.getByRole('dialog', { name: 'View row', exact: true });
  expect(await view.locator('dd').nth(2).locator('pre').textContent()).toBe(fullText);
  await view.getByRole('button', { name: 'Copy notes', exact: true }).click();
  await expect.poll(() => copied.at(-1)).toBe(fullText);
  await expect(view.locator('script')).toHaveCount(0);
  expect(await view.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
  await view.getByRole('button', { name: 'Edit', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Edit row', exact: true }).getByLabel('notes value', { exact: true })).toHaveValue(fullText);
});

test('create database/table, insert default and NULL, edit empty text, delete with confirmation', async ({ page }, info) => {
  await open(page);
  await page.getByRole('button', { name: 'Database', exact: true }).click();
  let dialog = page.getByRole('dialog', { name: 'Create database', exact: true });
  await dialog.getByLabel('Database name', { exact: true }).fill('classroom');
  await dialog.getByRole('button', { name: 'Create', exact: true }).click();
  await expect(page.getByLabel('Database', { exact: true })).toHaveValue('classroom');
  await page.getByRole('button', { name: 'Create table', exact: true }).first().click();
  dialog = page.getByRole('dialog', { name: 'Create table', exact: true });
  await dialog.getByLabel('Table name', { exact: true }).fill('students');
  const nameColumn = dialog.locator('.db-column-card').nth(1);
  await expect(nameColumn.getByLabel('Allow NULL', { exact: true })).not.toBeChecked();
  await nameColumn.getByLabel('Allow NULL', { exact: true }).check();
  await capture(page, info, 'create-table');
  await dialog.getByRole('button', { name: 'Create', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole('button', { name: 'Insert row', exact: true }).click();
  dialog = page.getByRole('dialog', { name: 'Insert row', exact: true });
  await expect(dialog.getByLabel('id value mode', { exact: true })).toHaveValue('default');
  await expect(dialog.getByLabel('name value mode', { exact: true })).toHaveValue('null');
  await expect(dialog.getByLabel('name value', { exact: true })).toBeEnabled();
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Table rows' })).toContainText('NULL');
  await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
  dialog = page.getByRole('dialog', { name: 'Edit row', exact: true });
  await expect(dialog.getByLabel('name value mode', { exact: true })).toHaveValue('null');
  await expect(dialog.getByLabel('name value', { exact: true })).toBeEnabled();
  await dialog.getByLabel('name value', { exact: true }).fill('Student May');
  await expect(dialog.getByLabel('name value mode', { exact: true })).toHaveValue('value');
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Table rows' })).toContainText('Student May');
  await capture(page, info, 'created-data');
  await page.getByRole('button', { name: 'Delete row 1', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Delete', exact: true }).click();
  await expect(page.getByText('0–0 of 0', { exact: true })).toBeVisible();
  const empty = page.getByText('No rows yet. Insert a row to get started.', { exact: true });
  await expect(empty).toBeVisible();
  const inset = await empty.evaluate(element => {
    const range = document.createRange(); range.selectNodeContents(element);
    const text = range.getBoundingClientRect(), panel = element.closest('[role=region]')!.getBoundingClientRect();
    return { left: text.left - panel.left, bottom: panel.bottom - text.bottom };
  });
  expect(inset.left).toBeGreaterThanOrEqual(20); expect(inset.bottom).toBeGreaterThanOrEqual(20);
  await capture(page, info, 'data-browser-empty');
  await action(page, 'Table', 'Delete table');
  dialog = page.getByRole('dialog', { name: 'Delete table', exact: true });
  await expect(dialog.getByRole('button', { name: 'Delete', exact: true })).toBeDisabled();
  await dialog.getByLabel('Type students to confirm', { exact: true }).fill('students');
  await dialog.getByRole('button', { name: 'Delete', exact: true }).click();
  await expect(page.getByText('No tables yet.', { exact: true })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Database workspace', exact: true }).getByRole('button', { name: 'Create table', exact: true })).toBeVisible();
  await action(page, 'Database', 'Delete database');
  dialog = page.getByRole('dialog', { name: 'Delete database', exact: true });
  await dialog.getByLabel('Type classroom to confirm', { exact: true }).fill('classroom');
  await dialog.getByRole('button', { name: 'Delete', exact: true }).click();
  await expect(page.getByLabel('Database', { exact: true })).toHaveValue('fairway');
});

test('insert offers defaults only where available and saves untouched value and NULL fields', async ({ page }, info) => {
  await open(page);
  await page.getByRole('button', { name: 'Create table', exact: true }).first().click();
  let dialog = page.getByRole('dialog', { name: 'Create table', exact: true });
  await dialog.getByLabel('Table name', { exact: true }).fill('insert_modes');
  const nameColumn = dialog.locator('.db-column-card').nth(1);
  await expect(nameColumn.getByLabel('Allow NULL', { exact: true })).not.toBeChecked();
  await nameColumn.getByLabel('Allow NULL', { exact: true }).uncheck();
  for (const definition of [
    { name: 'notes', type: 'VARCHAR', mode: 'none' },
    { name: 'empty_default', type: 'VARCHAR', mode: 'value', value: '' },
    { name: 'zero_default', type: 'INT', mode: 'value', value: '0' },
    { name: 'created_at', type: 'TIMESTAMP', mode: 'current_timestamp' },
  ]) {
    await dialog.getByRole('button', { name: 'Add column', exact: true }).click();
    const card = dialog.locator('.db-column-card').last();
    await card.getByLabel('Column name', { exact: true }).fill(definition.name);
    if (definition.name === 'notes') await card.getByLabel('Allow NULL', { exact: true }).check();
    await card.getByRole('combobox', { name: 'Type', exact: true }).selectOption(definition.type);
    await card.getByRole('combobox', { name: 'Default', exact: true }).selectOption(definition.mode);
    if (definition.value !== undefined) await card.getByLabel('Default value', { exact: true }).fill(definition.value);
  }
  await dialog.getByRole('button', { name: 'Create', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await page.getByRole('button', { name: 'Insert row', exact: true }).click();
  dialog = page.getByRole('dialog', { name: 'Insert row', exact: true });
  await expect(dialog.getByLabel('id value mode', { exact: true })).toHaveValue('default');
  for (const name of ['empty_default', 'zero_default', 'created_at']) {
    const mode = dialog.getByLabel(`${name} value mode`, { exact: true });
    await expect(mode).toHaveValue('default');
    await expect(mode.locator('option[value=default]')).toHaveText('Use default');
  }
  await expect(dialog.getByLabel('name value mode', { exact: true })).toHaveValue('value');
  await expect(dialog.getByLabel('name value', { exact: true })).toBeEnabled();
  await expect(dialog.getByLabel('notes value mode', { exact: true })).toHaveValue('null');
  const notes = dialog.getByLabel('notes value', { exact: true });
  await expect(notes).toBeEnabled();
  await notes.fill('draft');
  await expect(dialog.getByLabel('notes value mode', { exact: true })).toHaveValue('value');
  await notes.fill('');
  await expect(dialog.getByLabel('notes value mode', { exact: true })).toHaveValue('null');
  await dialog.getByLabel('notes value mode', { exact: true }).selectOption('value');
  await expect(dialog.getByLabel('notes value mode', { exact: true })).toHaveValue('value');
  await expect(notes).toHaveValue('');
  await dialog.getByLabel('notes value mode', { exact: true }).selectOption('null');
  for (const name of ['name', 'notes']) await expect(dialog.getByLabel(`${name} value mode`, { exact: true }).locator('option[value=default]')).toHaveCount(0);
  await capture(page, info, 'insert-value-modes');
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  const cells = page.getByRole('region', { name: 'Table rows', exact: true }).locator('tbody tr').first().locator('td');
  await expect(cells).toHaveText(['', '100', 'empty', 'NULL', 'empty', '0', 'CURRENT_TIMESTAMP']);
  await expect(cells.first().getByRole('button', { name: 'View row 1', exact: true })).toBeVisible();
  await expect(cells.first().getByRole('button', { name: 'Edit row 1', exact: true })).toBeVisible();
  await expect(cells.first().getByRole('button', { name: 'Delete row 1', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
  dialog = page.getByRole('dialog', { name: 'Edit row', exact: true });
  await expect(dialog.locator('option[value=default]')).toHaveCount(0);
});

test('schema editing, SQL inspection and populated database rename limitation', async ({ page }, info) => {
  await open(page);
  await page.getByRole('tab', { name: 'Structure', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Column', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Column', exact: true }).click();
  let dialog = page.getByRole('dialog', { name: 'Add column', exact: true });
  await dialog.getByLabel('Column name', { exact: true }).fill('nickname');
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit column nickname', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Edit column nickname', exact: true }).click();
  dialog = page.getByRole('dialog', { name: 'Edit column', exact: true });
  await dialog.getByLabel('Column name', { exact: true }).fill('alias');
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Delete column alias', exact: true })).toBeEnabled();
  await capture(page, info, 'data-browser-structure');
  await action(page, 'Database', 'Rename database');
  dialog = page.getByRole('dialog', { name: 'Rename database', exact: true });
  await dialog.getByLabel('Database name', { exact: true }).fill('renamed');
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('Only empty databases can be renamed');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

test('SQL results, errors, session history and cancellation stay reviewable', async ({ page }, info) => {
  await open(page);
  await page.getByRole('tab', { name: 'SQL query', exact: true }).click();
  expect((await page.locator('label[for=db-sql]').boundingBox())!.height).toBeLessThanOrEqual(20);
  await expect(page.getByRole('tooltip')).toHaveCount(0);
  await page.getByRole('button', { name: 'SQL query help', exact: true }).click();
  await expect(page.getByRole('tooltip')).toContainText('Cancel cannot undo completed statements');
  await expect(page.getByRole('tooltip')).toContainText('200 rows per statement');
  await capture(page, info, 'data-browser-query-help');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('tooltip')).toHaveCount(0);
  const editor = page.locator('#db-sql');
  await editor.fill('SELECT * FROM students');
  await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Query result 1', exact: true })).toContainText('9007199254740993');
  const resultRows = page.getByRole('region', { name: 'Query result 1', exact: true });
  const dimensions = await resultRows.evaluate(element => ({ height: element.clientHeight, content: element.scrollHeight }));
  expect(dimensions.height).toBeLessThanOrEqual(360);
  expect(dimensions.content).toBeGreaterThan(dimensions.height);
  await resultRows.hover(); await page.mouse.wheel(0, 350);
  await expect.poll(() => resultRows.evaluate(element => element.scrollTop)).toBeGreaterThan(0);
  await expect(page.getByLabel('Query history', { exact: true })).toBeEnabled();
  await capture(page, info, 'data-browser-query');
  await editor.fill('INVALID SQL');
  await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('SQL error');
  await editor.fill('SELECT SLEEP(10)');
  await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
  await page.getByRole('button', { name: 'Cancel query', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('canceled');
  await expect(page.getByRole('button', { name: 'Run selection / all', exact: true })).toBeEnabled();
  await capture(page, info, 'data-browser-canceled');
});

test('database messages stay on screen without moving the page or focus', async ({ page }) => {
  await open(page);
  await page.getByRole('tab', { name: 'SQL query', exact: true }).click();
  const editor = page.locator('#db-sql');
  const runSql = page.getByRole('button', { name: 'Run selection / all', exact: true });
  await editor.fill(`LONG ${'Database message detail. '.repeat(80)}`);
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await runSql.focus();
  const position = await page.evaluate(() => window.scrollY);
  expect(position).toBeGreaterThan(100);
  await runSql.press('Enter');
  const alert = page.getByRole('alert');
  await expect(alert).toContainText('Database message detail.');
  await expect(page.getByRole('button', { name: 'Dismiss database message', exact: true })).toBeInViewport({ ratio: 1 });
  expect(await page.locator('.db-notice').evaluate(element => element.getBoundingClientRect().height)).toBeLessThan(270);
  const message = page.locator('.db-notice-message');
  const dimensions = await message.evaluate(element => ({ height: element.clientHeight, content: element.scrollHeight }));
  expect(dimensions.content).toBeGreaterThan(dimensions.height);
  const samples = await page.evaluate(async () => {
    const positions: number[] = [];
    for (let i = 0; i < 45; i++) {
      await new Promise(requestAnimationFrame);
      positions.push(window.scrollY);
    }
    return positions;
  });
  expect(samples.every(value => Math.abs(value - position) <= 1)).toBe(true);
  await expect(runSql).toBeFocused();
});


test('table tabs stay stable and actions are tucked into keyboard-accessible menus', async ({ page }, info) => {
  await open(page);
  await expect(page.getByRole('button', { name: 'Delete table', exact: true })).toHaveCount(0);
  const rows = page.getByRole('tab', { name: 'Rows', exact: true });
  // Watch the visible control for the dimming that used to occur on each tab switch.
  await rows.evaluate(element => {
    element.setAttribute('data-switch-disabled', 'false');
    const observer = new MutationObserver(() => element.setAttribute('data-switch-disabled', 'true'));
    observer.observe(element, { attributes: true, attributeFilter: ['disabled'] });
    element.addEventListener('stop-tab-observation', () => observer.disconnect(), { once: true });
  });
  await page.getByRole('tab', { name: 'Structure', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Table structure', exact: true })).toBeVisible();
  await expect(rows).toHaveAttribute('data-switch-disabled', 'false');
  await rows.evaluate(element => element.dispatchEvent(new Event('stop-tab-observation')));
  await rows.click();
  const trigger = page.getByRole('button', { name: 'Table actions', exact: true });
  await trigger.press('ArrowDown');
  await expect(page.getByRole('menuitem', { name: 'Export table SQL', exact: true })).toBeFocused();
  await page.keyboard.press('End');
  await expect(page.getByRole('menuitem', { name: 'Delete table', exact: true })).toBeFocused();
  await capture(page, info, 'data-browser-actions');
  await page.keyboard.press('Escape');
  await expect(trigger).toBeFocused();
  await expect(page.getByRole('menu')).toHaveCount(0);
  await action(page, 'Database', 'Export database SQL');
  await expect(page.getByRole('status').filter({ hasText: 'SQL export saved:' })).toBeVisible();
});

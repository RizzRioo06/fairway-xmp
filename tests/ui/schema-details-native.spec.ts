import { expect, test } from '@playwright/test';
import { readdir } from 'node:fs/promises';
import { join } from 'node:path';
import { nativeBridge } from './native-bridge';

test.use({ baseURL: 'http://127.0.0.1:1422' });

for (const engine of ['mysql', 'sqlite'] as const) test(`${engine} native index management and foreign-key inspection`, async ({ page }, info) => {
  test.skip(!process.env.FAIRWAY_TEST_RUNTIME, 'Prepare the runtime and terminal_bridge example.');
  test.setTimeout(120000);
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/');
    await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
    if (engine === 'sqlite') await page.getByRole('button', { name: 'Open SQLite file…', exact: true }).click();
    else {
      await page.getByRole('button', { name: 'Start MySQL', exact: true }).click();
      await page.getByRole('button', { name: 'Database', exact: true }).click({ timeout: 60000 });
      const dialog = page.getByRole('dialog', { name: 'Create database', exact: true });
      await dialog.getByLabel('Database name', { exact: true }).fill('schema_browser');
      await dialog.getByRole('button', { name: 'Create', exact: true }).click();
      await expect(dialog).toHaveCount(0);
    }
    await page.getByRole('tab', { name: 'SQL query', exact: true }).click();
    await page.locator('#db-sql').fill(`CREATE TABLE teams(id INTEGER NOT NULL PRIMARY KEY, code VARCHAR(40) NOT NULL);
      CREATE TABLE memberships(id INTEGER NOT NULL PRIMARY KEY, team_id INTEGER NOT NULL, email VARCHAR(80) NOT NULL, term VARCHAR(20) NOT NULL, CONSTRAINT fk_memberships_team FOREIGN KEY(team_id) REFERENCES teams(id) ON UPDATE RESTRICT ON DELETE CASCADE);
      INSERT INTO teams VALUES(1,'class');
      INSERT INTO memberships VALUES(1,1,'same','a'),(2,1,'same','b'),(3,1,'third','b');`);
    await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
    await expect(page.getByLabel('Query results', { exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'memberships', exact: true }).click();
    await page.getByRole('tab', { name: 'Structure', exact: true }).click();
    const links = page.getByRole('region', { name: 'Foreign keys', exact: true });
    await expect(links).toContainText('memberships.team_id → teams.id');
    await expect(links).toContainText('On update: RESTRICT · On delete: CASCADE');
    await page.getByRole('button', { name: 'Index', exact: true }).click();
    let dialog = page.getByRole('dialog', { name: 'Create index', exact: true });
    await dialog.getByLabel('Index column 1', { exact: true }).selectOption('email');
    await dialog.getByLabel('Require unique values', { exact: true }).check();
    await dialog.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(dialog.getByRole('alert')).toContainText('existing values contain duplicates');
    await dialog.getByRole('button', { name: 'Add index column', exact: true }).click();
    await dialog.getByLabel('Index column 2', { exact: true }).selectOption('term');
    await dialog.getByRole('button', { name: 'Move index column 2 up', exact: true }).click();
    await dialog.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    const indexes = page.getByRole('region', { name: 'Indexes', exact: true });
    const unique = indexes.locator('li').filter({ has: page.getByText('uq_memberships_term_email', { exact: true }) });
    await expect(unique.locator('.db-index-parts code')).toHaveText(['term', 'email']);
    await expect(unique).toContainText('Unique');
    await page.getByRole('button', { name: 'Index', exact: true }).click();
    dialog = page.getByRole('dialog', { name: 'Create index', exact: true });
    await dialog.getByLabel('Index column 1', { exact: true }).selectOption('term');
    await dialog.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await indexes.getByRole('button', { name: 'Remove index idx_memberships_term', exact: true }).click();
    dialog = page.getByRole('dialog', { name: 'Remove index', exact: true });
    await dialog.getByLabel('Type idx_memberships_term to confirm', { exact: true }).fill('idx_memberships_term');
    await dialog.getByRole('button', { name: 'Remove', exact: true }).click();
    await expect(indexes.getByText('idx_memberships_term', { exact: true })).toHaveCount(0);
    const backups = await readdir(join(bridge.root, 'data 日本/backups'));
    expect(backups.some(name => name.endsWith(engine === 'sqlite' ? '.sqlite' : '.sql'))).toBe(true);
    await expect(indexes.getByRole('button', { name: /^Remove index PRIMARY/ })).toHaveCount(0);
    await page.getByRole('button', { name: 'teams', exact: true }).click();
    await expect(links).toContainText('Referenced by');
    await expect(links).toContainText('memberships.team_id → teams.id');
    await expect(links.getByRole('button')).toHaveCount(0);
    const screenshot = info.outputPath(`${engine}-schema-details.png`);
    await page.screenshot({ path: screenshot, fullPage: true });
    await info.attach(`${engine}-schema-details`, { path: screenshot, contentType: 'image/png' });
    expect(bridge.errors()).toBe('');
  } finally { await bridge.close(); }
});

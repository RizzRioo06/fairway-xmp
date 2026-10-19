"""Exercise the shipped SQLite worker against real project files and external writers."""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import unittest
from contextlib import closing

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = Path(os.environ.get('FAIRWAY_TEST_RUNTIME', ROOT / 'src-tauri/resources/runtime'))


class SQLiteBrowserTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.php = RUNTIME / 'php/bin' / ('php.exe' if os.name == 'nt' else 'php')
        if not cls.php.is_file():
            raise unittest.SkipTest('Prepare the bundled runtime or set FAIRWAY_TEST_RUNTIME.')

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='fairway-sqlite-')
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "Student May's 日本.sqlite"
        self.path.touch()

    def call(self, op, **args):
        request = dict(op=op, path=str(self.path), **args)
        result = subprocess.run(
            self.php_command(),
            input=json.dumps(dict(request=request)), text=True, capture_output=True,
            env={**os.environ, 'MIBS': '', 'PHP_INI_SCAN_DIR': ''}, timeout=35,
        )
        self.assertEqual(result.stderr, '')
        data = json.loads(result.stdout)
        if 'error' in data:
            raise RuntimeError(data['error'])
        return data['result']

    def php_command(self):
        command = [str(self.php), '-n', '-d', 'display_errors=stderr', '-d', 'memory_limit=128M']
        if os.name == 'nt':
            command += ['-d', 'extension_dir=' + (self.php.parent.parent / 'ext').as_posix(), '-d', 'extension=pdo_sqlite']
        return command + ['-f', str(ROOT / 'src-tauri/src/data_browser_sqlite.php')]

    def sql(self, sql):
        return self.call('query', sql=sql)

    def test_original_file_crud_exact_values_and_external_conflicts(self):
        self.assertEqual(self.call('tables')['tables'], [])
        self.call('create_table', table='students', columns=[
            dict(name='id', type='INTEGER', primary=True, autoIncrement=True),
            dict(name='name', type='TEXT', nullable=True),
            dict(name='notes', type='TEXT', nullable=True),
        ])
        self.call('insert', table='students', values=dict(id='9007199254740993', name="May's 日本", notes=None))
        page = self.call('rows', table='students')
        self.assertEqual(page['key'], ['id'])
        original = page['rows'][0]
        self.assertEqual(original, dict(id='9007199254740993', name="May's 日本", notes=None))
        self.call('update', table='students', original=original, values=dict(notes=''))
        with self.assertRaisesRegex(RuntimeError, 'changed or was deleted'):
            self.call('delete', table='students', original=original)
        with closing(sqlite3.connect(self.path)) as app:
            self.assertEqual(app.execute('SELECT notes FROM students').fetchone(), ('',))
            app.execute("UPDATE students SET name='Changed by project'")
            app.commit()
        with self.assertRaisesRegex(RuntimeError, 'changed or was deleted'):
            self.call('update', table='students', original=original, values=dict(name='stale'))
        current = self.call('rows', table='students')['rows'][0]
        self.call('delete', table='students', original=current)
        self.assertEqual(self.call('rows', table='students')['total'], '0')

    def test_sql_scripts_triggers_transactions_and_returning_execute_once(self):
        result = self.sql("""
            CREATE TABLE students(id INTEGER PRIMARY KEY, name TEXT);
            CREATE TABLE audit(value TEXT);
            CREATE TRIGGER log AFTER INSERT ON students BEGIN
              INSERT INTO audit VALUES(CASE WHEN new.name='a;b' THEN 'yes;ok' ELSE 'no' END);
              INSERT INTO audit VALUES('second');
            END;
            -- a semicolon ; in a comment
            INSERT INTO students(name) VALUES('a;b') RETURNING id;
            SELECT * FROM audit;
        """)
        self.assertEqual(len(result['sets']), 5)
        self.assertEqual(result['sets'][3]['rows'], [['1']])
        self.assertEqual(result['sets'][4]['rows'], [['yes;ok'], ['second']])
        self.assertEqual(self.sql('SELECT count(*) FROM students')['sets'][0]['rows'], [['1']])
        with self.assertRaisesRegex(RuntimeError, 'Finish the transaction'):
            self.sql("BEGIN; INSERT INTO students(name) VALUES('rolled back')")
        self.assertEqual(self.sql('SELECT count(*) FROM students')['sets'][0]['rows'], [['1']])
        self.sql("BEGIN; INSERT INTO students(name) VALUES('committed'); COMMIT;")
        with self.assertRaisesRegex(RuntimeError, 'at most ten'):
            self.sql("INSERT INTO audit VALUES('must not run');" + 'SELECT 1;' * 10)
        self.assertEqual(self.sql('SELECT count(*) FROM audit')['sets'][0]['rows'], [['4']])

    def test_keys_binary_and_views_are_conservative(self):
        self.sql("""CREATE TABLE nullable_key(id TEXT PRIMARY KEY, value TEXT);
            CREATE TABLE keyed(a TEXT NOT NULL, b TEXT NOT NULL, payload TEXT, UNIQUE(a,b));
            INSERT INTO keyed VALUES('A','B',X'006162');
            CREATE VIEW names AS SELECT * FROM keyed;
            CREATE TABLE descending(id INTEGER PRIMARY KEY DESC, value TEXT);
        """)
        self.assertEqual(self.call('rows', table='nullable_key')['key'], [])
        self.assertEqual(self.call('rows', table='descending')['key'], [])
        self.assertEqual(self.call('rows', table='names')['key'], [])
        page = self.call('rows', table='keyed')
        self.assertEqual(page['key'], ['a', 'b'])
        self.assertTrue(page['columns'][2]['binary'])
        self.assertEqual(page['rows'][0]['payload'], '0x006162')
        self.call('update', table='keyed', original=page['rows'][0], binaryColumns=['payload'], values=dict(a='C'))
        with self.assertRaisesRegex(RuntimeError, 'read-only'):
            self.call('insert', table='names', values=dict(a='X'))

    def test_locked_files_limits_and_missing_paths(self):
        self.sql('CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)')
        with closing(sqlite3.connect(self.path)) as app:
            app.execute('BEGIN IMMEDIATE')
            with self.assertRaisesRegex(RuntimeError, 'locked'):
                self.call('insert', table='t', values=dict(name='blocked'))
        self.assertEqual(self.call('rows', table='t')['total'], '0')
        result = self.sql('WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<250) SELECT x FROM n')
        self.assertEqual(len(result['sets'][0]['rows']), 200)
        self.assertTrue(result['sets'][0]['truncated'])
        self.path.unlink()
        with self.assertRaisesRegex(RuntimeError, 'unable to open'):
            self.call('tables')
        self.assertFalse(self.path.exists())

    def test_wal_backup_contains_committed_data_without_copying_sidecars(self):
        with closing(sqlite3.connect(self.path)) as app:
            app.execute('PRAGMA journal_mode=WAL')
            app.execute('CREATE TABLE t(id INTEGER PRIMARY KEY, value TEXT)')
            app.execute("INSERT INTO t VALUES(1, 'Committed in WAL')")
            app.commit()
            self.assertTrue(Path(str(self.path) + '-wal').is_file())
            destination = Path(self.temp.name) / 'backup.sqlite'
            destination.touch()  # The native backend reserves the unique filename.
            result = subprocess.run(
                self.php_command(),
                input=json.dumps(dict(request=dict(op='backup', path=str(self.path)), backupPath=str(destination))),
                text=True, capture_output=True, env={**os.environ, 'MIBS': '', 'PHP_INI_SCAN_DIR': ''}, timeout=35,
            )
            self.assertEqual(result.stderr, '')
            self.assertIn('result', json.loads(result.stdout))
            with closing(sqlite3.connect(destination)) as backup:
                self.assertEqual(backup.execute('SELECT value FROM t').fetchone(), ('Committed in WAL',))
            app.execute("INSERT INTO t VALUES(2, 'Uncommitted')")
            with closing(sqlite3.connect(destination)) as backup:
                self.assertEqual(backup.execute('SELECT count(*) FROM t').fetchone(), (1,))

    def test_real_values_and_virtual_table_auxiliaries_are_preserved(self):
        self.sql('CREATE TABLE numbers(id INTEGER PRIMARY KEY, amount REAL, name TEXT); INSERT INTO numbers VALUES(1,1.2345678901234567,\'May\'); CREATE VIRTUAL TABLE search USING fts5(value);')
        page = self.call('rows', table='numbers')
        self.call('update', table='numbers', original=page['rows'][0], values=dict(name='Changed'))
        with closing(sqlite3.connect(self.path)) as app:
            self.assertEqual(app.execute('SELECT amount FROM numbers').fetchone(), (1.2345678901234567,))
        names = [item['name'] for item in self.call('tables')['tables']]
        self.assertIn('search', names)
        self.assertNotIn('search_data', names)
        with self.assertRaisesRegex(RuntimeError, 'read-only'):
            self.call('insert', table='search_data', values={})

    def test_all_bundled_php_versions_open_sqlite(self):
        current = self.php
        for name in ['php', 'php-8.0', 'php-8.1', 'php-8.2', 'php-8.3', 'php-8.4']:
            candidate = RUNTIME / name / 'bin' / ('php.exe' if os.name == 'nt' else 'php')
            if candidate.is_file():
                with self.subTest(runtime=name):
                    self.php = candidate
                    self.assertEqual(self.call('tables')['tables'], [])
        self.php = current

    def test_index_creation_composite_uniqueness_and_stale_removal(self):
        self.sql("CREATE TABLE t(a TEXT NOT NULL,b TEXT NOT NULL); INSERT INTO t VALUES('one','a'),('one','b'),('two','b');")
        with self.assertRaisesRegex(RuntimeError, 'existing values contain duplicates'):
            self.call('create_index', table='t', name='uq_a', indexColumns=['a'], unique=True)
        self.assertEqual(self.call('structure', table='t')['indexes'], [])
        quoted = 'unique "日本 index'
        self.call('create_index', table='t', name=quoted, indexColumns=['b', 'a'], unique=True)
        structure = self.call('structure', table='t')
        index = structure['indexes'][0]
        self.assertEqual([part['name'] for part in index['parts']], ['b', 'a'])
        self.assertTrue(index['unique'])
        self.assertEqual(structure['key'], ['b', 'a'])
        self.sql('INSERT INTO t VALUES(\'three\',\'c\')')
        with self.assertRaisesRegex(RuntimeError, 'UNIQUE constraint failed'):
            self.sql("INSERT INTO t VALUES('one','a')")
        with self.assertRaisesRegex(RuntimeError, 'Confirm'):
            self.call('drop_index', table='t', name=quoted, originalIndex=index)
        self.call('drop_index', table='t', name=quoted, originalIndex=index, confirmed=True)
        self.assertEqual(self.call('structure', table='t')['indexes'], [])
        self.call('create_index', table='t', name='idx_a', indexColumns=['a'], unique=False)
        old = self.call('structure', table='t')['indexes'][0]
        self.sql('DROP INDEX idx_a; CREATE INDEX idx_a ON t(b)')
        with self.assertRaisesRegex(RuntimeError, 'changed or was removed'):
            self.call('drop_index', table='t', name='idx_a', originalIndex=old, confirmed=True)
        self.assertEqual(self.call('structure', table='t')['indexes'][0]['parts'][0]['name'], 'b')
        for parts in [['a', 'a'], ['missing'], []]:
            with self.assertRaises(RuntimeError):
                self.call('create_index', table='t', name='invalid', indexColumns=parts)

    def test_foreign_keys_composites_implicit_parent_keys_and_protected_indexes(self):
        self.sql("""CREATE TABLE parent(a TEXT NOT NULL,b TEXT NOT NULL, PRIMARY KEY(a,b));
            CREATE TABLE child(x TEXT,y TEXT, FOREIGN KEY(x,y) REFERENCES parent ON UPDATE CASCADE ON DELETE SET NULL);
            CREATE TABLE external_key(code TEXT NOT NULL, other TEXT UNIQUE);
            CREATE UNIQUE INDEX uq_code ON external_key(code);
            CREATE TABLE dependent(code TEXT REFERENCES external_key(code));
            CREATE INDEX partial ON external_key(other) WHERE other IS NOT NULL;
            CREATE INDEX expression ON external_key(lower(other));
        """)
        child = self.call('structure', table='child')
        fk = child['foreignKeys'][0]
        self.assertEqual(fk['columns'], ['x', 'y'])
        self.assertEqual(fk['referencedColumns'], ['a', 'b'])
        self.assertEqual((fk['onUpdate'], fk['onDelete']), ('CASCADE', 'SET NULL'))
        self.assertEqual(self.call('structure', table='parent')['referencedBy'][0], fk)
        parent = self.call('structure', table='external_key')
        self.assertEqual(len(parent['referencedBy']), 1)
        for index in parent['indexes']:
            self.assertFalse(index['removable'])
            with self.assertRaisesRegex(RuntimeError, 'manage it in SQL'):
                self.call('drop_index', table='external_key', name=index['name'], originalIndex=index, confirmed=True)
        primary = self.call('structure', table='parent')['indexes'][0]
        self.assertTrue(primary['primary'])
        self.assertFalse(primary['removable'])
        self.sql('CREATE TABLE rowid_table(id INTEGER PRIMARY KEY); CREATE VIEW rowid_view AS SELECT * FROM rowid_table')
        self.assertEqual(self.call('structure', table='rowid_table')['indexes'][0]['type'], 'ROWID')
        self.assertEqual(self.call('structure', table='rowid_view')['indexes'], [])


if __name__ == '__main__':
    unittest.main()

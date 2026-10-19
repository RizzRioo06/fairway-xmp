<?php
// Private native worker. The original project file is opened without CREATE.
function ident($name) {
    if (!is_string($name) || $name === '' || strlen($name) > 256 || preg_match('/[\x00-\x1f\x7f]/', $name)) throw new Exception('Use a name without control characters (up to 256 bytes).');
    return '"' . str_replace('"', '""', $name) . '"';
}
function rows($db, $sql, $args = []) {
    $s = $db->prepare($sql); $s->execute($args); return $s->fetchAll(PDO::FETCH_ASSOC);
}
function table_info($db, $table) {
    $info = rows($db, "SELECT type, sql FROM sqlite_master WHERE name=? AND type IN ('table','view')", [$table]);
    if (!$info) throw new Exception('This table no longer exists. Refresh the database.');
    $kind = $info[0]['type'] === 'view' ? 'VIEW' : (preg_match('/^CREATE\s+VIRTUAL\s+TABLE/i', $info[0]['sql']) || str_starts_with(strtolower($table), 'sqlite_') ? 'READ ONLY' : 'BASE TABLE');
    $listed = rows($db, 'PRAGMA table_list(' . ident($table) . ')');
    foreach ($listed as $item) if ($item['schema'] === 'main' && $item['type'] === 'shadow') $kind = 'READ ONLY';
    // Older bundled SQLite versions lack table_list; protect virtual-table auxiliaries conservatively.
    if (!$listed && str_contains($table, '_')) foreach (rows($db, "SELECT name, sql FROM sqlite_master WHERE type='table'") as $parent) {
        if (preg_match('/^CREATE\s+VIRTUAL\s+TABLE/i', $parent['sql'] ?? '') && str_starts_with($table, $parent['name'] . '_')) $kind = 'READ ONLY';
    }
    return [$kind, $info[0]['sql']];
}
function columns($db, $table, $ddl) {
    $raw = rows($db, 'PRAGMA table_xinfo(' . ident($table) . ')');
    $primary = array_filter($raw, fn($c) => (int)$c['pk'] > 0);
    $primaryIndex = array_filter(rows($db, 'PRAGMA index_list(' . ident($table) . ')'), fn($i) => $i['origin'] === 'pk');
    $result = [];
    foreach ($raw as $c) {
        if ((int)$c['hidden'] === 1) continue;
        $auto = !$primaryIndex && count($primary) === 1 && (int)$c['pk'] > 0 && strtoupper($c['type']) === 'INTEGER' && !preg_match('/\bWITHOUT\s+ROWID\b/i', $ddl);
        $result[] = ['name'=>$c['name'], 'type'=>$c['type'], 'nullable'=>!$auto && !(int)$c['notnull'], 'defaultValue'=>$c['dflt_value'], 'extra'=>$auto ? 'auto_increment' : '', 'columnKey'=>(int)$c['pk'] > 0 ? 'PRI' : '', 'expression'=>'', 'binary'=>stripos($c['type'], 'BLOB') !== false, 'generated'=>(int)$c['hidden'] > 1];
    }
    return $result;
}
function stable_key($db, $table, $columns, $kind) {
    if ($kind !== 'BASE TABLE') return [];
    $map = array_column($columns, null, 'name');
    $candidates = [];
    // Only a real rowid alias can bypass SQLite's nullable-primary-key behavior.
    foreach ($columns as $c) if ($c['extra'] === 'auto_increment') $candidates[] = [$c['name']];
    foreach (rows($db, 'PRAGMA index_list(' . ident($table) . ')') as $index) {
        if (!(int)$index['unique'] || (int)$index['partial']) continue;
        $parts = rows($db, 'PRAGMA index_info(' . ident($index['name']) . ')');
        $candidates[] = array_column($parts, 'name');
    }
    foreach ($candidates as $key) {
        if (!$key) continue;
        foreach ($key as $name) if (!isset($map[$name]) || $map[$name]['nullable'] || $map[$name]['binary'] || $map[$name]['generated']) continue 2;
        return $key;
    }
    return [];
}
function foreign_keys($db, $table) {
    $out = []; $incoming = [];
    foreach (rows($db, "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite\_%' ESCAPE '\' ORDER BY name") as $child) {
        $groups = [];
        foreach (rows($db, 'PRAGMA foreign_key_list(' . ident($child['name']) . ')') as $part) {
            $id = $part['id'];
            if (!isset($groups[$id])) $groups[$id] = ['name'=>'Foreign key ' . ((int)$id+1),'database'=>null,'table'=>$child['name'],'columns'=>[],'referencedDatabase'=>null,'referencedTable'=>$part['table'],'referencedColumns'=>[],'onUpdate'=>$part['on_update'],'onDelete'=>$part['on_delete']];
            $groups[$id]['columns'][(int)$part['seq']] = $part['from']; $groups[$id]['referencedColumns'][(int)$part['seq']] = $part['to'];
        }
        foreach ($groups as $fk) {
            ksort($fk['columns']); ksort($fk['referencedColumns']);
            $fk['columns'] = array_values($fk['columns']); $fk['referencedColumns'] = array_values($fk['referencedColumns']);
            // REFERENCES parent without a column list means the parent's ordered primary key.
            if (in_array(null,$fk['referencedColumns'],true)) {
                $pk = array_values(array_filter(rows($db,'PRAGMA table_xinfo(' . ident($fk['referencedTable']) . ')'),fn($c)=>(int)$c['pk']>0));
                usort($pk,fn($a,$b)=>(int)$a['pk']<=>(int)$b['pk']);
                foreach ($fk['referencedColumns'] as $i=>&$name) if ($name===null) $name=$pk[$i]['name']??null;
                unset($name);
            }
            if (strcasecmp($child['name'],$table)===0) $out[] = $fk;
            if (strcasecmp($fk['referencedTable'],$table)===0) $incoming[] = $fk;
        }
    }
    return ['foreignKeys'=>$out,'referencedBy'=>$incoming];
}
function table_indexes($db, $table, $columns, $links) {
    $result = []; $hasPrimary = false;
    foreach (rows($db, 'PRAGMA index_list(' . ident($table) . ')') as $index) {
        $parts = []; $advanced = (bool)(int)$index['partial'];
        foreach (rows($db,'PRAGMA index_xinfo(' . ident($index['name']) . ')') as $part) if ((int)$part['key']) {
            $parts[] = ['name'=>$part['name'],'expression'=>(int)$part['cid']===-2?'Expression (see index SQL)':null,'direction'=>(int)$part['desc']?'DESC':'ASC','prefix'=>null,'collation'=>$part['coll']];
            if ((int)$part['cid']<0 || ($part['coll']??'BINARY')!=='BINARY') $advanced=true;
        }
        $primary = $index['origin']==='pk'; $hasPrimary = $hasPrimary || $primary;
        $reason = $primary ? 'Primary key; manage it in SQL.' : ($index['origin']==='u' ? 'Declared UNIQUE constraint; manage it in SQL.' : ($advanced ? 'Advanced index; manage it in SQL.' : ''));
        $names = array_map(fn($part)=>strtolower($part['name']??''),$parts);
        if ((int)$index['unique']) foreach ($links['referencedBy'] as $fk) if ($names===array_map(fn($name)=>strtolower($name??''),$fk['referencedColumns'])) $reason='Supports a foreign key; manage it in SQL.';
        $sql = rows($db,"SELECT sql FROM sqlite_master WHERE type='index' AND name=?",[$index['name']]);
        $result[] = ['name'=>$index['name'],'unique'=>(bool)(int)$index['unique'],'primary'=>$primary,'type'=>'BTREE','parts'=>$parts,'details'=>implode(' · ',array_filter([(int)$index['partial']?'Partial':'',$index['origin']==='u'?'UNIQUE constraint':''])),'sql'=>$sql[0]['sql']??null,'removable'=>$reason==='','reason'=>$reason];
    }
    // INTEGER PRIMARY KEY aliases rowid and therefore has no separate index_list entry.
    if (!$hasPrimary) foreach ($columns as $c) if ($c['extra']==='auto_increment') {
        array_unshift($result,['name'=>'PRIMARY KEY','unique'=>true,'primary'=>true,'type'=>'ROWID','parts'=>[['name'=>$c['name'],'expression'=>null,'direction'=>'ASC','prefix'=>null,'collation'=>null]],'details'=>'INTEGER PRIMARY KEY (rowid)','sql'=>null,'removable'=>false,'reason'=>'Primary key; manage it in SQL.']); break;
    }
    return $result;
}
function create_index($db, $table, $r, $map) {
    $name = $r['name'] ?? ''; $qn = ident($name);
    if (str_starts_with(strtolower($name),'sqlite_')) throw new Exception('SQLite reserves index names beginning with sqlite_.');
    $parts = $r['indexColumns'] ?? [];
    if (!is_array($parts) || count($parts)<1 || count($parts)>16) throw new Exception('Choose 1–16 index columns in order.');
    $seen=[]; foreach ($parts as $part) {
        if (!is_string($part) || !isset($map[$part]) || in_array(strtolower($part),$seen,true)) throw new Exception('Choose existing, distinct index columns.');
        if ($map[$part]['generated'] || $map[$part]['binary']) throw new Exception('Use SQL for indexes on binary or generated columns.');
        $seen[]=strtolower($part);
    }
    try { $db->exec('CREATE ' . (($r['unique'] ?? false)===true?'UNIQUE ':'') . "INDEX $qn ON " . ident($table) . ' (' . implode(',',array_map('ident',$parts)) . ')'); }
    catch (PDOException $e) { if (($r['unique'] ?? false)===true && stripos($e->getMessage(),'UNIQUE constraint failed')!==false) throw new Exception('Cannot create this unique index because existing values contain duplicates. No index was created.'); throw $e; }
    return ['message'=>($r['unique'] ?? false)===true?'Unique index created.':'Index created.'];
}
function index_snapshot($value) {
    // Native JSON maps reorder object keys; preserve values and ordered list positions.
    if (is_array($value)) { ksort($value); foreach ($value as &$part) $part=index_snapshot($part); unset($part); }
    return $value;
}
function column_sql($db, $c, $adding = false) {
    $type = strtoupper($c['type'] ?? '');
    if (!in_array($type, ['INTEGER','TEXT','REAL','NUMERIC','BLOB'], true)) throw new Exception('Choose a SQLite column type.');
    if (!empty($c['unsigned'])) throw new Exception('SQLite does not support unsigned columns.');
    if ($adding && (!empty($c['primary']) || !empty($c['autoIncrement']))) throw new Exception('Use SQL to rebuild a table when changing its primary key.');
    $sql = ident($c['name'] ?? '') . ' ' . $type;
    if (!empty($c['autoIncrement'])) {
        if ($type !== 'INTEGER' || empty($c['primary']) || ($c['defaultMode'] ?? 'none') !== 'none') throw new Exception('Automatic IDs require an INTEGER primary key without a default.');
        $sql .= ' PRIMARY KEY AUTOINCREMENT';
    }
    $nullable = !empty($c['nullable']) && empty($c['primary']);
    if (!$nullable) $sql .= ' NOT NULL';
    $mode = $c['defaultMode'] ?? 'none';
    if ($mode === 'null') { if (!$nullable) throw new Exception('A NOT NULL column cannot default to NULL.'); $sql .= ' DEFAULT NULL'; }
    elseif ($mode === 'value') $sql .= ' DEFAULT ' . $db->quote((string)($c['defaultValue'] ?? ''));
    elseif ($mode === 'current_timestamp' && !$adding) $sql .= ' DEFAULT CURRENT_TIMESTAMP';
    elseif ($mode !== 'none') throw new Exception('Use a literal default when adding a SQLite column.');
    return $sql;
}
// Split scripts without splitting quoted text, comments, or trigger bodies.
function statements($sql) {
    $result = []; $start = 0; $quote = ''; $comment = ''; $words = []; $trigger = false; $depth = 0; $meaningful = false;
    $length = strlen($sql);
    for ($i = 0; $i < $length; $i++) {
        $c = $sql[$i]; $next = $sql[$i + 1] ?? '';
        if ($comment === '--') { if ($c === "\n" || $c === "\r") $comment = ''; continue; }
        if ($comment === '/*') { if ($c === '*' && $next === '/') { $comment = ''; $i++; } continue; }
        if ($quote !== '') { if ($c === $quote) { if ($next === $quote && $quote !== ']') $i++; else $quote = ''; } continue; }
        if ($c === '-' && $next === '-') { $comment = '--'; $i++; continue; }
        if ($c === '/' && $next === '*') { $comment = '/*'; $i++; continue; }
        if (in_array($c, ["'", '"', '`', '['], true)) { $quote = $c === '[' ? ']' : $c; $meaningful = true; continue; }
        if (ctype_alpha($c) || $c === '_') {
            $j = $i; while ($i + 1 < $length && (ctype_alnum($sql[$i + 1]) || $sql[$i + 1] === '_')) $i++;
            $word = strtoupper(substr($sql, $j, $i - $j + 1));
            if (count($words) < 4) $words[] = $word;
            if (preg_match('/^CREATE (?:(?:TEMP|TEMPORARY) )?TRIGGER$/', implode(' ', $words))) $trigger = true;
            if ($trigger && in_array($word, ['BEGIN','CASE'], true)) $depth++;
            if ($trigger && $word === 'END') $depth--;
            $meaningful = true; continue;
        }
        if ($c === ';' && (!$trigger || $depth <= 0)) {
            if ($meaningful) $result[] = substr($sql, $start, $i - $start + 1);
            $start = $i + 1; $words = []; $trigger = false; $depth = 0; $meaningful = false;
        } elseif (!ctype_space($c)) $meaningful = true;
    }
    if ($quote !== '' || $comment === '/*') throw new Exception('Close SQL quotes and comments before running.');
    if ($meaningful) $result[] = substr($sql, $start);
    if (count($result) > 10) throw new Exception('Run at most ten statements at a time.');
    return $result;
}
function query($db, $sql) {
    if (!is_string($sql) || trim($sql) === '' || strlen($sql) > 1024 * 1024) throw new Exception('Enter SQL up to 1 MiB.');
    $sets = [];
    try {
        foreach (statements($sql) as $text) {
            $s = $db->query($text); $names = []; $data = []; $truncated = false;
            for ($i = 0; $i < $s->columnCount(); $i++) $names[] = $s->getColumnMeta($i)['name'];
            while ($names && ($row = $s->fetch(PDO::FETCH_NUM)) !== false) {
                if (count($data) >= 200) { $truncated = true; break; }
                foreach ($row as $i => &$value) if ($value !== null && ($s->getColumnMeta($i)['native_type'] === 'blob' || !preg_match('//u', $value))) $value = '0x' . bin2hex($value);
                unset($value); $data[] = $row;
            }
            $sets[] = ['columns'=>$names, 'rows'=>$data, 'affected'=>$names ? 0 : $s->rowCount(), 'truncated'=>$truncated];
            $s->closeCursor();
        }
        // Workers are per request; avoid silently rolling back an unfinished transaction.
        if ($db->inTransaction()) throw new Exception('Finish the transaction with COMMIT or ROLLBACK in the same run.');
    } catch (Throwable $e) {
        if ($db->inTransaction()) $db->rollBack();
        throw new Exception($e->getMessage() . ' Earlier committed statements may have completed. Refresh to check the current data.');
    }
    return ['sets'=>$sets, 'message'=>'Query completed.'];
}
function run_request($db, $r) {
    $op = $r['op'] ?? '';
    if ($op === 'databases') return ['databases'=>[$r['path']]];
    if ($op === 'tables') {
        $tables = rows($db, "SELECT name, sql, type FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite\_%' ESCAPE '\' ORDER BY name");
        $shadow = []; foreach (rows($db, 'PRAGMA table_list') as $item) if ($item['schema'] === 'main' && $item['type'] === 'shadow') $shadow[] = $item['name'];
        $result = []; foreach ($tables as $t) if (!in_array($t['name'], $shadow, true)) $result[] = ['name'=>$t['name'], 'kind'=>$t['type'] === 'view' ? 'VIEW' : (preg_match('/^CREATE\s+VIRTUAL\s+TABLE/i', $t['sql']) ? 'READ ONLY' : 'BASE TABLE')];
        return ['tables'=>$result];
    }
    if ($op === 'query') return query($db, $r['sql'] ?? '');
    $table = $r['table'] ?? ''; $qt = ident($table);
    if (str_starts_with(strtolower($table), 'sqlite_')) throw new Exception('SQLite internal tables are read-only.');
    if ($op === 'create_table') {
        $columns = $r['columns'] ?? []; if (!is_array($columns) || count($columns) < 1 || count($columns) > 100) throw new Exception('Create a table with 1–100 columns.');
        $defs = []; $primary = []; $auto = 0;
        foreach ($columns as $c) { $defs[] = column_sql($db, $c); if (!empty($c['primary'])) $primary[] = ident($c['name']); if (!empty($c['autoIncrement'])) $auto++; }
        if ($auto && ($auto !== 1 || count($primary) !== 1)) throw new Exception('Automatic IDs require a single INTEGER primary key.');
        if ($primary && !$auto) $defs[] = 'PRIMARY KEY (' . implode(',', $primary) . ')';
        $db->exec("CREATE TABLE $qt (" . implode(',', $defs) . ')'); return ['message'=>'Table created.'];
    }
    [$kind, $ddl] = table_info($db, $table);
    $columns = columns($db, $table, $ddl); $map = array_column($columns, null, 'name');
    $key = stable_key($db, $table, $columns, $kind);
    if ($op === 'structure') { $links=foreign_keys($db,$table); return array_merge(['columns'=>$columns,'key'=>$key,'kind'=>$kind,'sql'=>$ddl,'indexes'=>table_indexes($db,$table,$columns,$links)],$links); }
    if ($op === 'rows') {
        $sort = $r['sort'] ?? ($key[0] ?? $columns[0]['name']); if (!isset($map[$sort])) throw new Exception('Choose a valid sort column.');
        $direction = ($r['direction'] ?? '') === 'DESC' ? 'DESC' : 'ASC';
        $offset = max(0, min(10000000, (int)($r['offset'] ?? 0))); $limit = max(1, min(100, (int)($r['limit'] ?? 50)));
        $order = ident($sort) . " $direction"; foreach ($key as $part) if ($part !== $sort) $order .= ',' . ident($part) . " $direction";
        // Fetch storage classes as well: SQLite permits BLOBs even in TEXT columns.
        $select = []; foreach ($columns as $c) { $qc = ident($c['name']); $select[] = "CASE typeof($qc) WHEN 'blob' THEN $qc WHEN 'real' THEN quote($qc) ELSE CAST($qc AS TEXT) END"; $select[] = "typeof($qc)"; }
        $s = $db->query('SELECT ' . implode(',', $select) . " FROM $qt ORDER BY $order LIMIT $limit OFFSET $offset");
        $raw = $s->fetchAll(PDO::FETCH_NUM);
        foreach ($columns as $i => &$c) foreach ($raw as $row) if ($row[$i * 2 + 1] === 'blob' || ($row[$i * 2] !== null && !preg_match('//u', $row[$i * 2]))) $c['binary'] = true;
        unset($c);
        $data = []; foreach ($raw as $row) { $item = []; foreach ($columns as $i => $c) { $v = $row[$i * 2]; $item[$c['name']] = $v !== null && $c['binary'] ? '0x' . bin2hex($v) : $v; } $data[] = $item; }
        foreach ($columns as $c) if ($c['binary'] && in_array($c['name'], $key, true)) { $key = []; break; }
        return ['columns'=>$columns, 'key'=>$key, 'kind'=>$kind, 'rows'=>$data, 'total'=>(string)$db->query("SELECT COUNT(*) FROM $qt")->fetchColumn(), 'offset'=>$offset, 'limit'=>$limit];
    }
    if ($kind !== 'BASE TABLE') throw new Exception('Views and virtual tables are read-only in forms. Use SQL for advanced changes.');
    if ($op === 'create_index') return create_index($db,$table,$r,$map);
    if ($op === 'drop_index') {
        if (($r['confirmed'] ?? false)!==true) throw new Exception('Confirm index removal first.');
        $db->exec('BEGIN IMMEDIATE');
        try {
            [$liveKind,$liveDdl]=table_info($db,$table);
            if ($liveKind!=='BASE TABLE') throw new Exception('This table is read-only in forms.');
            $indexes=table_indexes($db,$table,columns($db,$table,$liveDdl),foreign_keys($db,$table));
            $live=null; foreach ($indexes as $index) if ($index['name']===($r['name']??'')) $live=$index;
            if (!$live || index_snapshot($live)!==index_snapshot($r['originalIndex']??null)) throw new Exception('This index changed or was removed. Refresh before trying again.');
            if (!$live['removable']) throw new Exception($live['reason']);
            $db->exec('DROP INDEX ' . ident($live['name'])); $db->exec('COMMIT');
        } catch (Throwable $e) { if ($db->inTransaction()) $db->rollBack(); throw $e; }
        return ['message'=>'Index removed. A SQLite backup was saved first.'];
    }
    if ($op === 'rename_table') { $db->exec("ALTER TABLE $qt RENAME TO " . ident($r['name'] ?? '')); return ['message'=>'Table renamed.']; }
    if ($op === 'drop_table') { $db->exec("DROP TABLE $qt"); return ['message'=>'Table deleted. A SQLite backup was saved first.']; }
    if ($op === 'add_column') { $db->exec("ALTER TABLE $qt ADD COLUMN " . column_sql($db, $r['column'] ?? [], true)); return ['message'=>'Column added.']; }
    if ($op === 'drop_column') { if (!isset($map[$r['name'] ?? ''])) throw new Exception('Choose a valid column.'); $db->exec("ALTER TABLE $qt DROP COLUMN " . ident($r['name'])); return ['message'=>'Column deleted. A SQLite backup was saved first.']; }
    if (!in_array($op, ['insert','update','delete'], true)) throw new Exception('Use SQL for this SQLite operation.');
    $values = $r['values'] ?? []; if (!is_array($values)) throw new Exception('Invalid row values.');
    foreach ($values as $name => $value) {
        if (!isset($map[$name]) || $map[$name]['generated'] || $map[$name]['binary']) throw new Exception('This column is read-only.');
        if ($value !== null && !is_string($value)) throw new Exception('Row values must be text or SQL NULL.');
        if ($value !== null && preg_match('/^INTEGER$/i', $map[$name]['type']) && (!preg_match('/^-?(0|[1-9][0-9]*)$/D', $value) || filter_var($value, FILTER_VALIDATE_INT) === false)) throw new Exception('Enter a whole number within the signed 64-bit range.');
    }
    if ($op === 'insert') {
        $sql = $values ? "INSERT INTO $qt (" . implode(',', array_map('ident', array_keys($values))) . ') VALUES (' . implode(',', array_fill(0, count($values), '?')) . ')' : "INSERT INTO $qt DEFAULT VALUES";
        $s = $db->prepare($sql); $s->execute(array_values($values)); return ['message'=>'Row inserted.'];
    }
    if (!$key) throw new Exception('This table has no non-null primary or unique key. Use SQL for ambiguous edits.');
    // Reserve the write lock before checking metadata, identity and original values.
    $db->exec('BEGIN IMMEDIATE');
    try {
        [$liveKind, $liveDdl] = table_info($db, $table);
        if ($liveKind !== 'BASE TABLE' || $liveDdl !== $ddl || stable_key($db, $table, columns($db, $table, $liveDdl), $liveKind) !== $key) throw new Exception('The table changed. Refresh before saving.');
        $original = $r['original'] ?? []; $where = []; $args = [];
        foreach ($columns as $c) {
            $name = $c['name']; if (!array_key_exists($name, $original)) throw new Exception('Refresh this row before changing it.');
            $value = $original[$name]; if ($value !== null && !is_string($value)) throw new Exception('Invalid original row.');
            // Reproduce the display representation, including dynamic BLOB storage.
            $binary = $c['binary'] || in_array($name, $r['binaryColumns'] ?? [], true);
            if ($binary && $value !== null) { if (!preg_match('/^0x([0-9a-f]{2})*$/iD', $value)) throw new Exception('Invalid binary value.'); $value = hex2bin(substr($value, 2)); }
            $qc = ident($name);
            $where[] = "CAST(CASE WHEN typeof($qc)='real' THEN quote($qc) ELSE $qc END AS BLOB) IS CAST(? AS BLOB)"; $args[] = $value;
        }
        $predicate = implode(' AND ', $where);
        if (count(rows($db, "SELECT 1 FROM $qt WHERE $predicate LIMIT 2", $args)) !== 1) throw new Exception('This row changed or was deleted. Refresh before saving.');
        if ($op === 'delete') { $s = $db->prepare("DELETE FROM $qt WHERE $predicate"); $s->execute($args); }
        elseif ($values) { $s = $db->prepare("UPDATE $qt SET " . implode(',', array_map(fn($name) => ident($name) . '=?', array_keys($values))) . " WHERE $predicate"); $s->execute([...array_values($values), ...$args]); }
        if (($op === 'delete' || $values) && $s->rowCount() !== 1) throw new Exception('This row changed or was deleted. Refresh before saving.');
        $db->exec('COMMIT');
    } catch (Throwable $e) { $db->exec('ROLLBACK'); throw $e; }
    return ['message'=>$op === 'delete' ? 'Row deleted.' : 'Row saved.'];
}
try {
    if (!extension_loaded('pdo_sqlite')) throw new Exception('The bundled PHP SQLite driver is unavailable.');
    $input = json_decode(stream_get_contents(STDIN), true, 512, JSON_THROW_ON_ERROR); $r = $input['request'];
    $flags = defined('Pdo\\Sqlite::ATTR_OPEN_FLAGS') ? constant('Pdo\\Sqlite::ATTR_OPEN_FLAGS') : constant('PDO::SQLITE_ATTR_OPEN_FLAGS');
    $readwrite = defined('Pdo\\Sqlite::OPEN_READWRITE') ? constant('Pdo\\Sqlite::OPEN_READWRITE') : constant('PDO::SQLITE_OPEN_READWRITE');
    $db = new PDO('sqlite:' . $r['path'], null, null, [PDO::ATTR_ERRMODE=>PDO::ERRMODE_EXCEPTION, PDO::ATTR_STRINGIFY_FETCHES=>true, $flags=>$readwrite]);
    $db->exec('PRAGMA busy_timeout=5000'); $db->exec('PRAGMA foreign_keys=ON');
    $start = microtime(true);
    if (($r['op'] ?? '') === 'backup') {
        $s = $db->prepare('VACUUM main INTO ?'); $s->execute([$input['backupPath']]); $result = ['message'=>'SQLite backup saved.'];
    } else $result = run_request($db, $r);
    $result['elapsedMs'] = (int)((microtime(true) - $start) * 1000);
    $json = json_encode(['result'=>$result], JSON_THROW_ON_ERROR);
    if (strlen($json) > 8 * 1024 * 1024) throw new Exception('The result is too large. Select fewer rows or columns.');
    echo $json;
} catch (Throwable $e) { echo json_encode(['error'=>$e->getMessage()], JSON_INVALID_UTF8_SUBSTITUTE); exit(1); }

<?php
// Used by CLI and Nginx/PHP smoke requests against a disposable external server.
set_error_handler(function ($severity, $message) { throw new Exception($message); });
try {
    if (!extension_loaded('pdo_pgsql') || !extension_loaded('pgsql')) {
        throw new Exception('Both PostgreSQL PHP extensions must be enabled.');
    }
    $port = (int)getenv('FAIRWAY_TEST_POSTGRES_PORT');
    $password = getenv('FAIRWAY_TEST_POSTGRES_PASSWORD');
    $dsn = "pgsql:host=127.0.0.1;port=$port;dbname=postgres;sslmode=disable;connect_timeout=5";
    $options = [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION, PDO::ATTR_STRINGIFY_FETCHES => true];
    $db = new PDO($dsn, 'fairway_client_test', $password, $options);
    // initdb uses SCRAM authentication: the fixture must not accidentally use trust.
    try {
        new PDO($dsn, 'fairway_client_test', $password . '-wrong', $options);
        throw new Exception('An incorrect password was accepted.');
    } catch (PDOException $expected) {}
    $db->beginTransaction();
    $db->exec('CREATE TEMP TABLE client_test(id BIGINT PRIMARY KEY, amount NUMERIC(20,4), name TEXT, extra JSONB)');
    $insert = $db->prepare('INSERT INTO client_test VALUES(?, ?, ?, ?::jsonb)');
    $insert->execute(['9007199254740993', '1234567890123.4567', "Student 日本's name", '{"ok":true}']);
    $row = $db->query("SELECT id::text, amount::text, name, extra->>'ok' AS ok FROM client_test")->fetch(PDO::FETCH_ASSOC);
    if ($row !== ['id' => '9007199254740993', 'amount' => '1234567890123.4567', 'name' => "Student 日本's name", 'ok' => 'true']) {
        throw new Exception('PDO values did not round-trip exactly.');
    }
    $update = $db->prepare('UPDATE client_test SET name=? WHERE id=?');
    $update->execute(['updated', '9007199254740993']);
    if ($update->rowCount() !== 1) throw new Exception('PDO update failed.');
    $db->exec('DELETE FROM client_test');
    if ($db->query('SELECT count(*) FROM client_test')->fetchColumn() !== '0') throw new Exception('PDO delete failed.');
    $db->rollBack();
    if ($db->query("SELECT count(*) FROM pg_class WHERE oid = to_regclass('pg_temp.client_test')")->fetchColumn() !== '0') {
        throw new Exception('PDO transaction rollback failed.');
    }

    // The password is generated as hexadecimal, so this test connection string needs no escaping.
    $pg = pg_connect("host=127.0.0.1 port=$port dbname=postgres user=fairway_client_test password=$password sslmode=disable connect_timeout=5");
    if (!$pg) throw new Exception('pgsql connection failed.');
    $result = pg_query_params($pg, 'SELECT $1::text AS name, $2::bigint::text AS id', ["Student 日本's name", '9007199254740993']);
    if (pg_fetch_assoc($result) !== ['name' => "Student 日本's name", 'id' => '9007199254740993']) {
        throw new Exception('pgsql values did not round-trip exactly.');
    }
    pg_close($pg);
    header('Content-Type: application/json');
    echo json_encode(['postgresClients' => 'ok', 'php' => PHP_VERSION], JSON_THROW_ON_ERROR);
} catch (Throwable $error) {
    http_response_code(500);
    // Do not include credentials or raw connection errors in HTTP/test output.
    echo json_encode(['postgresClients' => 'failed', 'type' => get_class($error)], JSON_THROW_ON_ERROR);
    exit(1);
}

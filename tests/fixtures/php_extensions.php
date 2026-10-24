<?php
// CLI and web checks use only disposable loopback sockets and in-memory SOAP.
set_error_handler(function ($severity, $message) { throw new Exception($message); });
function verify_extension_check(bool $condition, string $message): void {
    if (!$condition) throw new RuntimeException($message);
}
try {
    $required = ['sockets', 'soap', 'ldap'];
    if (PHP_OS_FAMILY !== 'Windows') $required = [...$required, 'pcntl', 'posix'];
    foreach ($required as $extension) {
        verify_extension_check(extension_loaded($extension), "Missing extension: $extension");
    }

    $sockets = [];
    try {
        $listener = socket_create(AF_INET, SOCK_STREAM, SOL_TCP);
        verify_extension_check($listener !== false, 'Socket creation failed.');
        $sockets[] = $listener;
        verify_extension_check(socket_bind($listener, '127.0.0.1', 0), 'Socket bind failed.');
        verify_extension_check(socket_getsockname($listener, $host, $port), 'Socket address failed.');
        verify_extension_check(socket_listen($listener, 1), 'Socket listen failed.');
        $client = socket_create(AF_INET, SOCK_STREAM, SOL_TCP);
        verify_extension_check($client !== false, 'Client socket creation failed.');
        $sockets[] = $client;
        verify_extension_check(socket_connect($client, '127.0.0.1', $port), 'Socket connect failed.');
        $peer = socket_accept($listener);
        verify_extension_check($peer !== false, 'Socket accept failed.');
        $sockets[] = $peer;
        verify_extension_check(socket_set_option($peer, SOL_SOCKET, SO_RCVTIMEO, ['sec' => 3, 'usec' => 0]), 'Socket timeout failed.');
        $payload = "Redis / AMQP 日本\0payload";
        verify_extension_check(socket_write($client, $payload) === strlen($payload), 'Socket write failed.');
        $received = '';
        while (strlen($received) < strlen($payload)) {
            $chunk = socket_read($peer, strlen($payload) - strlen($received));
            verify_extension_check($chunk !== false && $chunk !== '', 'Socket read failed.');
            $received .= $chunk;
        }
        verify_extension_check($received === $payload, 'Socket payload changed.');
    } finally {
        foreach (array_reverse($sockets) as $socket) socket_close($socket);
    }

    class ExtensionSoapService {
        public function reflectText(string $value): string { return $value; }
    }
    class ExtensionSoapClient extends SoapClient {
        // The sixth optional argument is used by PHP 8.5; earlier PHP accepts it too.
        public function __doRequest(string $request, string $location, string $action, int $version, bool $oneWay = false, ?string $uriParserClass = null): ?string {
            $server = new SoapServer(null, ['uri' => 'urn:fairway-extension-check']);
            $server->setClass(ExtensionSoapService::class);
            ob_start();
            try {
                $server->handle($request);
                return ob_get_contents();
            } finally {
                ob_end_clean();
            }
        }
    }
    $soap = new ExtensionSoapClient(null, ['uri' => 'urn:fairway-extension-check', 'location' => 'http://127.0.0.1/unused']);
    $text = 'Student 日本 <&>';
    verify_extension_check($soap->__soapCall('reflectText', [$text]) === $text, 'SOAP round trip failed.');

    // ldap_connect initializes the client lazily; no directory server is contacted.
    $ldap = ldap_connect('ldap://127.0.0.1:389');
    verify_extension_check($ldap !== false, 'LDAP initialization failed.');
    try {
        verify_extension_check(ldap_set_option($ldap, LDAP_OPT_PROTOCOL_VERSION, 3), 'LDAP protocol configuration failed.');
        verify_extension_check(ldap_get_option($ldap, LDAP_OPT_PROTOCOL_VERSION, $protocol) && $protocol === 3, 'LDAP protocol read failed.');
        verify_extension_check(ldap_escape('a*(b)', '', LDAP_ESCAPE_FILTER) === 'a\\2a\\28b\\29', 'LDAP filter escaping failed.');
    } finally {
        ldap_unbind($ldap);
    }

    // Forking/signal checks belong in CLI workers, never in a web worker.
    $worker = 'unavailable';
    if (PHP_OS_FAMILY !== 'Windows') {
        $worker = 'extensions';
        if (PHP_SAPI === 'cli') {
            verify_extension_check(posix_getpid() === getmypid(), 'POSIX process identity failed.');
            $signalled = false;
            $previous = pcntl_signal_get_handler(SIGUSR1);
            try {
                verify_extension_check(pcntl_signal(SIGUSR1, function () use (&$signalled) { $signalled = true; }), 'Signal handler failed.');
                verify_extension_check(posix_kill(posix_getpid(), SIGUSR1), 'Signal delivery failed.');
                pcntl_signal_dispatch();
                verify_extension_check($signalled, 'Signal handler was not called.');
            } finally {
                pcntl_signal(SIGUSR1, $previous);
            }
            $child = pcntl_fork();
            verify_extension_check($child !== -1, 'Worker fork failed.');
            if ($child === 0) exit(42);
            verify_extension_check(pcntl_waitpid($child, $status) === $child && pcntl_wifexited($status) && pcntl_wexitstatus($status) === 42, 'Worker child status failed.');
            $worker = 'ok';
        }
    }
    header('Content-Type: application/json');
    echo json_encode(['phpExtensions' => 'ok', 'php' => PHP_VERSION, 'worker' => $worker], JSON_THROW_ON_ERROR), PHP_EOL;
} catch (Throwable $error) {
    http_response_code(500);
    echo json_encode(['phpExtensions' => 'failed', 'error' => $error->getMessage()], JSON_THROW_ON_ERROR);
    exit(1);
}

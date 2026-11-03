<?php
// Plain PHP tests for website/api/crash.php (OUT-247): php website/api/tests/crash_test.php
declare(strict_types=1);

define('OPENSPIDER_CRASH_NO_MAIN', true);
require __DIR__ . '/../crash.php';

$failures = 0;
$checks = 0;
function check(bool $condition, string $message): void
{
    global $failures, $checks;
    $checks++;
    if (!$condition) {
        $failures++;
        fwrite(STDERR, "FAIL: $message\n");
    }
}

function temp_dir(): string
{
    $dir = sys_get_temp_dir() . DIRECTORY_SEPARATOR . 'os-crash-' . bin2hex(random_bytes(6));
    mkdir($dir, 0700);
    return $dir;
}

function remove_dir(string $dir): void
{
    foreach (array_diff(scandir($dir) ?: [], ['.', '..']) as $name) {
        $path = $dir . DIRECTORY_SEPARATOR . $name;
        is_dir($path) ? remove_dir($path) : unlink($path);
    }
    rmdir($dir);
}

$root = temp_dir();
// Fehlerlog in eine Datei umleiten: darin darf keine IP-Adresse stehen.
$errorLog = $root . DIRECTORY_SEPARATOR . 'php-error.log';
ini_set('log_errors', '1');
ini_set('error_log', $errorLog);
$config = [
    'data_dir' => $root . DIRECTORY_SEPARATOR . 'data',
    'salt_dir' => $root . DIRECTORY_SEPARATOR . 'salt',
    'mail_to' => 'inbox@example.invalid',
    'mail_from' => 'sender@example.invalid',
];
$mails = [];
$mailer = function (string $to, string $subject, string $body, array $headers) use (&$mails): bool {
    $mails[] = compact('to', 'subject', 'body', 'headers');
    return true;
};
$now = gmmktime(10, 30, 0, 11, 3, 2026);
$ip = '192.0.2.77';
$server = fn(string $ip = '192.0.2.77', array $extra = []) => $extra + ['REQUEST_METHOD' => 'POST', 'REMOTE_ADDR' => $ip, 'CONTENT_TYPE' => 'application/json'];
// Bericht-Fixture aus OUT-246, genau wie die App ihn sendet: {"report": <Text der Datei>, "note": …, "email": …}
$reportText = (string) file_get_contents(__DIR__ . '/../../../tests/fixtures/crash/crash-panic.json');
$report = json_decode($reportText, true);
$body = fn(array $report, string $note = '', string $email = '') => '{"report":' . json_encode($report, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES) . ',"note":' . json_encode($note) . ',"email":' . json_encode($email) . '}';
$files = fn() => glob($config['data_dir'] . '/crash-*.json') ?: [];
$json = fn(array $r) => json_decode($r['body'], true);

check(is_array($report) && count($report) === 19, 'fixture crash-panic.json has the 19 fields of OUT-246');

// --- Method, size, content type ---
$r = cr_handle(['REQUEST_METHOD' => 'GET'], '', $config, $mailer, $now);
check($r['status'] === 405 && $r['headers']['Allow'] === 'POST', 'GET is rejected with 405 and Allow: POST');
$big = $body(array_merge($report, ['log_tail' => [str_repeat('x', 81 * 1024)]]));
$r = cr_handle($server(), $big, $config, $mailer, $now);
check($r['status'] === 413 && $json($r) === ['ok' => false, 'error' => 'too_large'], '81 KB → 413 too_large');
$r = cr_handle($server($ip, ['CONTENT_LENGTH' => (string) (81 * 1024)]), '{}', $config, $mailer, $now);
check($r['status'] === 413, 'Content-Length over 80 KB → 413');
$r = cr_handle($server($ip, ['CONTENT_TYPE' => 'text/plain']), $body($report), $config, $mailer, $now);
check($r['status'] === 400 && $json($r)['error'] === 'invalid', 'only application/json is accepted');
check($files() === [] && $mails === [], 'rejected requests store and mail nothing');

// --- Validation → 400 invalid ---
$cases = [
    'wrong format' => $body(array_merge($report, ['format' => 2])),
    'format as string' => $body(array_merge($report, ['format' => '1'])),
    'missing field' => $body(array_diff_key($report, ['build' => true])),
    'extra field' => $body(array_merge($report, ['hostname' => 'x'])),
    'wrong type bool' => $body(array_merge($report, ['crawl_running' => 'yes'])),
    'wrong type list' => $body(array_merge($report, ['backtrace' => 'a, b'])),
    'list with number' => $body(array_merge($report, ['log_tail' => [1]])),
    'unknown kind' => $body(array_merge($report, ['kind' => 'oops'])),
    'unknown process' => $body(array_merge($report, ['process' => 'web'])),
    'id not uuid v4' => $body(array_merge($report, ['id' => 'abc'])),
    'message too long' => $body(array_merge($report, ['message' => str_repeat('m', 501)])),
    'too many frames' => $body(array_merge($report, ['backtrace' => array_fill(0, 65, 'f')])),
    'too many log lines' => $body(array_merge($report, ['log_tail' => array_fill(0, 201, 'l')])),
    'invalid email' => $body($report, '', 'kein@'),
    'email with header' => $body($report, '', "a@example.invalid\r\nBcc: b@example.invalid"),
    'email too long' => $body($report, '', str_repeat('a', 250) . '@example.invalid'),
    'note too long' => $body($report, str_repeat('ä', 2001)),
    'missing note' => '{"report":' . $reportText . ',"email":""}',
    'not json' => '{"report":',
    'list instead of object' => '[1,2]',
    'invalid utf-8' => "{\"report\":{},\"note\":\"\xC3\x28\",\"email\":\"\"}",
];
foreach ($cases as $name => $payload) {
    $r = cr_handle($server(), $payload, $config, $mailer, $now);
    check($r['status'] === 400 && $json($r) === ['ok' => false, 'error' => 'invalid'], "400 invalid: $name");
}
check($files() === [] && $mails === [], 'invalid reports store and mail nothing');
check((cr_validate($body($report, str_repeat('ä', 2000), 'ops@example.com'))['data']['note'] ?? null) === str_repeat('ä', 2000), '2 000 multibyte characters and a valid email are allowed');

// --- Success: reference, file, mail ---
$r = cr_handle($server(), '{"report":' . $reportText . ',"note":"Export gestartet\r\nZweite Zeile ä","email":"tester@example.invalid"}', $config, $mailer, $now);
$answer = $json($r);
check($r['status'] === 200 && $answer['ok'] === true, 'valid report → 200 ok');
check(is_string($answer['ref'] ?? null) && preg_match('/^[A-Z2-7]{8}$/', $answer['ref']) === 1, '8-character Base32 reference: ' . ($answer['ref'] ?? ''));
check(str_starts_with($r['headers']['Content-Type'], 'application/json'), 'JSON answer');
check(count($files()) === 1, 'report stored as one JSON file');
$stored = json_decode((string) file_get_contents($files()[0]), true);
check(array_keys($stored) === ['ref', 'received', 'note', 'email', 'report'], 'stored record has ref, received, note, email, report');
check($stored['ref'] === $answer['ref'] && $stored['received'] === '2026-11-03T10:30:00Z', 'reference and UTC time stored');
check($stored['report'] === $report, 'report stored unchanged');
check($stored['note'] === "Export gestartet\nZweite Zeile ä", 'note stored with normalised line breaks');
check(basename($files()[0]) === 'crash-20261103-103000-' . $answer['ref'] . '.json', 'file name crash-<UTC stamp>-<ref>.json');
check((fileperms($files()[0]) & 0777) === 0600, 'report file mode is 0600');
check(count($mails) === 1 && $mails[0]['to'] === 'inbox@example.invalid', 'one mail to mail_to');
$mail = $mails[0];
$expectedSubject = '[OpenSpider Absturz] 1.0.0-beta.2 macOS 15.1 Sequoia/aarch64 panic: called `Option::unwrap()` on a `None` value';
check($mail['subject'] === '=?UTF-8?B?' . base64_encode($expectedSubject) . '?=', 'subject [OpenSpider Absturz] <app_version> <os>/<arch> <kind>: <message>');
check(($mail['headers']['Reply-To'] ?? '') === 'tester@example.invalid', 'Reply-To for a valid email');
$decoded = quoted_printable_decode($mail['body']);
check(str_contains($decoded, 'Referenz: ' . $answer['ref']) && str_contains($decoded, '"kind": "panic"') && str_contains($decoded, 'Zweite Zeile ä'), 'mail body carries reference, note and report');
check(!preg_match('/[^\r]\n/', $mail['body']) && max(array_map('strlen', explode("\r\n", $mail['body']))) <= 76, 'quoted-printable body uses CRLF lines of at most 76 characters');
$long = array_merge($report, ['message' => str_repeat('Ä', 120), 'kind' => 'frontend', 'id' => '11111111-2222-4333-8444-555555555555']);
cr_handle($server('192.0.2.78'), $body($long), $config, $mailer, $now);
$subject = base64_decode(substr($mails[1]['subject'], 10, -2));
check(str_ends_with($subject, 'frontend: ' . str_repeat('Ä', 79) . '…'), 'long message shortened in the subject');
$evil = array_merge($report, ['os' => "Linux\r\nBcc: victim@example.invalid", 'id' => '11111111-2222-4333-8444-555555555556']);
cr_handle($server('192.0.2.79'), $body($evil), $config, $mailer, $now);
foreach ($mails[2]['headers'] as $name => $value) {
    check(!preg_match('/[\r\n]/', $name . $value), "header $name has no line break");
}
check(!preg_match('/[\r\n]/', $mails[2]['subject']), 'subject is encoded without line breaks');

// --- Rate limit: 10 per hour per salted daily IP hash ---
$mails = [];
for ($i = 0; $i < 10; $i++) {
    $r = cr_handle($server('198.51.100.7'), $body($report), $config, $mailer, $now + $i);
    check($r['status'] === 200, "report $i within limit");
}
$r = cr_handle($server('198.51.100.7'), $body($report), $config, $mailer, $now + 20);
check($r['status'] === 429 && $json($r) === ['ok' => false, 'error' => 'rate'], '11th report of the same IP hash within one hour → 429');
check(count($mails) === 10, 'rate-limited report sends no mail');
$r = cr_handle($server('198.51.100.8'), $body($report), $config, $mailer, $now + 21);
check($r['status'] === 200, 'another address is not affected');
$r = cr_handle($server('198.51.100.7'), $body($report), $config, $mailer, $now + CR_RATE_WINDOW + 1);
check($r['status'] === 200, 'limit resets after one hour');
$hashFiles = glob($config['salt_dir'] . '/rate-*');
check($hashFiles !== [] && preg_match('/^rate-[0-9a-f]{64}$/', basename($hashFiles[0])) === 1, 'rate files are named by HMAC-SHA-256 hash');

// --- Failures ---
$r = cr_handle($server('192.0.2.90'), $body($report), $config, fn() => false, $now);
check($r['status'] === 200, 'mail failure still stores and answers with a reference');
$broken = ['data_dir' => $root . '/file-not-dir'] + $config;
touch($broken['data_dir']);
$r = cr_handle($server('192.0.2.91'), $body($report), $broken, function () { throw new RuntimeException('smtp down'); }, $now);
check($r['status'] === 500 && $json($r) === ['ok' => false, 'error' => 'server'], 'neither stored nor mailed → 500 server');
$noSalt = ['salt_dir' => $root . '/file-not-dir'] + $config;
$r = cr_handle($server('192.0.2.92'), $body($report), $noSalt, $mailer, $now);
check($r['status'] === 500, 'rate limit storage unavailable → 500 server');

// --- No plaintext IP anywhere ---
$everything = '';
foreach (array_merge($files(), glob($config['salt_dir'] . '/*') ?: []) as $file) {
    $everything .= (string) file_get_contents($file);
}
foreach ($mails as $m) {
    $everything .= $m['subject'] . $m['body'] . implode('', $m['headers']);
}
$everything .= is_file($errorLog) ? (string) file_get_contents($errorLog) : '';
foreach (['192.0.2.77', '192.0.2.90', '192.0.2.91', '198.51.100.7', '198.51.100.8'] as $address) {
    check(!str_contains($everything, $address), "IP $address is stored in no file, mail or error log");
}
check(is_file($errorLog) && str_contains((string) file_get_contents($errorLog), 'openspider-crash: '), 'error log receives fixed messages');

// --- Reference format ---
$refs = [];
for ($i = 0; $i < 200; $i++) {
    $refs[] = cr_new_ref();
}
check(count(array_filter($refs, fn($r) => preg_match('/^[A-Z2-7]{8}$/', $r) === 1)) === 200, 'every reference has 8 Base32 characters');
check(count(array_unique($refs)) > 190, 'references are random');

// --- Configuration ---
$ini = $root . '/crash.ini';
file_put_contents($ini, "data_dir = /srv/data\nsalt_dir = /srv/salt\nmail_to = a@example.invalid\nmail_from = b@example.invalid\n");
check(cr_load_config($ini) === ['data_dir' => '/srv/data', 'mail_to' => 'a@example.invalid', 'mail_from' => 'b@example.invalid', 'salt_dir' => '/srv/salt'], 'INI configuration is read');
file_put_contents($ini, "data_dir = /srv/data\nsalt_dir = /srv/salt\nmail_to = a@example.invalid\n");
check(cr_load_config($ini) === null, 'missing key → no configuration');
check(cr_load_config('') === null && cr_load_config($root . '/missing.ini') === null, 'missing file → no configuration');
check(cr_load_config(__DIR__ . '/../crash.example.ini') !== null, 'example configuration is complete');
$r = cr_unconfigured();
check($r['status'] === 500 && $json($r) === ['ok' => false, 'error' => 'server'], 'missing configuration → 500 server (OUT-247)');

// --- Datenschutzerklärung de/en beschreibt Absturzberichte (OUT-247) ---
foreach (['datenschutz/index.html' => ['absturzberichte', '90 Tagen', '„Senden“'], 'en/privacy/index.html' => ['crash-reports', '90 days', '“Send”']] as $page => [$anchor, $retention, $click]) {
    $html = (string) file_get_contents(__DIR__ . '/../../site/' . $page);
    check(str_contains($html, 'id="' . $anchor . '"'), "$page has a crash report section");
    check(str_contains($html, $retention), "$page names the retention on the server");
    check(str_contains($html, $click) && str_contains($html, '/api/crash'), "$page says reports are sent only after the click");
}

remove_dir($root);
echo $failures === 0 ? "OK: $checks checks\n" : "$failures of $checks checks failed\n";
exit($failures === 0 ? 0 : 1);

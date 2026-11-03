<?php
// Plain PHP tests for website/api/feedback.php: php website/api/tests/feedback_test.php
declare(strict_types=1);

define('OPENSPIDER_FEEDBACK_NO_MAIN', true);
require __DIR__ . '/../feedback.php';

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
    $dir = sys_get_temp_dir() . DIRECTORY_SEPARATOR . 'os-feedback-' . bin2hex(random_bytes(6));
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
$now = gmmktime(10, 30, 0, 11, 2, 2026);
$server = fn(string $ip = '192.0.2.1', array $extra = []) => $extra + ['REQUEST_METHOD' => 'POST', 'REMOTE_ADDR' => $ip, 'CONTENT_LENGTH' => '400', 'HTTP_USER_AGENT' => 'Mozilla/5.0 (Fixture) ' . str_repeat('x', 400)];
$valid = [
    'lang' => 'de', 'system' => 'macos-arm', 'version' => '1.0.0-beta.2', 'type' => 'bug',
    'description' => "Crawl bleibt bei 50 % stehen.\r\nZweite Zeile ä€😀", 'tested' => 'fixture.test, 12.000 URLs',
    'email' => '', 'consent' => 'yes', 'website' => '',
];
$jsonl = fn() => $config['data_dir'] . DIRECTORY_SEPARATOR . 'feedback-2026-11.jsonl';
$lines = fn() => is_file($jsonl()) ? file($jsonl(), FILE_IGNORE_NEW_LINES) : [];
$location = fn(array $response) => $response['headers']['Location'] ?? '';

// --- Method and size ---
$r = fb_handle(['REQUEST_METHOD' => 'GET'], [], $config, $mailer, $now);
check($r['status'] === 405 && $r['headers']['Allow'] === 'POST', 'GET is rejected with 405 and Allow: POST');
$r = fb_handle($server('192.0.2.1', ['CONTENT_LENGTH' => '20481']), $valid, $config, $mailer, $now);
check($r['status'] === 303 && $location($r) === '/beta/?error=too_large#feedback-error-too_large', 'body over 20 KB is rejected');
check($lines() === [] && $mails === [], 'oversized body stores and mails nothing');

// --- Validation ---
$cases = [
    'description' => ['description' => "  \r\n "],
    'description ' => ['description' => str_repeat('a', 5001)],
    'consent' => ['consent' => ''],
    'email' => ['email' => "a@example.invalid\r\nBcc: victim@example.invalid"],
    'invalid' => ['system' => 'beos'],
    'invalid ' => ['type' => 'rant'],
    'invalid  ' => ['version' => str_repeat('1', 65)],
    'invalid   ' => ['description' => "\xC3\x28"],
    'invalid    ' => ['description' => ['array']],
];
foreach ($cases as $code => $change) {
    $code = trim($code);
    $r = fb_handle($server(), array_merge($valid, $change), $config, $mailer, $now);
    check($r['status'] === 303 && str_starts_with($location($r), '/beta/?error=' . $code) && str_ends_with($location($r), '#feedback-error-' . $code), "validation error $code: " . $location($r));
}
check(fb_validate(array_merge($valid, ['description' => str_repeat('ä', 5000)]))['data']['description'] === str_repeat('ä', 5000), '5,000 multibyte characters are allowed');
$r = fb_handle($server(), array_merge($valid, ['lang' => 'en', 'consent' => 'no']), $config, $mailer, $now);
check($location($r) === '/en/beta/?error=consent&version=1.0.0-beta.2#feedback-error-consent', 'English error keeps the version for the form: ' . $location($r));
$r = fb_handle($server(), array_merge($valid, ['lang' => 'xx', 'consent' => 'no', 'version' => "1.0\r\nX: y"]), $config, $mailer, $now);
check(!preg_match('/[\r\n]/', $location($r)) && str_starts_with($location($r), '/beta/?error=consent&version=1.0%20X%3A%20y#'), 'unknown language falls back to German; Location has no line breaks: ' . $location($r));
check($lines() === [] && $mails === [], 'invalid submissions store and mail nothing');

// --- Honeypot ---
$r = fb_handle($server(), array_merge($valid, ['website' => 'https://spam.example']), $config, $mailer, $now);
check($r['status'] === 303 && $location($r) === '/beta/danke/', 'filled honeypot looks like success');
check($lines() === [] && $mails === [], 'honeypot stores and mails nothing');
check(!is_dir($config['salt_dir']) || glob($config['salt_dir'] . '/rate-*') === [], 'honeypot does not count against the rate limit');

// --- Success, storage format, mail ---
$r = fb_handle($server(), $valid, $config, $mailer, $now);
check($r['status'] === 303 && $location($r) === '/beta/danke/', 'valid German submission redirects to /beta/danke/');
$stored = $lines();
check(count($stored) === 1, 'one JSON line per submission');
$record = json_decode($stored[0] ?? '{}', true);
check(array_keys($record) === ['id', 'time', 'lang', 'system', 'version', 'type', 'description', 'tested', 'email', 'user_agent'], 'record has exactly the documented fields');
check($record['time'] === '2026-11-02T10:30:00Z' && $record['lang'] === 'de' && $record['system'] === 'macos-arm' && $record['type'] === 'bug', 'UTC time and fields stored');
check($record['description'] === "Crawl bleibt bei 50 % stehen.\nZweite Zeile ä€😀", 'description stored with normalised line breaks and Unicode');
check(fb_length($record['user_agent']) === 200 && str_starts_with($record['user_agent'], 'Mozilla/5.0 (Fixture) '), 'user agent truncated to 200 characters');
check(!str_contains($stored[0], '192.0.2.1') && !str_contains(implode('', array_map('file_get_contents', glob($config['salt_dir'] . '/*'))), '192.0.2.1'), 'IP address is stored nowhere');
check((fileperms($jsonl()) & 0777) === 0600, 'feedback file mode is 0600');
check(count($mails) === 1 && $mails[0]['to'] === 'inbox@example.invalid', 'one mail to mail_to');
$mail = $mails[0];
check($mail['subject'] === '=?UTF-8?B?' . base64_encode('[OpenSpider Beta] Fehler – macOS (Apple Silicon) – 1.0.0-beta.2') . '?=', 'subject format [OpenSpider Beta] <Art> – <System> – <Version>');
check(!isset($mail['headers']['Reply-To']) && $mail['headers']['From'] === 'sender@example.invalid', 'no Reply-To without email');
$decoded = quoted_printable_decode($mail['body']);
check(str_contains($decoded, "Zweite Zeile ä€😀") && str_contains($decoded, 'ID: ' . $record['id']) && !str_contains($decoded, '192.0.2.1'), 'plain text body contains the feedback, not the IP');
check(!preg_match('/[^\r]\n/', $mail['body']) && max(array_map('strlen', explode("\r\n", $mail['body']))) <= 76, 'quoted-printable body uses CRLF lines of at most 76 characters');

// --- Reply-To and header injection ---
$mails = [];
$r = fb_handle($server('192.0.2.2'), array_merge($valid, ['lang' => 'en', 'email' => 'tester@example.invalid', 'type' => 'idea', 'system' => 'windows-arm', 'version' => '']), $config, $mailer, $now);
check($location($r) === '/en/beta/thanks/', 'English submission redirects to /en/beta/thanks/');
check(($mails[0]['headers']['Reply-To'] ?? '') === 'tester@example.invalid', 'Reply-To set for a valid email');
check(base64_decode(substr($mails[0]['subject'], 10, -2)) === '[OpenSpider Beta] Idee – Windows ARM – unbekannt', 'subject without version');
$mails = [];
$evil = "1.0\r\nBcc: victim@example.invalid";
$r = fb_handle($server('192.0.2.3'), array_merge($valid, ['version' => $evil]), $config, $mailer, $now);
check($location($r) === '/beta/danke/' && count($mails) === 1, 'version with line break is accepted as text');
foreach ($mails[0]['headers'] as $name => $value) {
    check(!preg_match('/[\r\n]/', $name . $value), "header $name has no line break");
}
check(!preg_match('/[\r\n]/', $mails[0]['subject']) && !str_contains($mails[0]['subject'], 'Bcc'), 'subject is encoded without line breaks');
check(json_decode($lines()[2], true)['version'] === '1.0 Bcc: victim@example.invalid', 'control characters in single-line fields are replaced');
foreach (["a@example.invalid\nBcc: b@example.invalid", "a@example.invalid%0ABcc:b@example.invalid", "\"a\r\n\"@example.invalid", str_repeat('a', 250) . '@example.invalid'] as $email) {
    check(fb_validate(array_merge($valid, ['email' => $email])) === ['error' => 'email'], 'rejects email ' . json_encode($email));
}

// --- Mail failure does not prevent storage ---
$before = count($lines());
$r = fb_handle($server('192.0.2.4'), $valid, $config, fn() => false, $now);
check($location($r) === '/beta/danke/' && count($lines()) === $before + 1, 'mail failure still stores and confirms');
$r = fb_handle($server('192.0.2.4'), $valid, $config, function () { throw new RuntimeException('smtp down'); }, $now);
check($location($r) === '/beta/danke/' && count($lines()) === $before + 2, 'mailer exception still stores and confirms');
$broken = ['data_dir' => $root . '/file-not-dir'] + $config;
touch($broken['data_dir']);
$r = fb_handle($server('192.0.2.5'), $valid, $broken, fn() => false, $now);
check(str_starts_with($location($r), '/beta/?error=server'), 'neither stored nor mailed → server error');

// --- Rate limit: 5 per hour per salted IP hash ---
$mails = [];
for ($i = 0; $i < 5; $i++) {
    $r = fb_handle($server('198.51.100.7'), $valid, $config, $mailer, $now + $i);
    check($location($r) === '/beta/danke/', "submission $i within limit");
}
$r = fb_handle($server('198.51.100.7'), $valid, $config, $mailer, $now + 10);
check(str_starts_with($location($r), '/beta/?error=rate'), 'sixth submission within one hour is rate limited');
check(count($mails) === 5, 'rate-limited submission sends no mail');
$r = fb_handle($server('198.51.100.8'), $valid, $config, $mailer, $now + 11);
check($location($r) === '/beta/danke/', 'another address is not affected');
$r = fb_handle($server('198.51.100.7'), $valid, $config, $mailer, $now + FB_RATE_WINDOW + 1);
check($location($r) === '/beta/danke/', 'limit resets after one hour');
$salts = glob($config['salt_dir'] . '/salt-*');
check(count($salts) === 1 && (fileperms($salts[0]) & 0777) === 0600, 'one daily salt with mode 0600');
$hashFiles = glob($config['salt_dir'] . '/rate-*');
check($hashFiles !== [] && preg_match('/^rate-[0-9a-f]{64}$/', basename($hashFiles[0])) === 1, 'rate files are named by HMAC-SHA-256 hash');
$tomorrow = $now + 86400;
fb_handle($server('198.51.100.9'), $valid, $config, $mailer, $tomorrow);
$newSalts = glob($config['salt_dir'] . '/salt-*');
check(count($newSalts) === 1 && $newSalts !== $salts, 'salt changes daily and the old salt is deleted');
check(count(glob($config['salt_dir'] . '/rate-*')) === 1, 'rate files older than the window are deleted');
check(count(file($config['data_dir'] . '/feedback-2026-11.jsonl')) === count($lines()), 'monthly file name in UTC');

// --- Configuration ---
$ini = $root . '/feedback.ini';
file_put_contents($ini, "data_dir = /srv/data\nsalt_dir = /srv/salt\nmail_to = a@example.invalid\nmail_from = b@example.invalid\n");
check(fb_load_config($ini) === ['data_dir' => '/srv/data', 'mail_to' => 'a@example.invalid', 'mail_from' => 'b@example.invalid', 'salt_dir' => '/srv/salt'], 'INI configuration is read');
file_put_contents($ini, "data_dir = /srv/data\nsalt_dir = /srv/salt\nmail_to = a@example.invalid\n");
check(fb_load_config($ini) === null, 'missing key → no configuration');
check(fb_load_config('') === null && fb_load_config($root . '/missing.ini') === null, 'missing file → no configuration');
check(fb_load_config(__DIR__ . '/../feedback.example.ini') !== null, 'example configuration is complete');

// --- The form pages display every error code and post the expected fields ---
foreach (['beta/index.html', 'en/beta/index.html'] as $page) {
    $html = (string) file_get_contents(__DIR__ . '/../../site/' . $page);
    foreach (FB_ERRORS as $code) {
        check(str_contains($html, 'id="feedback-error-' . $code . '"'), "$page shows error $code");
    }
    foreach (['lang', 'system', 'version', 'type', 'description', 'tested', 'email', 'consent', 'website'] as $field) {
        check(str_contains($html, 'name="' . $field . '"'), "$page posts field $field");
    }
    foreach (array_merge(array_keys(FB_SYSTEMS), array_keys(FB_TYPES)) as $value) {
        check(str_contains($html, 'value="' . $value . '"'), "$page offers $value");
    }
    check(str_contains($html, 'action="/api/feedback" method="post"'), "$page posts to /api/feedback");
}
foreach (['de', 'en'] as $lang) {
    check(is_file(__DIR__ . '/../../site' . FB_PAGES[$lang]['thanks'] . 'index.html'), "thanks page for $lang exists");
}

remove_dir($root);
echo $failures === 0 ? "OK: $checks checks\n" : "$failures of $checks checks failed\n";
exit($failures === 0 ? 0 : 1);

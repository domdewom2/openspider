<?php
// OpenSpider crash report endpoint (POST /api/crash, OUT-247). PHP 8.3, no dependencies.
// The app sends a report only after the user clicked "Send". Same operating rules as feedback.php:
// configuration outside the repository (INI named by OPENSPIDER_CRASH_CONFIG with data_dir, mail_to,
// mail_from, salt_dir), no IP address is stored, the error log receives fixed messages only.
declare(strict_types=1);

const CR_MAX_BODY = 81920;           // 80 KB
const CR_MAX_NOTE = 2000;
const CR_MAX_EMAIL = 254;
const CR_RATE_LIMIT = 10;
const CR_RATE_WINDOW = 3600;
const CR_SUBJECT_MESSAGE = 80;
const CR_BASE32 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
const CR_KINDS = ['panic', 'frontend', 'unclean_exit'];
const CR_PROCESSES = ['app', 'cli'];
/** Report fields of OUT-246 in their order: name => [type, maximum length in characters (strings) or entries (lists)]. */
const CR_FIELDS = [
    'format' => ['int', 0],
    'id' => ['string', 36],
    'kind' => ['string', 16],
    'time' => ['string', 40],
    'app_version' => ['string', 64],
    'build' => ['string', 64],
    'os' => ['string', 200],
    'arch' => ['string', 32],
    'process' => ['string', 8],
    'uptime_s' => ['int', 0],
    'thread' => ['string', 200],
    'message' => ['string', 500],
    'location' => ['string', 500],
    'backtrace' => ['list', 64],
    'crawl_running' => ['bool', 0],
    'rendering_active' => ['bool', 0],
    'log_tail' => ['list', 200],
    'sent' => ['?string', 64],
    'server_ref' => ['?string', 64],
];

/** Reads and validates the INI configuration; null when unusable. */
function cr_load_config(string $path): ?array
{
    if ($path === '' || !is_file($path) || !is_readable($path)) {
        return null;
    }
    $ini = parse_ini_file($path, false, INI_SCANNER_RAW);
    if (!is_array($ini)) {
        return null;
    }
    $config = [];
    foreach (['data_dir', 'mail_to', 'mail_from', 'salt_dir'] as $key) {
        $value = isset($ini[$key]) && is_string($ini[$key]) ? trim($ini[$key]) : '';
        if ($value === '') {
            return null;
        }
        $config[$key] = $value;
    }
    foreach (['mail_to', 'mail_from'] as $key) {
        if (!cr_valid_email($config[$key])) {
            return null;
        }
    }
    return $config;
}

function cr_valid_email(string $email): bool
{
    return $email !== ''
        && strlen($email) <= CR_MAX_EMAIL
        && !preg_match('/[\x00-\x1f\x7f]/', $email)
        && filter_var($email, FILTER_VALIDATE_EMAIL) !== false;
}

/** Character count of valid UTF-8 without requiring mbstring. */
function cr_length(string $text): int
{
    return function_exists('mb_strlen') ? mb_strlen($text, 'UTF-8') : (int) preg_match_all('/./su', $text);
}

/** Truncates valid UTF-8 to at most $max characters. */
function cr_truncate(string $text, int $max): string
{
    if (function_exists('mb_substr')) {
        return mb_substr($text, 0, $max, 'UTF-8');
    }
    preg_match('/^.{0,' . $max . '}/su', $text, $match);
    return $match[0] ?? '';
}

/** Single line: all control characters (including CR/LF) replaced by spaces. */
function cr_line(string $text): string
{
    return trim((string) preg_replace('/[\x00-\x1f\x7f]+/', ' ', $text));
}

/** Checks one report against the field names and types of OUT-246. */
function cr_valid_report(mixed $report): bool
{
    if (!is_array($report) || array_is_list($report)) {
        return false;
    }
    $keys = array_keys($report);
    $expected = array_keys(CR_FIELDS);
    sort($keys);
    sort($expected);
    if ($keys !== $expected) {
        return false;
    }
    foreach (CR_FIELDS as $name => [$type, $max]) {
        $value = $report[$name];
        $ok = match ($type) {
            'int' => is_int($value) && $value >= 0,
            'bool' => is_bool($value),
            'string' => is_string($value) && cr_length($value) <= $max,
            '?string' => $value === null || (is_string($value) && cr_length($value) <= $max),
            'list' => is_array($value) && array_is_list($value) && count($value) <= $max
                && array_reduce($value, fn(bool $all, mixed $line) => $all && is_string($line) && strlen($line) <= 16384, true),
            default => false,
        };
        if (!$ok) {
            return false;
        }
    }
    return $report['format'] === 1
        && preg_match('/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/', $report['id']) === 1
        && in_array($report['kind'], CR_KINDS, true)
        && in_array($report['process'], CR_PROCESSES, true);
}

/** Validates the request body. Returns ['error' => code] or ['data' => [report, note, email]]. */
function cr_validate(string $body): array
{
    if (!preg_match('//u', $body)) {
        return ['error' => 'invalid'];
    }
    try {
        $data = json_decode($body, true, 16, JSON_THROW_ON_ERROR);
    } catch (JsonException) {
        return ['error' => 'invalid'];
    }
    if (!is_array($data) || array_is_list($data)) {
        return ['error' => 'invalid'];
    }
    $keys = array_keys($data);
    sort($keys);
    if ($keys !== ['email', 'note', 'report']) {
        return ['error' => 'invalid'];
    }
    if (!is_string($data['note']) || !is_string($data['email']) || !cr_valid_report($data['report'])) {
        return ['error' => 'invalid'];
    }
    $note = str_replace(["\r\n", "\r"], "\n", $data['note']);
    $email = trim($data['email']);
    if (cr_length($note) > CR_MAX_NOTE || ($email !== '' && !cr_valid_email($email))) {
        return ['error' => 'invalid'];
    }
    return ['data' => ['report' => $data['report'], 'note' => $note, 'email' => $email]];
}

function cr_ensure_dir(string $dir): bool
{
    return is_dir($dir) || (@mkdir($dir, 0700, true) && is_dir($dir));
}

/** Daily salt (UTC day). Salts of other days are deleted, so a salt lives at most 24 hours. */
function cr_daily_salt(string $saltDir, int $now): ?string
{
    if (!cr_ensure_dir($saltDir)) {
        return null;
    }
    $file = $saltDir . DIRECTORY_SEPARATOR . 'salt-' . gmdate('Y-m-d', $now);
    foreach (glob($saltDir . DIRECTORY_SEPARATOR . 'salt-*') ?: [] as $old) {
        if ($old !== $file) {
            @unlink($old);
        }
    }
    $handle = @fopen($file, 'c+');
    if ($handle === false) {
        return null;
    }
    @chmod($file, 0600);
    $salt = null;
    if (flock($handle, LOCK_EX)) {
        $salt = trim((string) stream_get_contents($handle));
        if (!preg_match('/^[0-9a-f]{64}$/', $salt)) {
            $salt = bin2hex(random_bytes(32));
            ftruncate($handle, 0);
            rewind($handle);
            fwrite($handle, $salt);
            fflush($handle);
        }
        flock($handle, LOCK_UN);
    }
    fclose($handle);
    return $salt;
}

/** Sliding one-hour window per salted IP hash (HMAC-SHA-256 with the daily salt); records allowed reports. */
function cr_rate_allow(string $saltDir, string $ip, int $now): ?bool
{
    $salt = cr_daily_salt($saltDir, $now);
    if ($salt === null) {
        return null;
    }
    $file = $saltDir . DIRECTORY_SEPARATOR . 'rate-' . hash_hmac('sha256', $ip, $salt);
    foreach (glob($saltDir . DIRECTORY_SEPARATOR . 'rate-*') ?: [] as $other) {
        $mtime = @filemtime($other);
        if ($other !== $file && $mtime !== false && $mtime < $now - CR_RATE_WINDOW) {
            @unlink($other);
        }
    }
    $handle = @fopen($file, 'c+');
    if ($handle === false) {
        return null;
    }
    @chmod($file, 0600);
    if (!flock($handle, LOCK_EX)) {
        fclose($handle);
        return null;
    }
    $times = [];
    foreach (preg_split('/\s+/', (string) stream_get_contents($handle), -1, PREG_SPLIT_NO_EMPTY) as $line) {
        if (ctype_digit($line) && (int) $line > $now - CR_RATE_WINDOW && (int) $line <= $now) {
            $times[] = (int) $line;
        }
    }
    $allowed = count($times) < CR_RATE_LIMIT;
    if ($allowed) {
        $times[] = $now;
    }
    ftruncate($handle, 0);
    rewind($handle);
    fwrite($handle, implode("\n", $times) . "\n");
    fflush($handle);
    flock($handle, LOCK_UN);
    fclose($handle);
    @touch($file, $now);
    return $allowed;
}

/** Short random reference: 8 characters Base32 (40 random bits). */
function cr_new_ref(): string
{
    $bits = '';
    foreach (str_split(random_bytes(5)) as $byte) {
        $bits .= str_pad(decbin(ord($byte)), 8, '0', STR_PAD_LEFT);
    }
    $ref = '';
    foreach (str_split($bits, 5) as $chunk) {
        $ref .= CR_BASE32[bindec($chunk)];
    }
    return $ref;
}

/** Stores one received report as data_dir/crash-YYYYMMDD-HHMMSS-<ref>.json (UTC), mode 0600, never overwriting. */
function cr_store(string $dataDir, array $record, int $now): bool
{
    if (!cr_ensure_dir($dataDir)) {
        return false;
    }
    $json = json_encode($record, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
    if ($json === false) {
        return false;
    }
    $file = $dataDir . DIRECTORY_SEPARATOR . 'crash-' . gmdate('Ymd-His', $now) . '-' . $record['ref'] . '.json';
    $previous = umask(0077);
    $handle = @fopen($file, 'xb');
    umask($previous);
    if ($handle === false) {
        return false;
    }
    @chmod($file, 0600);
    $ok = fwrite($handle, $json . "\n") === strlen($json) + 1 && fflush($handle);
    fclose($handle);
    if (!$ok) {
        @unlink($file);
    }
    return $ok;
}

/** Plain subject text: [OpenSpider Absturz] <app_version> <os>/<arch> <kind>: <message shortened>. */
function cr_subject_text(array $report): string
{
    $message = cr_line($report['message']);
    if (cr_length($message) > CR_SUBJECT_MESSAGE) {
        $message = cr_truncate($message, CR_SUBJECT_MESSAGE - 1) . '…';
    }
    return '[OpenSpider Absturz] ' . cr_line($report['app_version']) . ' ' . cr_line($report['os']) . '/'
        . cr_line($report['arch']) . ' ' . $report['kind'] . ': ' . $message;
}

/** Mail as [to, subject, body, headers]; RFC 2047 subject, quoted-printable UTF-8 body with CRLF line breaks. */
function cr_build_mail(array $config, array $record): array
{
    $report = json_encode($record['report'], JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
    $lines = [
        'Referenz: ' . $record['ref'],
        'Empfangen (UTC): ' . $record['received'],
        'E-Mail: ' . ($record['email'] !== '' ? $record['email'] : '–'),
        '',
        'Was zuletzt gemacht wurde:',
        $record['note'] !== '' ? $record['note'] : '–',
        '',
        'Bericht:',
        (string) $report,
    ];
    $headers = [
        'From' => $config['mail_from'],
        'MIME-Version' => '1.0',
        'Content-Type' => 'text/plain; charset=UTF-8',
        'Content-Transfer-Encoding' => 'quoted-printable',
        'X-Mailer' => 'OpenSpider crash',
    ];
    if ($record['email'] !== '' && cr_valid_email($record['email'])) {
        $headers['Reply-To'] = $record['email'];
    }
    $subject = '=?UTF-8?B?' . base64_encode(cr_subject_text($record['report'])) . '?=';
    $body = quoted_printable_encode(str_replace("\n", "\r\n", implode("\n", $lines)));
    return [$config['mail_to'], $subject, $body, $headers];
}

/** Default mailer; tests replace it via the $mailer argument of cr_handle(). */
function cr_send_mail(string $to, string $subject, string $body, array $headers): bool
{
    return mail($to, $subject, $body, $headers);
}

function cr_json(int $status, array $payload, array $headers = []): array
{
    return [
        'status' => $status,
        'headers' => $headers + ['Content-Type' => 'application/json; charset=UTF-8'],
        'body' => json_encode($payload, JSON_UNESCAPED_SLASHES) . "\n",
    ];
}

function cr_error(int $status, string $code, array $headers = []): array
{
    return cr_json($status, ['ok' => false, 'error' => $code], $headers);
}

/**
 * Handles one request and returns ['status', 'headers', 'body'].
 * $body: raw request body (at most CR_MAX_BODY + 1 bytes are read by cr_main()).
 * $mailer: callable(string $to, string $subject, string $body, array $headers): bool
 */
function cr_handle(array $server, string $body, array $config, callable $mailer, int $now): array
{
    if (($server['REQUEST_METHOD'] ?? '') !== 'POST') {
        return cr_error(405, 'method', ['Allow' => 'POST']);
    }
    $length = $server['CONTENT_LENGTH'] ?? null;
    if ((is_numeric($length) && (int) $length > CR_MAX_BODY) || strlen($body) > CR_MAX_BODY) {
        return cr_error(413, 'too_large');
    }
    $type = strtolower(trim(explode(';', (string) ($server['CONTENT_TYPE'] ?? ''))[0]));
    if ($type !== 'application/json') {
        return cr_error(400, 'invalid');
    }
    $result = cr_validate($body);
    if (isset($result['error'])) {
        return cr_error(400, $result['error']);
    }
    $data = $result['data'];
    $allowed = cr_rate_allow($config['salt_dir'], (string) ($server['REMOTE_ADDR'] ?? ''), $now);
    if ($allowed === null) {
        error_log('openspider-crash: rate limit storage unavailable');
        return cr_error(500, 'server');
    }
    if (!$allowed) {
        return cr_error(429, 'rate');
    }
    $record = [
        'ref' => cr_new_ref(),
        'received' => gmdate('Y-m-d\TH:i:s\Z', $now),
        'note' => $data['note'],
        'email' => $data['email'],
        'report' => $data['report'],
    ];
    $stored = cr_store($config['data_dir'], $record, $now);
    if (!$stored) {
        error_log('openspider-crash: storing report failed');
    }
    $sent = false;
    try {
        $sent = (bool) $mailer(...cr_build_mail($config, $record));
    } catch (Throwable) {
        $sent = false;
    }
    if (!$sent) {
        error_log('openspider-crash: sending notification mail failed');
    }
    if (!$stored && !$sent) {
        return cr_error(500, 'server');
    }
    return cr_json(200, ['ok' => true, 'ref' => $record['ref']]);
}

function cr_main(): void
{
    $path = getenv('OPENSPIDER_CRASH_CONFIG');
    if (!is_string($path) || $path === '') {
        $path = is_string($_SERVER['OPENSPIDER_CRASH_CONFIG'] ?? null) ? $_SERVER['OPENSPIDER_CRASH_CONFIG'] : '';
    }
    $config = cr_load_config($path);
    if ($config === null) {
        error_log('openspider-crash: configuration missing or invalid');
        $response = cr_error(503, 'server');
    } else {
        $body = (string) file_get_contents('php://input', false, null, 0, CR_MAX_BODY + 1);
        $response = cr_handle($_SERVER, $body, $config, 'cr_send_mail', time());
    }
    http_response_code($response['status']);
    header('Cache-Control: no-store');
    header('X-Content-Type-Options: nosniff');
    header('Referrer-Policy: no-referrer');
    foreach ($response['headers'] as $name => $value) {
        header($name . ': ' . $value);
    }
    echo $response['body'];
}

if (!defined('OPENSPIDER_CRASH_NO_MAIN')) {
    cr_main();
}

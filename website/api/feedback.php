<?php
// OpenSpider beta feedback endpoint (POST /api/feedback). PHP 8.3, no dependencies.
// Configuration lives outside the repository: an INI file named by the FPM environment
// variable OPENSPIDER_FEEDBACK_CONFIG (keys data_dir, mail_to, mail_from, salt_dir).
// Never stores IP addresses and never writes form contents to the error log.
declare(strict_types=1);

const FB_MAX_BODY = 20480;
const FB_MAX_DESCRIPTION = 5000;
const FB_MAX_TESTED = 1000;
const FB_MAX_VERSION = 64;
const FB_MAX_EMAIL = 254;
const FB_MAX_USER_AGENT = 200;
const FB_RATE_LIMIT = 5;
const FB_RATE_WINDOW = 3600;
const FB_SYSTEMS = [
    'macos-arm' => 'macOS (Apple Silicon)',
    'macos-intel' => 'macOS (Intel)',
    'windows-x64' => 'Windows x64',
    'windows-arm' => 'Windows ARM',
    'linux-cli' => 'Linux (CLI)',
];
const FB_TYPES = ['bug' => 'Fehler', 'idea' => 'Idee', 'praise' => 'Lob', 'other' => 'Sonstiges'];
const FB_PAGES = [
    'de' => ['form' => '/beta/', 'thanks' => '/beta/danke/'],
    'en' => ['form' => '/en/beta/', 'thanks' => '/en/beta/thanks/'],
];
/** Error codes the form pages can display (element id feedback-error-<code>). */
const FB_ERRORS = ['description', 'consent', 'email', 'invalid', 'too_large', 'rate', 'server'];

/** Reads and validates the INI configuration; null when unusable. */
function fb_load_config(string $path): ?array
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
        if (!fb_valid_email($config[$key])) {
            return null;
        }
    }
    return $config;
}

function fb_valid_email(string $email): bool
{
    return $email !== ''
        && strlen($email) <= FB_MAX_EMAIL
        && !preg_match('/[\x00-\x1f\x7f]/', $email)
        && filter_var($email, FILTER_VALIDATE_EMAIL) !== false;
}

/** Character count of valid UTF-8 without requiring mbstring. */
function fb_length(string $text): int
{
    return function_exists('mb_strlen') ? mb_strlen($text, 'UTF-8') : (int) preg_match_all('/./su', $text);
}

/** Truncates valid UTF-8 to at most $max characters. */
function fb_truncate(string $text, int $max): string
{
    if (function_exists('mb_substr')) {
        return mb_substr($text, 0, $max, 'UTF-8');
    }
    preg_match('/^.{0,' . $max . '}/su', $text, $match);
    return $match[0] ?? '';
}

/** Valid UTF-8, line breaks normalised, control characters except tab and newline removed. */
function fb_clean_text(string $text): ?string
{
    if (!preg_match('//u', $text)) {
        return null;
    }
    $text = str_replace(["\r\n", "\r"], "\n", $text);
    return trim((string) preg_replace('/[\x00-\x08\x0b-\x1f\x7f]/', '', $text));
}

/** Single-line value: all control characters (including CR/LF) removed. */
function fb_clean_line(string $text): ?string
{
    if (!preg_match('//u', $text)) {
        return null;
    }
    return trim((string) preg_replace('/[\x00-\x1f\x7f]+/', ' ', $text));
}

/** Validates the submitted fields. Returns ['error' => code] or ['data' => fields]. */
function fb_validate(array $post): array
{
    foreach (['system', 'version', 'type', 'description', 'tested', 'email', 'consent'] as $key) {
        if (isset($post[$key]) && !is_string($post[$key])) {
            return ['error' => 'invalid'];
        }
    }
    $description = fb_clean_text($post['description'] ?? '');
    if ($description === null) {
        return ['error' => 'invalid'];
    }
    if ($description === '' || fb_length($description) > FB_MAX_DESCRIPTION) {
        return ['error' => 'description'];
    }
    $system = $post['system'] ?? '';
    $type = $post['type'] ?? '';
    $version = fb_clean_line($post['version'] ?? '');
    $tested = fb_clean_text($post['tested'] ?? '');
    if (!isset(FB_SYSTEMS[$system]) || !isset(FB_TYPES[$type]) || $version === null || $tested === null
        || fb_length($version) > FB_MAX_VERSION || fb_length($tested) > FB_MAX_TESTED) {
        return ['error' => 'invalid'];
    }
    $email = trim($post['email'] ?? '');
    if ($email !== '' && !fb_valid_email($email)) {
        return ['error' => 'email'];
    }
    if (($post['consent'] ?? '') !== 'yes') {
        return ['error' => 'consent'];
    }
    return ['data' => [
        'system' => $system,
        'version' => $version,
        'type' => $type,
        'description' => $description,
        'tested' => $tested,
        'email' => $email,
    ]];
}

function fb_ensure_dir(string $dir): bool
{
    return is_dir($dir) || (@mkdir($dir, 0700, true) && is_dir($dir));
}

/** Daily salt (UTC day). Salts of other days are deleted, so a salt lives at most 24 hours. */
function fb_daily_salt(string $saltDir, int $now): ?string
{
    if (!fb_ensure_dir($saltDir)) {
        return null;
    }
    $day = gmdate('Y-m-d', $now);
    $file = $saltDir . DIRECTORY_SEPARATOR . 'salt-' . $day;
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

/**
 * Sliding one-hour window per salted IP hash. Returns true when the submission is allowed and
 * records it. Files of other hashes older than the window are removed on the way.
 */
function fb_rate_allow(string $saltDir, string $ip, int $now): ?bool
{
    $salt = fb_daily_salt($saltDir, $now);
    if ($salt === null) {
        return null;
    }
    $hash = hash_hmac('sha256', $ip, $salt);
    $file = $saltDir . DIRECTORY_SEPARATOR . 'rate-' . $hash;
    foreach (glob($saltDir . DIRECTORY_SEPARATOR . 'rate-*') ?: [] as $other) {
        $mtime = @filemtime($other);
        if ($other !== $file && $mtime !== false && $mtime < $now - FB_RATE_WINDOW) {
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
        if (ctype_digit($line) && (int) $line > $now - FB_RATE_WINDOW && (int) $line <= $now) {
            $times[] = (int) $line;
        }
    }
    $allowed = count($times) < FB_RATE_LIMIT;
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

/** Appends one JSON line to data_dir/feedback-YYYY-MM.jsonl (UTC) under an exclusive lock, mode 0600. */
function fb_store(string $dataDir, array $record, int $now): bool
{
    if (!fb_ensure_dir($dataDir)) {
        return false;
    }
    $file = $dataDir . DIRECTORY_SEPARATOR . 'feedback-' . gmdate('Y-m', $now) . '.jsonl';
    $json = json_encode($record, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
    if ($json === false) {
        return false;
    }
    $previous = umask(0077);
    $handle = @fopen($file, 'ab');
    umask($previous);
    if ($handle === false) {
        return false;
    }
    @chmod($file, 0600);
    $ok = false;
    if (flock($handle, LOCK_EX)) {
        $ok = fwrite($handle, $json . "\n") === strlen($json) + 1 && fflush($handle);
        flock($handle, LOCK_UN);
    }
    fclose($handle);
    return $ok;
}

/** RFC 2047 encoded subject; the encoded form cannot carry header line breaks. */
function fb_subject(array $data): string
{
    $version = $data['version'] !== '' ? $data['version'] : 'unbekannt';
    $subject = '[OpenSpider Beta] ' . FB_TYPES[$data['type']] . ' – ' . FB_SYSTEMS[$data['system']] . ' – ' . $version;
    return '=?UTF-8?B?' . base64_encode($subject) . '?=';
}

/** Mail as [to, subject, body, headers]; body is quoted-printable UTF-8 text with CRLF line breaks. */
function fb_build_mail(array $config, array $record, array $data): array
{
    $lines = [
        'ID: ' . $record['id'],
        'Zeit (UTC): ' . $record['time'],
        'Sprache: ' . $record['lang'],
        'Art: ' . FB_TYPES[$data['type']],
        'System: ' . FB_SYSTEMS[$data['system']],
        'Version: ' . ($data['version'] !== '' ? $data['version'] : '–'),
        'E-Mail: ' . ($data['email'] !== '' ? $data['email'] : '–'),
        'User-Agent: ' . ($record['user_agent'] !== '' ? $record['user_agent'] : '–'),
        '',
        'Beschreibung:',
        $data['description'],
        '',
        'Was wurde getestet:',
        $data['tested'] !== '' ? $data['tested'] : '–',
    ];
    $headers = [
        'From' => $config['mail_from'],
        'MIME-Version' => '1.0',
        'Content-Type' => 'text/plain; charset=UTF-8',
        'Content-Transfer-Encoding' => 'quoted-printable',
        'X-Mailer' => 'OpenSpider feedback',
    ];
    if ($data['email'] !== '' && fb_valid_email($data['email'])) {
        $headers['Reply-To'] = $data['email'];
    }
    // quoted_printable_encode() keeps only CRLF as line breaks; the description contains LF only.
    $body = quoted_printable_encode(str_replace("\n", "\r\n", implode("\n", $lines)));
    return [$config['mail_to'], fb_subject($data), $body, $headers];
}

/** Default mailer; tests replace it via the $mailer argument of fb_handle(). */
function fb_send_mail(string $to, string $subject, string $body, array $headers): bool
{
    return mail($to, $subject, $body, $headers);
}

function fb_redirect(string $location): array
{
    return ['status' => 303, 'headers' => ['Location' => $location], 'body' => ''];
}

function fb_error_redirect(string $lang, string $code, string $version = ''): array
{
    $query = ['error' => $code];
    if ($version !== '') {
        $query['version'] = $version;
    }
    return fb_redirect(FB_PAGES[$lang]['form'] . '?' . http_build_query($query, '', '&', PHP_QUERY_RFC3986) . '#feedback-error-' . $code);
}

/**
 * Handles one request and returns ['status', 'headers', 'body'].
 * $mailer: callable(string $to, string $subject, string $body, array $headers): bool
 */
function fb_handle(array $server, array $post, array $config, callable $mailer, int $now): array
{
    if (($server['REQUEST_METHOD'] ?? '') !== 'POST') {
        return ['status' => 405, 'headers' => ['Allow' => 'POST', 'Content-Type' => 'text/plain; charset=UTF-8'], 'body' => "Method Not Allowed\n"];
    }
    $lang = is_string($post['lang'] ?? null) && isset(FB_PAGES[$post['lang']]) ? $post['lang'] : 'de';
    $length = $server['CONTENT_LENGTH'] ?? null;
    $size = is_numeric($length) ? (int) $length : strlen(http_build_query($post));
    if ($size > FB_MAX_BODY) {
        return fb_error_redirect($lang, 'too_large');
    }
    // Honeypot: bots get the normal confirmation, nothing is stored or sent.
    if (($post['website'] ?? '') !== '') {
        return fb_redirect(FB_PAGES[$lang]['thanks']);
    }
    $result = fb_validate($post);
    $version = is_string($post['version'] ?? null) ? (fb_clean_line($post['version']) ?? '') : '';
    $version = fb_length($version) <= FB_MAX_VERSION ? $version : '';
    if (isset($result['error'])) {
        return fb_error_redirect($lang, $result['error'], $version);
    }
    $data = $result['data'];
    $allowed = fb_rate_allow($config['salt_dir'], (string) ($server['REMOTE_ADDR'] ?? ''), $now);
    if ($allowed === null) {
        error_log('openspider-feedback: rate limit storage unavailable');
        return fb_error_redirect($lang, 'server', $version);
    }
    if (!$allowed) {
        return fb_error_redirect($lang, 'rate', $version);
    }
    $agent = fb_clean_line(is_string($server['HTTP_USER_AGENT'] ?? null) ? $server['HTTP_USER_AGENT'] : '') ?? '';
    $record = [
        'id' => bin2hex(random_bytes(8)),
        'time' => gmdate('Y-m-d\TH:i:s\Z', $now),
        'lang' => $lang,
        'system' => $data['system'],
        'version' => $data['version'],
        'type' => $data['type'],
        'description' => $data['description'],
        'tested' => $data['tested'],
        'email' => $data['email'],
        'user_agent' => fb_truncate($agent, FB_MAX_USER_AGENT),
    ];
    $stored = fb_store($config['data_dir'], $record, $now);
    if (!$stored) {
        error_log('openspider-feedback: storing feedback failed');
    }
    $sent = false;
    try {
        $sent = (bool) $mailer(...fb_build_mail($config, $record, $data));
    } catch (Throwable) {
        $sent = false;
    }
    if (!$sent) {
        error_log('openspider-feedback: sending notification mail failed');
    }
    if (!$stored && !$sent) {
        return fb_error_redirect($lang, 'server', $version);
    }
    return fb_redirect(FB_PAGES[$lang]['thanks']);
}

function fb_main(): void
{
    $path = getenv('OPENSPIDER_FEEDBACK_CONFIG');
    if (!is_string($path) || $path === '') {
        $path = is_string($_SERVER['OPENSPIDER_FEEDBACK_CONFIG'] ?? null) ? $_SERVER['OPENSPIDER_FEEDBACK_CONFIG'] : '';
    }
    $config = fb_load_config($path);
    if ($config === null) {
        error_log('openspider-feedback: configuration missing or invalid');
        $response = ['status' => 503, 'headers' => ['Content-Type' => 'text/plain; charset=UTF-8'], 'body' => "Service Unavailable\n"];
    } else {
        $response = fb_handle($_SERVER, $_POST, $config, 'fb_send_mail', time());
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

if (!defined('OPENSPIDER_FEEDBACK_NO_MAIN')) {
    fb_main();
}

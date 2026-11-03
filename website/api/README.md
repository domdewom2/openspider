# Beta-Feedback-Endpunkt

`feedback.php` nimmt das Formular der Beta-Seite (`/beta/#feedback`, `/en/beta/#feedback`) als
`POST /api/feedback` entgegen. PHP 8.3 (FPM), keine Abhängigkeiten, kein Drittanbieter.

## Verhalten

- Nur `POST` (sonst 405), Body höchstens 20 KB (`CONTENT_LENGTH`).
- Felder: `lang` (de/en), `system` (`macos-arm`, `macos-intel`, `windows-x64`, `windows-arm`,
  `linux-cli`), `version` (optional, ≤ 64 Zeichen), `type` (`bug`, `idea`, `praise`, `other`),
  `description` (Pflicht, ≤ 5.000 Zeichen), `tested` (optional, ≤ 1.000), `email` (optional,
  gültige Adresse), `consent=yes` (Pflicht), Honeypot `website` (muss leer sein).
- Honeypot gefüllt: Weiterleitung auf die Danke-Seite, nichts wird gespeichert oder gesendet.
- Rate-Limit: 5 Einsendungen pro Stunde je HMAC-SHA-256 der IP-Adresse mit täglich neuem Salz
  (`salt_dir/salt-YYYY-MM-DD`, `salt_dir/rate-<hash>`). Die IP-Adresse selbst wird nirgends gespeichert.
- Speicherung: eine JSON-Zeile je Rückmeldung in `data_dir/feedback-YYYY-MM.jsonl` (UTC), Felder
  `id, time, lang, system, version, type, description, tested, email, user_agent` (User-Agent auf
  200 Zeichen gekürzt), Schreiben unter `flock`, Dateirechte 0600.
- Danach Mail per `mail()` an `mail_to`, Betreff `[OpenSpider Beta] <Art> – <System> – <Version>`
  (RFC 2047), Text-Body (quoted-printable), `Reply-To` nur bei gültiger E-Mail. Ein Mailfehler
  verhindert die Speicherung nicht; nur wenn beides scheitert, gilt die Einsendung als fehlgeschlagen.
- Antwort: 303 auf `/beta/danke/` bzw. `/en/beta/thanks/`; bei Fehlern 303 auf
  `/beta/?error=<code>[&version=…]#feedback-error-<code>` (Codes: `description`, `consent`, `email`,
  `invalid`, `too_large`, `rate`, `server`). Die Seite zeigt die Meldung per CSS `:target`, auch ohne JavaScript.
- Der Fehlerlog erhält nur feste Meldungen (`openspider-feedback: …`), nie Formularinhalte.

## Konfiguration

Die Konfiguration liegt **nicht** im Repository und nicht im Web-Root. Die FPM-Umgebungsvariable
`OPENSPIDER_FEEDBACK_CONFIG` nennt den Pfad einer INI-Datei mit den Schlüsseln:

| Schlüssel | Bedeutung |
| --- | --- |
| `data_dir` | Verzeichnis der `feedback-YYYY-MM.jsonl` (wird mit 0700 angelegt) |
| `salt_dir` | Verzeichnis für Tagessalz und Rate-Limit-Dateien |
| `mail_to` | Empfänger der Benachrichtigung |
| `mail_from` | Absender (`From`) der Benachrichtigung |

Vorlage mit Platzhaltern: `feedback.example.ini`. Fehlt die Datei oder ein Schlüssel, antwortet
der Endpunkt mit 503.

## Betrieb

- Beispiel FPM-Pool: `env[OPENSPIDER_FEEDBACK_CONFIG] = /etc/openspider/feedback.ini`; `data_dir` und
  `salt_dir` gehören dem FPM-Benutzer und liegen außerhalb des Web-Roots.
- Der Webserver leitet nur `/api/feedback` an diese Datei weiter (z. B. Caddy:
  `php_fastcgi` mit festem `SCRIPT_FILENAME` auf `feedback.php`); das Verzeichnis `website/api/`
  selbst, insbesondere `tests/` und die `.ini`-Vorlage, wird nicht ausgeliefert.
- Der MTA muss lokale Mails über `sendmail` annehmen (`sendmail_path` in PHP).
- Salz und Rate-Dateien werden bei Anfragen bereinigt. Damit sie auch ohne Anfragen nach spätestens
  24 Stunden verschwinden (Datenschutzerklärung), stündlich per Timer/Cron alle Dateien in `salt_dir`
  löschen, die älter als 1440 Minuten sind.
- Löschfrist der Rückmeldungen: spätestens 12 Monate nach Ende der Beta (Datenschutzerklärung).

## Tests

```sh
php -l website/api/feedback.php
php website/api/tests/feedback_test.php
```

Die Tests ersetzen `mail()` durch eine Stub-Funktion (Argument `$mailer` von `fb_handle()`) und
arbeiten in einem temporären Verzeichnis.

# Absturzberichte (`crash.php`)

`crash.php` nimmt Absturzberichte der Desktop-App als `POST /api/crash` entgegen (OUT-247). Die App sendet
nur, nachdem jemand im Hinweis bzw. unter Hilfe → „Absturzberichte…“ auf „Senden“ geklickt hat; nie automatisch.
PHP 8.3 (FPM), keine Abhängigkeiten, kein Drittanbieter. Es gelten dieselben Betriebsregeln wie für `feedback.php`.

## Verhalten

- Nur `POST` (sonst 405) mit `Content-Type: application/json`; Body höchstens 80 KB (sonst `413 too_large`).
- Körper genau `{"report": <Bericht>, "note": "<Text>", "email": "<Adresse>"}`. Der Bericht muss genau die
  19 Felder aus OUT-246 mit den richtigen Typen haben (`format` = 1, `id` UUID v4, `kind` ∈ `panic`/`frontend`/
  `unclean_exit`, `process` ∈ `app`/`cli`, Listen `backtrace` ≤ 64 und `log_tail` ≤ 200 Einträge, `message`
  ≤ 500 Zeichen). `note` höchstens 2.000 Zeichen, `email` leer oder gültig (≤ 254). Sonst `400 invalid`.
- Rate-Limit: 10 Berichte pro Stunde je HMAC-SHA-256 der IP-Adresse mit täglich neuem Salz
  (`salt_dir/salt-YYYY-MM-DD`, `salt_dir/rate-<hash>`), sonst `429 rate`. Die IP-Adresse selbst wird nirgends
  gespeichert, auch nicht im Fehlerlog.
- Speicherung: eine Datei je Bericht `data_dir/crash-YYYYMMDD-HHMMSS-<ref>.json` (UTC, Rechte 0600) mit
  `ref`, `received`, `note`, `email`, `report`.
- Danach Mail per `mail()` an `mail_to`, Betreff `[OpenSpider Absturz] <app_version> <os>/<arch> <kind>: <message>`
  (Meldung auf 80 Zeichen gekürzt, RFC 2047), Text-Body (quoted-printable) mit Referenz, Notiz und Bericht;
  `Reply-To` nur bei gültiger E-Mail. Scheitern Speicherung **und** Mail, antwortet der Endpunkt `500 server`.
- Antwort `200 {"ok":true,"ref":"<8 Zeichen Base32>"}`; die Referenz zeigt die App an („Danke! Referenz: …“),
  Nutzer können sie in einem GitHub-Issue angeben. Fehler: `{"ok":false,"error":"invalid"|"too_large"|"rate"|"server"}`.
- Der Fehlerlog erhält nur feste Meldungen (`openspider-crash: …`).

## Konfiguration

Wie beim Feedback, aber eigene Datei: Die FPM-Umgebungsvariable `OPENSPIDER_CRASH_CONFIG` nennt eine INI-Datei mit
`data_dir`, `salt_dir`, `mail_to`, `mail_from` (Vorlage `crash.example.ini`). Fehlt die Datei oder ein Schlüssel,
antwortet der Endpunkt mit 503.

## Betrieb

- Beispiel FPM-Pool: `env[OPENSPIDER_CRASH_CONFIG] = /etc/openspider/crash.ini`; `data_dir` und `salt_dir` gehören
  dem FPM-Benutzer und liegen außerhalb des Web-Roots (eigene Ordner, getrennt vom Feedback).
- Der Webserver leitet nur `/api/crash` an diese Datei weiter (fester `SCRIPT_FILENAME` auf `crash.php`).
- Salz und Rate-Dateien wie beim Feedback stündlich bereinigen (älter als 1440 Minuten).
- Löschfrist: Berichte in `data_dir` nach 90 Tagen löschen (Datenschutzerklärung), z. B. täglich per Timer/Cron
  alle `crash-*.json` älter als 90 Tage; die Benachrichtigungsmails im Postfach ebenso.

## Tests

```sh
php -l website/api/crash.php
php website/api/tests/crash_test.php
```

Die Tests verwenden die Bericht-Fixture `tests/fixtures/crash/crash-panic.json`, ersetzen `mail()` durch eine
Stub-Funktion (Argument `$mailer` von `cr_handle()`) und leiten den Fehlerlog in eine temporäre Datei um.

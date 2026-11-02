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

# OpenSpider – Regeln für Agents

- **Clean-Room:** Nur `docs/spec/*` beschreibt, was gebaut wird. Implementierende Agents schauen sich Screaming Frog (App, Bytecode, Ressourcen) NICHT an und kopieren keine Texte daraus. Unklarheiten in der Spec → im Ergebnis melden, nicht raten.
- Architektur und Crate-Zuständigkeiten: `docs/ARCHITECTURE.md`. Neue Crates nur nach Absprache mit dem Orchestrator.
- Jede umgesetzte Funktion referenziert ihre Spec-ID im Test (`// spec: ONPAGE-012`) und hat mindestens einen Test gegen eine Fixture.
- Vor Abgabe muss grün sein: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`. Für die App zusätzlich `npm run -w apps/desktop typecheck && npm run -w apps/desktop test`.
- Abhängigkeiten nur mit Apache-2.0/MIT/BSD/ISC/Zlib/MPL-2.0-kompatibler Lizenz.
- Plattformneutral bleiben: keine hartkodierten Pfadtrenner, `dirs`-Crate für Benutzerpfade, kein Shell-Aufruf.
- Commits: klein, Präfix mit Bereich (`fetch:`, `parse:`, `ui:` …).

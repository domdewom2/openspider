# fix-int15b – Integrationskorrektur Runde 14b (zweite Runde)

Orchestrator-Fixpaket ohne eigenen Eintrag in `M2-packages.json` (wie fix-int15). Inhalt, Ursachen und Gate-Ergebnisse stehen im Abschnitt „Integrationskorrektur int-15 (zweite Runde)“ in `docs/plan/M2-reports/fix-n02.md`; der Orchestrator hatte diesen Ort vorgegeben.

- Astra-Erstprüfung (29.10.2026): needs_work 72. Befunde 1–3 betreffen den Prozess (Paketeintrag, Berichtsort, CI-Nachweise); die CI-Matrix des Integrationszweigs liefert die Plattformnachweise.
- Befund 4 (Zeittest mit realer Zeit in `apps/desktop/src/features/export/export.test.tsx`) behoben: feste Uhr mit `vi.useFakeTimers` und gezieltem Fortschalten statt realem Warten.
- Testablaufänderungen in `crates/os-app/tests/ops_cancel.rs` und `apps/desktop/src/ui/table/table.test.tsx`: Freigabe durch den Orchestrator nach K2-63 (gleiche Absicht – genau einmalige Zustellung –, nur unabhängig vom Flush-Zeitpunkt geprüft).

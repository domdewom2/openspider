Wort- und Link-Fixture für UI-097/UI-100 (M3-V02; UI-099 nutzt dieselbe Zielseite). Die Zielseite `/target.html`
hat 30 eingehende Hyperlinks von 23 Quellseiten: Start-URL (Navigation + Inhalt), `/p1` … `/p20` (Inhalt, `/p16` …
`/p20` mit `rel="nofollow"`), `/p1` … `/p6` zusätzlich im Footer, `/p21` nur im Footer und ein Selbstlink der Zielseite
im Inhalt. Eindeutige Zahlen je
Option (nofollow, Navigation/Footer) stehen in `expected.json`. Die Zielseite selbst verlinkt sieben interne Ziele
(Header, Navigation, Inhalt mit Selbstlink, Footer) und hat sichtbaren Body-Text mit 41 Wörtern außerhalb von
Header/Navigation/Footer (Wortzahl nach ONPAGE-141 mit Standard-Inhaltsbereich); die erwarteten Begriffe der
Body-Wortwolke mit Standardoptionen (Stoppwörter en, Mindestlänge 3, max. 100) stehen ebenfalls dort. `/deep-1.html` und
`/deep-2.html` werden nur von der Zielseite entdeckt (ihre Kinder im Crawl-Baum). Alle Abrufe bleiben lokal.

Verzeichnis-Fixture für UI-093/UI-094 (M3-V02): sieben interne URLs mit zwei Verzeichnissen ohne Indexseite
(`/docs/`, `/blog/`), einem Verzeichnis mit Indexseite und abschließendem Slash (`/about/`) und einer Query-Variante
(`/blog/post?page=2`), die als Kind ihrer Seite erscheint. Erwarteter Verzeichnisbaum in `expected.json`: Schema-Knoten
(ohne URL) → Host (= Start-URL) → Pfadsegmente; hohl sind Schema, `docs` und `blog`. Alle Abrufe bleiben auf dem lokalen
Fixture-Server.

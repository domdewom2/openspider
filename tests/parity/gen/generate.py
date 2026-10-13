#!/usr/bin/env python3
"""Erzeugt die Referenz-Fixtures fuer den Paritaetsabgleich (M1-P10).

    python3 tests/parity/gen/generate.py

Schreibt nach tests/fixtures/{out-basic-site,parity-*}/ und tests/parity/coverage/*.toml.
Deterministisch: ein zweiter Lauf aendert keine Datei. Nicht mehr erzeugte Dateien eines frueheren
Laufs werden entfernt (Liste in tests/parity/gen/generated.txt). Manifeste, Mappings und README
sind von Hand gepflegt und werden hier nicht angefasst.
"""

import sys
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))

import lib  # noqa: E402
import site_basic  # noqa: E402
import site_onpage  # noqa: E402
import site_response  # noqa: E402
import site_technical  # noqa: E402

SYNTHETIC = """\
# parity-synthetic (Port 8905): entspricht synthetic:n=400,links=10,seed=42 (Kurzform) und ergaenzt die
# Anteile, die die Kurzform nicht setzen kann (301/404/5xx, noindex, Canonical auf andere URL,
# Duplikat-Titel), damit Struktur, Tiefen, Duplikat-Titel, noindex und Canonical vorkommen (M1.md 5.1).
# Die Site wird on-the-fly aus (seed, Pfad) berechnet; es gibt keine Dateien neben dieser.
[synthetic]
pages = 400
links_per_page = 10
seed = 42
status_301_share = 0.02
status_404_share = 0.02
status_5xx_share = 0.01
noindex_share = 0.05
canonical_other_share = 0.03
duplicate_title_share = 0.10
"""


def main() -> None:
    written: list[str] = []
    for mod in (site_basic, site_onpage, site_technical, site_response):
        written += lib.write_site(mod.build())
    synth = lib.FIXTURES / "parity-synthetic" / "scenario.toml"
    synth.parent.mkdir(parents=True, exist_ok=True)
    synth.write_text(SYNTHETIC, encoding="utf-8")
    written.append(str(synth.relative_to(lib.ROOT)))

    previous = set()
    if lib.GENERATED_LIST.exists():
        previous = set(lib.GENERATED_LIST.read_text(encoding="utf-8").split())
    for rel in sorted(previous - set(written)):
        (lib.ROOT / rel).unlink(missing_ok=True)
    lib.GENERATED_LIST.write_text("\n".join(sorted(written)) + "\n", encoding="utf-8")
    print(f"{len(written)} Dateien geschrieben")


if __name__ == "__main__":
    main()

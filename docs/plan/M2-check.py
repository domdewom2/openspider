#!/usr/bin/env python3
"""Prüfskript für den M2-Plan (M2.md Abschnitt 4.1 und Anhang B).

Aufruf aus der Repo-Wurzel:  python3 -I docs/plan/M2-check.py
Exit 0 = alle Prüfungen bestanden, 1 = Befunde (Liste auf stdout).

Prüft:
  P1  Paket-IDs eindeutig, Pflichtfelder vorhanden, Wellen bekannt.
  P2  depends_on zeigt nur auf frühere Wellen (M1-Pakete ausgenommen) – keine Abhängigkeit innerhalb einer Welle.
  P3  spec_ids: jede ID genau einmal, Anzahl = ERWARTETE_SPEC_IDS.
  P4  owns: keine Überschneidung innerhalb einer Welle (Heuristik über Glob-Stichproben; „ohne …“-Ausschlüsse werden beachtet).
  P5  Alle IDs aus M1.md 7.3 stehen in spec_ids/extra_spec_ids eines M2-Pakets oder in NICHT_M2 (= M2.md Abschnitt 9).
  P6  Alle P0-Einträge mit [P1]-Kriterien (01, 02, 03, 05, 06, 10) stehen in spec_ids/extra_spec_ids oder in NICHT_M2.
  P7  C9: Jeder Platzhalter-Ort einer Welle hat in der Folgewelle genau einen Besitzer (N01 bzw. N02).
  P8  Pakete mit extra_spec_ids tragen den B2-Hinweis „je [P1]-/7.3-Kriterium ein getaggter Test“ in prompt_notes.
  P9  Jede Testdatei aus M2.md 5.1 (zurückgestellte M1-Tests) gehört dem dort genannten Paket (owns).
  P10 C14 M1-Sperrbereich: Pakete der Wellen in SPERR_WELLEN besitzen nichts unter crates/{os-store,os-engine,os-analyze};
      jedes andere Paket mit Besitz dort hängt (transitiv) an M1_SPERRE_BIS.
  P11 C1: N-Pakete (C9) hängen an allen Paketen ihrer Quellwelle.
  P12 K2-75: Nutzer-Abnahmen ("human": true) laufen lokal (runner local-mac), tragen keine spec_ids und
      M2-Q04 hängt (transitiv) an jeder von ihnen.

Das Skript ist Teil der Planpflege: Wer M2-packages.json ändert, führt es vor dem Commit aus.
"""
import fnmatch
import itertools
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLAN = ROOT / "docs" / "plan"
SPEC = ROOT / "docs" / "spec"

ERWARTETE_SPEC_IDS = 590  # 19.10.: +AGENT-016 … 019 (M03); Rev. 3: +CRAWL-145 … 147 (E03), +OUT-233/234 (F02), +OUT-273 (V01)

# C14: Bis zum Merge von M1_SPERRE_BIS kein M2-Besitz in diesen Crates.
M1_SPERRE = ("crates/os-store", "crates/os-engine", "crates/os-analyze")
M1_SPERRE_BIS = "M1-N27"
SPERR_WELLEN = ["0a"]
SPERR_PROBEN = ("Cargo.toml", "src/lib.rs", "src/q.rs", "src/q/z.rs", "tests/q.rs", "migrations/q.sql", "benches/q.rs")
WELLEN = ["0a", "0b", "0c", "0d", "0e", "1", "2", "2b", "3", "3h", "3f", "4a", "4", "4h", "4n", "4b", "5"]  # 2b: M2-N01b (K2-68); 4a: M2-E07 vor Q03 (K2-74), M2-U34 vor U32/U33 (K2-75); 3h/4h: Nutzer-Abnahmen A02/A03, 4n: N-Paket aus A03 vor Q04 (K2-75); 3f: M2-N03 Funktionsfehler aus A02 vor U34 (K2-75a)
ID_RE = r"\b(?:CRAWL|ONPAGE|TECH|OUT|AGENT|UI|INTEG|PERF)-\d{3}\b"

# M2.md Abschnitt 9: bewusst nicht in M2 (Grund in Klammern). Wird von P5/P6 als Ausnahme gelesen.
NICHT_M2 = {
    "CRAWL-012": "Nicht-ASCII-Reason-Phrasen im Original, kein Ziel (K-39)",
    "CRAWL-025": "P2 (IPv6)",
    "CRAWL-049": "P2 (AMP)",
    "CRAWL-054": "P2 (Crawl Invalid Links); ONPAGE-054-Test erhält Grund P2:",
    "OUT-002": "P2-Kriterium (Trennzeichen Semikolon/Tab)",
    "OUT-003": "M3 (manuelle Office-Prüfung je Release)",
    "OUT-188": "P2-Kriterium (--gui)",
    "OUT-225": "M3 (manuelle Schlüsselbund-Prüfung je Release)",
    "OUT-261": "M3 (Installer)",
}

# Gewollte Überschneidungen innerhalb einer Welle (Paar → Grund).
ERLAUBT = {
    frozenset({"M2-U31", "M2-U32"}): "e2e-b/** ohne perf (U32) vs. e2e-b/perf/** (U31)",
}

B2_HINWEIS = "B2: je [P1]-/7.3-Kriterium"

# C9: Platzhalter-Orte je Welle und ihr Auflöser in der Folgewelle.
C9 = {
    "M2-N01": {"quelle": "1", "orte": [
        "crates/*/tests/pending_w1_*.rs",
        "apps/desktop/e2e/{table,detail,sidebar,header,statusbar,menu,keyboard,shell,export}/**",
    ]},
    "M2-N02": {"quelle": "2", "orte": [
        "apps/desktop/e2e/{config,settings-sections,crawls,table-advanced,detail-fach,sidebar-fach,viz,mcp,integrations,tasks}/**",
    ]},
}


def expand_braces(p):
    m = re.search(r"\{([^{}]*)\}", p)
    if not m:
        return [p]
    out = []
    for alt in m.group(1).split(","):
        out.extend(expand_braces(p[: m.start()] + alt.strip() + p[m.end():]))
    return out


def parse_owns(entry):
    """Liefert (Muster[], Ausschlüsse[]) für einen owns-Eintrag."""
    text = entry
    excl = []
    for paren in re.findall(r"\(([^()]*)\)", text):
        if "ohne" in paren:
            for part in re.split(r";|,(?![^{]*\})|\s+und\s+", paren):
                part = part.strip()
                part = re.sub(r"^ohne\s+", "", part)
                if part and ("/" in part or "." in part or "*" in part):
                    excl.append(part.split()[0])
    text = re.sub(r"\([^()]*\)", "", text)
    pats = []
    for chunk in re.split(r"\s+(?:→|und|bzw\.)\s+|,\s+(?![^{]*\})", text):
        chunk = chunk.strip()
        if not chunk or " " in chunk or "/" not in chunk and "." not in chunk:
            continue
        chunk = re.sub(r"<[^>]*>", "*", chunk)
        pats.extend(expand_braces(chunk))
    return pats, excl


def to_regex(glob):
    i, out = 0, ""
    while i < len(glob):
        if glob.startswith("**", i):
            out += ".*"
            i += 2
        elif glob[i] == "*":
            out += "[^/]*"
            i += 1
        else:
            out += re.escape(glob[i])
            i += 1
    return re.compile("^" + out + "$")


def sample(glob):
    return glob.replace("**", "q/z").replace("*", "q")


def base_dir(glob):
    return glob.split("*")[0].rsplit("/", 1)[0]


def excluded(path, pat, excl):
    b = base_dir(pat)
    for e in excl:
        for e2 in expand_braces(e):
            for cand in (e2, f"{b}/{e2}"):
                if to_regex(cand).match(path) or path.startswith(cand.rstrip("*").rstrip("/") + "/"):
                    return True
    return False


def overlap(a_entries, b_entries):
    for ea in a_entries:
        pa, xa = parse_owns(ea)
        for eb in b_entries:
            pb, xb = parse_owns(eb)
            for ga, gb in itertools.product(pa, pb):
                ra, rb = to_regex(ga), to_regex(gb)
                for s, r, own_pat, own_x, other_pat, other_x in (
                    (sample(ga), rb, ga, xa, gb, xb),
                    (sample(gb), ra, gb, xb, ga, xa),
                ):
                    if r.match(s) and not excluded(s, other_pat, other_x) and not excluded(s, own_pat, own_x):
                        return f"{ea}  ×  {eb}"
    return None


def ids_m1_73():
    text = (PLAN / "M1.md").read_text(encoding="utf-8")
    start = text.index("### 7.3 ")
    end = text.index("\n## 8 ", start)
    return sorted(set(re.findall(ID_RE, text[start:end])))


def ids_with_p1_criteria():
    res = []
    for f in sorted(SPEC.glob("*.md")):
        if not re.match(r"^(0[1-6]|10)-", f.name):
            continue
        t = f.read_text(encoding="utf-8")
        parts = re.split(r"^(#{2,4} +[A-Z]+-\d{3}\b.*)$", t, flags=re.M)
        for i in range(1, len(parts), 2):
            sid = re.search(r"[A-Z]+-\d{3}", parts[i]).group(0)
            if "[P1]" in parts[i + 1]:
                res.append(sid)
    return sorted(set(res))


def main():
    pk = json.loads((PLAN / "M2-packages.json").read_text(encoding="utf-8"))
    errs = []
    byid = {}
    for p in pk:
        p["wave"] = str(p.get("wave"))
    for p in pk:
        for k in ("id", "title", "wave", "spec_ids", "owns", "depends_on", "size", "prompt_notes"):
            if k not in p:
                errs.append(f"P1 {p.get('id')}: Feld {k} fehlt")
        if p["id"] in byid:
            errs.append(f"P1 doppelte ID {p['id']}")
        if p["wave"] not in WELLEN:
            errs.append(f"P1 {p['id']}: unbekannte Welle {p['wave']}")
        byid[p["id"]] = p
    for p in pk:
        for d in p["depends_on"]:
            if d.startswith("M1-"):
                continue
            if d not in byid:
                errs.append(f"P2 {p['id']}: unbekannte Abhängigkeit {d}")
            elif WELLEN.index(byid[d]["wave"]) >= WELLEN.index(p["wave"]):
                errs.append(f"P2 {p['id']} (W{p['wave']}) hängt an {d} (W{byid[d]['wave']})")
    seen = {}
    for p in pk:
        for s in p["spec_ids"]:
            if s in seen:
                errs.append(f"P3 {s} doppelt: {seen[s]} und {p['id']}")
            seen[s] = p["id"]
    if len(seen) != ERWARTETE_SPEC_IDS:
        errs.append(f"P3 {len(seen)} spec_ids statt {ERWARTETE_SPEC_IDS}")
    for w in WELLEN:
        wp = [p for p in pk if p["wave"] == w and p.get("kind", "package") == "package"]
        for a, b in itertools.combinations(wp, 2):
            if frozenset({a["id"], b["id"]}) in ERLAUBT:
                continue
            hit = overlap(a["owns"], b["owns"])
            if hit:
                errs.append(f"P4 Welle {w}: {a['id']} / {b['id']}: {hit}")
    cov = {}
    for p in pk:
        for s in p["spec_ids"] + p.get("extra_spec_ids", []):
            cov.setdefault(s, []).append(p["id"])
    for s in ids_m1_73():
        if s not in cov and s not in NICHT_M2:
            errs.append(f"P5 {s} aus M1.md 7.3 ohne M2-Paket und ohne Eintrag in Abschnitt 9")
    for s in ids_with_p1_criteria():
        if s not in cov and s not in NICHT_M2:
            errs.append(f"P6 {s} hat [P1]-Kriterien, steht aber in keinem Paket")
    for nid, c in C9.items():
        if nid not in byid:
            errs.append(f"P7 {nid} fehlt")
            continue
        owns = " | ".join(byid[nid]["owns"])
        for o in c["orte"]:
            if o not in owns:
                errs.append(f"P7 {nid} besitzt Platzhalter-Ort {o} nicht")
    for p in pk:
        if p.get("extra_spec_ids") and B2_HINWEIS not in p["prompt_notes"]:
            errs.append(f"P8 {p['id']}: B2-Hinweis fehlt in prompt_notes")
    def sperr_treffer(entries):
        for e in entries:
            pats, x = parse_owns(e)
            for gl in pats:
                if any(sample(gl).startswith(c + "/") for c in M1_SPERRE) and not excluded(sample(gl), gl, x):
                    return e
                for c in M1_SPERRE:
                    for probe in SPERR_PROBEN:
                        path = f"{c}/{probe}"
                        if to_regex(gl).match(path) and not excluded(path, gl, x):
                            return e
        return None

    def alle_deps(pid, acc=None):
        acc = set() if acc is None else acc
        for d in byid.get(pid, {"depends_on": []})["depends_on"]:
            if d not in acc:
                acc.add(d)
                alle_deps(d, acc)
        return acc

    for p in pk:
        hit = sperr_treffer(p["owns"])
        if not hit:
            continue
        if p["wave"] in SPERR_WELLEN:
            errs.append(f"P10 {p['id']} (W{p['wave']}) besitzt im M1-Sperrbereich: {hit}")
        elif M1_SPERRE_BIS not in alle_deps(p["id"]):
            errs.append(f"P10 {p['id']} besitzt im M1-Sperrbereich ({hit}), hängt aber nicht an {M1_SPERRE_BIS}")
    for nid, c in C9.items():
        if nid not in byid:
            continue
        fehlt = [p["id"] for p in pk if p["wave"] == c["quelle"] and p["id"] != nid and p["id"] not in byid[nid]["depends_on"]]
        if fehlt:
            errs.append(f"P11 {nid} hängt nicht an allen Paketen der Welle {c['quelle']}: {', '.join(fehlt)}")
    q04 = alle_deps("M2-Q04") if "M2-Q04" in byid else set()
    for p in pk:
        if not p.get("human"):
            continue
        if p.get("runner") != "local-mac":
            errs.append(f"P12 {p['id']}: Nutzer-Abnahme ohne runner local-mac")
        if p["spec_ids"]:
            errs.append(f"P12 {p['id']}: Nutzer-Abnahme mit spec_ids")
        if p["id"] not in q04:
            errs.append(f"P12 M2-Q04 hängt nicht an der Nutzer-Abnahme {p['id']}")
    m2md = (PLAN / "M2.md").read_text(encoding="utf-8")
    sec = m2md[m2md.index("### 5.1 "): m2md.index("### 5.2 ")]
    for line in sec.splitlines():
        cells = [x.strip() for x in line.strip().strip("|").split("|")]
        if len(cells) < 4 or not cells[0].startswith("`os-"):
            continue
        path = "crates/" + cells[0].strip("`").split("`")[0]
        for pid in re.findall(r"\b([A-Z]\d{2}B?) \(W", " ".join(cells[2:])):
            ok = False
            for e in byid.get("M2-" + pid, {"owns": []})["owns"]:
                pats, x = parse_owns(e)
                if any(to_regex(gl).match(path) and not excluded(path, gl, x) for gl in pats):
                    ok = True
            if not ok:
                errs.append(f"P9 {path}: M2-{pid} besitzt die Datei nicht (M2.md 5.1)")
    n73 = len(ids_m1_73())
    np1 = len(ids_with_p1_criteria())
    print(f"Pakete {len(pk)}, spec_ids {len(seen)}, M1-7.3-IDs {n73}, P0/P1-Einträge mit [P1]-Kriterien {np1}")
    if errs:
        print("\n".join(errs))
        return 1
    print("OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())

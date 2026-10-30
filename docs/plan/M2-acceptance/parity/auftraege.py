#!/usr/bin/env python3
"""Rebuild the grouped appendix from the classified findings (preserves editorial orders)."""
import collections
import csv
from pathlib import Path

out=Path(__file__).resolve().parent
p=out/'nachbesserung.md'
prefix=p.read_text().split('## Einzelaufträge nach Crate')[0]
rows=list(csv.DictReader((out/'befunde.csv').open()))
groups=collections.defaultdict(list)
for r in rows: groups[(r['Crate'],r['Kategorie'],r['Spalte'],r['Spec-ID'])].append(r)
def cell(value): return str(value).replace('|','\\|').replace('\n',' ')[:300]
lines=['## Einzelaufträge nach Crate','']
last=None
for (crate,kind,col,spec),rs in sorted(groups.items()):
    if crate!=last:
        lines+=['### '+crate,'','| Kategorie / Schwere / Gate | Spalte · Spec | Datei | Umfang | Beispiel-URL / Schlüssel | Erwartet → gemessen |','| --- | --- | --- | --- | --- | --- |']
        last=crate
    r=rs[0]
    high=kind in ['Implementierungsfehler','Mapping-/Formatfehler']
    scope=len({(v['Site'],v['Export']) for v in rs})
    values=[kind+' / '+('Hoch / ja' if high else 'Mittel / nein'), col+' · '+spec,r['Datei'],
            str(len(rs))+' Befunde in '+str(scope)+' Exporten',r['Site']+' '+r['URL/Schluessel'],
            repr(r['Referenz'])+' → '+repr(r['OpenSpider'])]
    lines.append('| '+' | '.join(map(cell,values))+' |')
p.write_text(prefix+'\n'.join(lines)+'\n')
print('Gruppen:',len(groups))

#!/usr/bin/env python3
"""Evaluate os-parity reports only; never opens reference exports."""
import collections
import csv
import json
from pathlib import Path
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
source = Path(sys.argv[1])
summary = json.loads(source.read_text())
filters = []
for name in ['M1', 'M2']:
    for tab in tomllib.loads((ROOT / f'docs/plan/{name}-filters.toml').read_text())['tab']:
        filters.extend((tab['id'], f) for f in tab['filters'])
by_filter = {(tab.lower(), f.get('default_label', f['id']).lower()): f['spec'] for tab, f in filters}
by_tab = collections.defaultdict(set)
for tab, f in filters:
    by_tab[tab].add(f['spec'])


def spec_for(e, column='', key=''):
    if column == 'URLs' and ' | ' in key:
        tab, item = key.split(' | ', 1)
        return by_filter.get((tab.lower(), item.lower()), 'OUT-080')
    for token, spec in [('Link Score', 'TECH-145'), ('Inlinks', 'TECH-130'),
                        ('Outlinks', 'TECH-133'), ('Pixel Width', 'ONPAGE-011'),
                        ('Word Count', 'ONPAGE-141'), ('Text Ratio', 'ONPAGE-013'),
                        ('Sentence', 'ONPAGE-149'), ('Reading', 'ONPAGE-149'),
                        ('Readability', 'ONPAGE-149'), ('Lastmod', 'OUT-123'),
                        ('Changefreq', 'OUT-125'), ('Indexability', 'TECH-001'),
                        ('Canonical', 'TECH-010'), ('hreflang', 'TECH-071')]:
        if token.lower() in column.lower():
            return spec
    parts = e['os'].split(':')
    if parts[0] == 'tab':
        tab = parts[1]
        if len(parts) > 2 and (tab.lower(), parts[2].lower()) in by_filter:
            return by_filter[(tab.lower(), parts[2].lower())]
        return ', '.join(sorted(by_tab.get(tab, {'OUT-050'})))
    return 'OUT-085' if e['mapping']=='list_chains' else 'OUT-083' if 'redirect_canonical_chain' in e['mapping'] else 'OUT-086' if 'canonical_chain' in e['mapping'] else 'OUT-082' if 'chain' in e['mapping'] else 'OUT-031' if e['os'].startswith('bulk:') else 'OUT-098' if 'serp' in e['mapping'] else 'OUT-080'


def classify(e, column, ref, measured, key=''):
    # N06: entschiedene SQ-02…13 sind Produktaufträge. Dokumentierte DEV sind
    # bereits im Runner angewandt; unentschuldigte Werte werden nicht nachträglich grün.
    if column == 'Nicht verglichen':
        return 'Dokumentierter Skip', 'os-parity', 'tests/golden/parity/*/manifest.toml (Grund im Laufjournal)'
    if column == 'Exportfehler' and e['mapping'] == 'content_soft404':
        return 'Implementierungsfehler', 'os-analyze', 'N08: crates/os-analyze/src/tabs/content.rs (URL-Schlüssel fehlt)'
    if column == 'Mapping':
        if key in e.get('_missing_os', []) and e['mapping'].startswith('content'):
            return 'Implementierungsfehler', 'os-analyze', 'N08: crates/os-analyze/src/tabs/content.rs; crates/os-export/src/pipeline.rs'
        return 'Mapping-/Formatfehler', 'os-parity', 'tests/parity/mappings/' + e['mapping'] + '.toml'
    if column == 'Exportfehler':
        return 'Mapping-/Formatfehler', 'os-parity', 'crates/os-parity/src/run.rs; tests/golden/parity/*/manifest.toml'
    if 'sitemap' in e['mapping']:
        return 'Implementierungsfehler', 'os-export', 'N11 / G22: crates/os-export/src/sitemap_gen/'
    if 'chain' in e['mapping']:
        return 'Implementierungsfehler', 'os-export', 'N11 / G23: crates/os-export/src/reports/'
    if column == 'Text Ratio' or any(t in column for t in ['Word Count', 'Reading', 'Readability', 'Sentence']):
        return 'Implementierungsfehler', 'os-parse', 'N08 / G06–08/G20: crates/os-parse/src/; crates/os-analyze/src/tabs/content.rs'
    if 'Pixel Width' in column:
        return 'Implementierungsfehler', 'os-parse', 'N07 / G21: crates/os-parse/src/pixel/'
    if column == 'Zeilenmenge' and e['mapping'] == 'crawl_overview':
        return 'Implementierungsfehler', 'os-export', 'N11 / G25: crates/os-export/src/reports/overview.rs'
    if column == 'URLs' and key.startswith('security |'):
        return 'Implementierungsfehler', 'os-analyze', 'N10 / G10/G14: crates/os-analyze/src/tabs/security.rs'
    if column == 'URLs' and key.startswith('internal |'):
        return 'Implementierungsfehler', 'os-analyze', 'N07 / G12: crates/os-analyze/src/tabs/internal.rs'
    if column == 'URLs' and key.startswith('content |'):
        return 'Implementierungsfehler', 'os-parse', 'N08: crates/os-parse/src/readability.rs'
    if e['mapping'].startswith(('canonicals', 'hreflang', 'pagination', 'validation', 'directives', 'response_codes')):
        return 'Implementierungsfehler', 'os-analyze', 'N09 / G14–17: crates/os-analyze/src/tabs/; crates/os-parse/src/'
    if e['os'].startswith('bulk:'):
        return 'Implementierungsfehler', 'os-export', 'N07/N09: Linkposition und Linktypen gemäß Triage G17/G24'
    return 'Implementierungsfehler', 'os-analyze', 'N07–N10: Zuordnung nach triage.md G09–G21'

rows = []
accepted = []
exports = []
site_counts = []
metadata = []
columns = []
for site in summary['sites']:
    counts = collections.Counter()
    for e in site['exports']:
        counts[e['outcome']] += 1
        r = e.get('report')
        if r:
            e['_missing_os'] = r['missing_os_columns']
            for d in r['applied_deviations']:
                accepted.append([site['site'], e['ref_file'], d['id'], d['spec'], d['count'], d['scope'], d['note'], json.dumps(d['keys'], ensure_ascii=False)])
            counts['passed' if r['passed'] else 'failed'] += 1
            exports.append([site['site'], e['ref_file'], e['mapping'], r['ref_rows'], r['os_rows'],
                            r['agreement_pct'], r['threshold_pct'], r['url_set_identical'],
                            r.get('row_order_identical', ''), r['passed'], r['cells_compared'],
                            r['cells_compared']-r['cells_equal'], e['status'], e['outcome'], ''])
            for c in r['columns']:
                columns.append([site['site'], e['ref_file'], c['ref_column'], c['os_column'], c['compared'], c['errors']])
                if c['errors']:
                    # max_examples=100000 in the run: every differing cell is retained.
                    for x in c['examples']:
                        rows.append([site['site'], e['ref_file'], e['mapping'], c['ref_column'],
                                     x['key'], x['ref'], x['os'], spec_for(e, c['ref_column'], x['key']),
                                     *classify(e, c['ref_column'], x['ref'], x['os'], x['key'])])
            for direction in ['missing_in_os', 'extra_in_os']:
                for key in r[direction]:
                    rows.append([site['site'], e['ref_file'], e['mapping'], 'Zeilenmenge', key,
                                 'vorhanden' if direction == 'missing_in_os' else 'fehlt',
                                 'fehlt' if direction == 'missing_in_os' else 'vorhanden', spec_for(e, 'URLs', key),
                                 *classify(e, 'Zeilenmenge', '', '', key)])
            if r.get('row_order_identical') is False:
                rows.append([site['site'], e['ref_file'], e['mapping'], 'Original Url / Reihenfolge', '*',
                             'Upload-Reihenfolge', 'abweichend', 'OUT-015, OUT-085',
                             *classify(e, 'Original Url / Reihenfolge', '', '')])
            for field in ['missing_ref_columns', 'missing_os_columns', 'unmapped_ref_columns']:
                for col in r[field]:
                    rows.append([site['site'], e['ref_file'], e['mapping'], 'Mapping', col,
                                 field, 'nicht vollstaendig verglichen', spec_for(e, col),
                                 *classify(e, 'Mapping', '', '', col)])
            metadata.append(dict(site=site['site'], ref_file=e['ref_file'], mapping=e['mapping'],
                                 outcome='compared', agreement_pct=r['agreement_pct'], threshold_pct=r['threshold_pct'],
                                 passed=r['passed'], url_set_identical=r['url_set_identical'],
                                 compared_columns=len(r['columns']), missing_ref_columns=r['missing_ref_columns'],
                                 missing_os_columns=r['missing_os_columns'], unmapped_ref_columns=r['unmapped_ref_columns']))
        else:
            column = 'Exportfehler' if e['outcome']=='error' else 'Nicht verglichen'
            reason = e.get('message', e.get('reason', ''))
            exports.append([site['site'], e['ref_file'], e['mapping']] + [''] * 9 + [e['status'], e['outcome'], reason])
            rows.append([site['site'], e['ref_file'], e['mapping'], column, '*', e['status'], reason,
                         spec_for(e), *classify(e, column, '', '')])
            metadata.append(dict(site=site['site'], ref_file=e['ref_file'], mapping=e['mapping'], outcome=e['outcome']))
    site_counts.append([site['site'], len(site['exports']), counts['compared'], counts['passed'], counts['failed'], counts['error'], counts['skipped']])

for name, header, data in [
    ('exporte.csv', ['Site','Export','Mapping','Referenzzeilen','OS-Zeilen','Prozent','Schwelle','URL-Menge','Reihenfolge','Bestanden','Zellen','Zellfehler','Status','Ergebnis','Grund'],exports),
    ('spalten.csv', ['Site','Export','Referenzspalte','OS-Spalte','Verglichen','Fehler'], columns),
    ('befunde.csv', ['Site','Export','Mapping','Spalte','URL/Schluessel','Referenz','OpenSpider','Spec-ID','Kategorie','Crate','Datei'], rows),
    ('abweichungen.csv', ['Site','Export','ID','Spec-ID','Anzahl','Scope','Begruendung','Beispiele'],accepted),
    ('sites.csv', ['Site','Exporte','Verglichen','Bestanden','Verfehlt','Fehler','Uebersprungen'],site_counts)]:
    with (OUT/name).open('w') as f:
        w=csv.writer(f, lineterminator="\n");w.writerow(header);w.writerows(data)
(OUT/'export-evidence.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(site_counts,ensure_ascii=False,indent=2))
print('Befunde:',len(rows), 'Kategorien:', dict(collections.Counter(r[8] for r in rows)))

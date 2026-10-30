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
    if column in e.get('_missing_os', []) and any(token in column for token in ['Sentence','Reading','Readability']):
        return 'Implementierungsfehler', 'os-analyze', 'crates/os-analyze/src/tabs/content.rs (fehlende Spalten im Export-View)'
    if 'Outlinks To Localhost' in e['os'] or key == 'links | outlinks to localhost':
        return 'Referenzbesonderheit', 'os-parity', 'tests/parity/deviations.toml (TECH-141: Start-Host+Port ist ausgenommen)'
    if column == 'Inlinks' and key in ['{BASE}/contact.html','{BASE}/products.html']:
        return 'Referenzbesonderheit', 'os-parity', 'tests/parity/deviations.toml (TECH-130/K-07, fehlende Inlinks-Spalte in DEV-011)'
    if column == 'Word Count' and ref == '4' and measured == '3' and ('/graph/' in key or '/bookmarks.html' in key):
        return 'Referenzbesonderheit', 'os-parity', 'tests/parity/deviations.toml (ONPAGE-140/141: benachbarte Inline-Anker ohne Leerraum)'
    if column == 'URLs' and measured == '' and key in ['url | broken bookmark','content | near duplicates','images | background images','images | incorrectly sized images']:
        return 'Referenzbesonderheit', 'os-parity', 'tests/parity/deviations.toml (OUT-080: needs_config/needs_rendering schreibt leer)'

    if column == 'Nicht verglichen' and e['status'] == 'present':
        return 'Mapping-/Formatfehler', 'os-parity', 'tests/golden/parity/parity-response/manifest.toml (veraltetes M1-skip fuer OUT-084)'
    if column == 'Nicht verglichen':
        return 'Referenzbesonderheit', 'os-parity', 'tests/golden/parity/*/manifest.toml (dokumentierte Nichtverfuegbarkeit)'
    if e['mapping'] == 'crawl_overview' and column == 'Zeilenmenge':
        return 'Mapping-/Formatfehler', 'os-parity', 'tests/parity/mappings/crawl_overview.toml (Abschnitts-/Filternamen)'
    if column == 'URLs' and measured == '':
        return 'Implementierungsfehler', 'os-export', 'crates/os-export/src/reports/overview.rs (Zustand/Zaehler)'
    if column == 'URLs' and key.startswith('security |'):
        return 'Implementierungsfehler', 'os-analyze', 'crates/os-analyze/src/tabs/security.rs'
    if column == 'URLs' and key.startswith('content | readability'):
        return 'Implementierungsfehler', 'os-parse', 'crates/os-parse/src/ (Lesbarkeit)'
    if column == 'URLs' and key.startswith('internal | other'):
        return 'Referenzbesonderheit', 'os-parity', 'tests/parity/deviations.toml (ONPAGE-024/DEV-014: fehlender Content-Type ist Unknown)'

    if column in ['Mapping', 'Nicht verglichen']:
        return 'Mapping-/Formatfehler', 'os-parity', 'crates/os-parity/src/run.rs; tests/parity/mappings/' + e['mapping'] + '.toml'
    if column == 'Exportfehler':
        return 'Mapping-/Formatfehler', 'os-parity', 'crates/os-parity/src/run.rs; tests/golden/parity/*/manifest.toml'
    if ref == '0' or ref == '0.000':
        if measured == '' and any(t in column for t in ['Length', 'Width', 'Count', 'Ratio']):
            return 'Referenzbesonderheit', 'os-parity', 'tests/parity/deviations.toml (DEV-003/004, Geltungsbereich der neuen Mapping-Slugs)'
    if column in ['Text Ratio']:
        return 'Mapping-/Formatfehler', 'os-parity', 'tests/parity/mappings/' + e['mapping'] + '.toml (Rundung/Toleranz)'
    if 'sitemap' in e['mapping']:
        return 'Implementierungsfehler', 'os-export', 'crates/os-export/src/sitemap_gen/'
    if column == 'Original Url / Reihenfolge' or e['mapping'] == 'list_internal' and column in ['Zeilenmenge', 'Address']:
        return 'Referenzbesonderheit', 'os-cli', 'crates/os-cli/src/listing.rs (CRAWL-030: ungueltige Eingaben beibehalten)'
    if any(t in column for t in ['Width', 'Word Count', 'Reading', 'Readability', 'Sentence']):
        return 'Implementierungsfehler', 'os-parse', 'crates/os-parse/src/ (Pixel/Text/Lesbarkeit); crates/os-analyze/src/tabs/'
    if 'chain' in e['mapping']:
        return 'Mapping-/Formatfehler', 'os-parity', 'tests/parity/mappings/' + e['mapping'] + '.toml (OUT-081, DEV-019/030 und Spaltenalias)'
    if e['os'].startswith('bulk:'):
        return 'Implementierungsfehler', 'os-export', 'crates/os-export/src/links_export.rs; crates/os-export/src/pipeline.rs'
    return 'Implementierungsfehler', 'os-analyze', 'crates/os-analyze/src/tabs/; crates/os-analyze/src/analysis/steps/'

rows = []
accepted = []
exports = []
site_counts = []
metadata = []
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
                            r['cells_compared']-r['cells_equal']])
            for c in r['columns']:
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
                                 *classify(e, 'Zeilenmenge', '', '')])
            if r.get('row_order_identical') is False:
                rows.append([site['site'], e['ref_file'], e['mapping'], 'Original Url / Reihenfolge', '*',
                             'Upload-Reihenfolge', 'abweichend', 'OUT-015, OUT-085',
                             *classify(e, 'Original Url / Reihenfolge', '', '')])
            for field in ['missing_ref_columns', 'missing_os_columns', 'unmapped_ref_columns']:
                for col in r[field]:
                    rows.append([site['site'], e['ref_file'], e['mapping'], 'Mapping', col,
                                 field, 'nicht vollstaendig verglichen', spec_for(e, col),
                                 *classify(e, 'Mapping', '', '')])
            metadata.append(dict(site=site['site'], ref_file=e['ref_file'], mapping=e['mapping'],
                                 outcome='compared', agreement_pct=r['agreement_pct'], threshold_pct=r['threshold_pct'],
                                 passed=r['passed'], url_set_identical=r['url_set_identical'],
                                 compared_columns=len(r['columns']), missing_ref_columns=r['missing_ref_columns'],
                                 missing_os_columns=r['missing_os_columns'], unmapped_ref_columns=r['unmapped_ref_columns']))
        else:
            column = 'Exportfehler' if e['outcome']=='error' else 'Nicht verglichen'
            reason = e.get('message', e.get('reason', ''))
            rows.append([site['site'], e['ref_file'], e['mapping'], column, '*', e['status'], reason,
                         spec_for(e), *classify(e, column, '', '')])
            metadata.append(dict(site=site['site'], ref_file=e['ref_file'], mapping=e['mapping'], outcome=e['outcome']))
    site_counts.append([site['site'], len(site['exports']), counts['compared'], counts['passed'], counts['failed'], counts['error'], counts['skipped']])

for name, header, data in [
    ('exporte.csv', ['Site','Export','Mapping','Referenzzeilen','OS-Zeilen','Prozent','Schwelle','URL-Menge','Reihenfolge','Bestanden','Zellen','Zellfehler'],exports),
    ('befunde.csv', ['Site','Export','Mapping','Spalte','URL/Schluessel','Referenz','OpenSpider','Spec-ID','Kategorie','Crate','Datei'], rows),
    ('abweichungen.csv', ['Site','Export','ID','Spec-ID','Anzahl','Scope','Begruendung','Beispiele'],accepted),
    ('sites.csv', ['Site','Exporte','Verglichen','Bestanden','Verfehlt','Fehler','Uebersprungen'],site_counts)]:
    with (OUT/name).open('w') as f:
        w=csv.writer(f, lineterminator="\n");w.writerow(header);w.writerows(data)
(OUT/'export-evidence.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(site_counts,ensure_ascii=False,indent=2))
print('Befunde:',len(rows), 'Kategorien:', dict(collections.Counter(r[8] for r in rows)))

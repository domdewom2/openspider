#!/usr/bin/env python3
"""M2-P01: einmaliger Golden-Rahmen aus Spec/Katalog, niemals ref/ schreiben.

Erstaufbau: python3 -B tests/parity/gen/m2_exports.py
Nach einem Referenzexport Manifeste von Hand pflegen; dieses Skript überschreibt keine Dateien.
"""
import json
import re
import sys
from pathlib import Path
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
MAP = ROOT / 'tests/parity/mappings'
Q = lambda s: json.dumps(s, ensure_ascii=False)
SLUG = lambda s: re.sub('[^a-z0-9]+', '_', s.lower()).strip('_')
NORMALIZE = '\n[normalize]\nbase_url = true\ntrim = true\nbool = "case-insensitive"\nnumber_decimals = 3\nempty_equals_null = true\n'


def mapping(name, columns, key=('Address',), *, xml=False, ordered=False):
    target = MAP / (name + '.toml')
    if target.exists():
        return name
    text = '# M2-P01: Spec-abgeleitete Spalten; keine Referenzwerte oder Beschreibungstexte.\n'
    text += 'key = ' + Q(list(key)) + '\ndrop_ref_columns = ["Description", "How To Fix"]\n'
    if xml: text += 'format = "xml"\n'
    if ordered: text += 'ordered = true\n'
    text += '\n[columns]\n' + '\n'.join(Q(c) + ' = ' + Q(c) for c in columns) + '\n' + NORMALIZE
    target.write_text(text.rstrip() + '\n')
    return name


def new_manifest(site, port, entries, list_source=None):
    target = ROOT / 'tests/golden/parity' / site / 'manifest.toml'
    if target.exists(): return
    target.parent.mkdir(parents=True, exist_ok=True)
    text = '# M2-P01: Exportauftrag an den Orchestrator; ref/ bleibt unberührt.\n'
    text += f'site = {Q(site)}\nport = {port}\nref_version = "24.3"\nexported_at = ""\n'
    text += 'ref_settings = "Free, macOS, English (UK), Standardkonfiguration; Crawl Analysis ausgeführt. Hreflang/Pagination speichern, nicht crawlen. Always-Follow aus (Not Followed). Kein JavaScript, kein TLS. Gesperrte/leere Exporte als unavailable mit Grund dokumentieren."\n'
    if list_source: text += 'list_source = ' + Q(list_source) + '\n'
    for entry in entries:
        text += '\n[[export]]\n'
        for key, value in entry.items():
            if key in ['config', 'select']: continue
            text += key + ' = ' + Q(value) + '\n'
        text += 'status = "pending"\n'
        for field in ['config', 'select']:
            if entry.get(field):
                text += f'[export.{field}]\n' + '\n'.join(Q(k) + ' = ' + Q(v) for k, v in entry[field].items()) + '\n'
    target.write_text(text.rstrip() + '\n')


def entry(os, name=None, **opts):
    stem = ('bulk_' if os.startswith('bulk:') else 'report_' if os.startswith('report:Hreflang:') else '') + SLUG(os.split(':', 1)[1])
    result = dict(ref_file=stem + '.csv', os=os, mapping=name or stem)
    result.update(opts)
    return result


def tab(tab, filter='All', name=None):
    return entry('tab:' + tab + ':' + filter, name or SLUG(tab + ' All'))


def main():
    # Only tab/filter labels from the regular catalogue layout are needed here. The Rust
    # integration tests validate all resulting TOML, including the canonical filter catalogue.
    filters = []
    for section in (ROOT / 'docs/plan/M2-filters.toml').read_text().split('[[tab]]')[1:]:
        name = re.search(r'^id = "([^"]+)"', section, re.M).group(1)
        rows = [dict(id=label, extension='extension = true' in line) for line in section.splitlines()
                for label in re.findall(r'^  \{ id = "([^"]+)"', line)]
        filters.append(dict(id=name, filters=rows))
    defaults = {'title.max_px': 561, 'title.min_px': 200, 'meta_description.max_px': 985, 'meta_description.min_px': 400,
                'validation.html_doc_max_bytes': 2, 'validation.resource_max_bytes': 2}
    def label(text):
        for key, value in defaults.items(): text = text.replace('{' + key + '}', str(value))
        return text
    def selected(tabs, excludes=('All',)):
        return [tab(t['id'], label(f['id'])) for t in filters if t['id'] in tabs for f in t['filters'] if f['id'] not in excludes and not f.get('extension') and f['id'] not in ['Spelling Errors', 'Grammar Errors', 'Background Images', 'Incorrectly Sized Images']]
    for name, columns in {
        'meta_keywords_all': ['Address', 'Occurrences', 'Meta Keywords 1', 'Meta Keywords 1 Length', 'Meta Keywords 2', 'Meta Keywords 2 Length', 'Indexability', 'Indexability Status'],
        'pagination_all': ['Address', 'Occurrences', 'Indexability', 'Indexability Status', 'rel="next" 1', 'rel="next" 2', 'rel="prev" 1', 'Canonical Link Element 1', 'HTTP Canonical 1', 'Meta Robots 1', 'X-Robots-Tag 1'],
        'validation_all': ['Address', 'Content Type', 'Status Code', 'Status', 'Indexability', 'Indexability Status'],
        'security_all': ['Address', 'Content', 'Status Code', 'Status', 'Indexability', 'Indexability Status', 'Canonical Link Element 1', 'Canonical Link Element 2', 'Canonical Link Element 3', 'Meta Robots 1', 'Meta Robots 2', 'Meta Robots 3', 'X-Robots-Tag 1', 'X-Robots-Tag 2', 'X-Robots-Tag 3'],
        'm2_canonicals_non_indexable_canonicals': ['Source', 'Canonical', 'Status Code', 'Status', 'Indexability', 'Indexability Status', 'Unlinked'],
        'links_anchor_text': ['Source', 'Destination', 'Anchor', 'Alt Text', 'Link Position', 'Follow', 'Status Code'],
        'images_missing_sizes': ['From', 'To', 'Alt Text', 'Size', 'Real Dimensions (WxH)', 'Dimensions in Attributes (WxH)', 'Display Dimensions (WxH)', 'Potential Savings', 'Path Type', 'Link Path', 'Link Position', 'Link Origin'],
        'serp_summary': ['Address', 'Title 1', 'Title 1 Length', 'Title 1 Pixel Width', 'Meta Description 1', 'Meta Description 1 Length', 'Meta Description 1 Pixel Width'],
        'content_near_duplicates': ['Address', 'Near Duplicate Address', 'Similarity'],
        'crawl_path': ['Target Address', 'Step', 'Address', 'Status Code', 'Link Type', 'Anchor', 'Crawl Depth'],
        'hreflang_urls': ['Address', 'Hreflang Language', 'Hreflang URL', 'Annotation Source', 'Status Code', 'Indexability', 'Is Self Reference', 'Status'],
        'hreflang_missing_returns': ['Address', 'Hreflang Language', 'Hreflang URL', 'Hreflang URL Status Code', 'Hreflang URL Indexability', 'Missing Return Link To'],
        'hreflang_inconsistent_returns': ['Address', 'Hreflang URL', 'Expected Language', 'Actual Language'],
        'hreflang_noncanonical_returns': ['Address', 'Hreflang Language', 'Hreflang URL', 'Canonical Of Hreflang URL'],
        'hreflang_noindex_returns': ['Address', 'Hreflang Language', 'Hreflang URL', 'Indexability Status'],
    }.items():
        keys = ('Source', 'Destination', '#') if name == 'links_anchor_text' else ('Source', 'Canonical') if name == 'm2_canonicals_non_indexable_canonicals' else ('From', 'To', '#') if name == 'images_missing_sizes' else ('Target Address', 'Step') if name == 'crawl_path' else ('Address', 'Near Duplicate Address') if name == 'content_near_duplicates' else ('Address', 'Hreflang URL', '#') if name.startswith('hreflang_') else ('Address',)
        mapping(name, columns, keys)
    mapping('sitemap_xml', ['Address', 'Lastmod', 'Changefreq', 'Priority', 'Images'], xml=True)
    # Separate M2 copies compare newly implemented columns; existing M1 mappings stay untouched.
    for old, new in [('content_all', 'content_m2'), ('internal_all', 'internal_m2'), ('images_all', 'images_m2'), ('url_all', 'url_m2')]:
        target = MAP / (new + '.toml')
        if target.exists(): continue
        source = (MAP / (old + '.toml')).read_text()
        extra = {'content_all': ['Sentence Count', 'Average Words Per Sentence', 'Flesch Reading Ease Score', 'Readability', 'Closest Near Duplicate Match', 'No. Near Duplicates'],
                 'internal_all': ['Link Score'], 'images_all': [], 'url_all': []}[old]
        # Replace old ignore rules for M2-owned values, preserving all remaining baseline columns.
        for column in extra:
            pattern = r'^' + re.escape(Q(column)) + r' = .*?$'
            if re.search(pattern, source, re.M): source = re.sub(pattern, Q(column) + ' = ' + Q(column), source, flags=re.M)
            else: source = source.replace('[normalize]', Q(column) + ' = ' + Q(column) + '\n\n[normalize]')
        source = source.replace('"Closest Near Duplicate Match" = "Closest Near Duplicate Match"', '"Closest Near Duplicate Match" = { os = "Closest Similarity Match" }')
        target.write_text(source)
    # Upload order is significant, including repeat occurrences of an address.
    target = MAP / 'list_internal.toml'
    if not target.exists(): target.write_text((MAP / 'internal_m2.toml').read_text().replace('key = ["Address"]', 'ordered = true\nkey = ["Address", "#"]'))
    common = [tab('Internal', name='internal_m2'), tab('Response Codes'), entry('report:Crawl Overview')]
    onpage = common + [tab(t, name={'Content': 'content_m2', 'URL': 'url_m2', 'Images': 'images_m2'}.get(t)) for t in ['Page Titles', 'Meta Description', 'Meta Keywords', 'H1', 'H2', 'Content', 'URL', 'Images']]
    onpage += selected(['Page Titles', 'Meta Description', 'Meta Keywords', 'H1', 'H2', 'Content', 'URL', 'Images', 'Internal'])
    # Only requested Internal filters and spec-derived M2 tab mappings.
    for e in onpage:
        if e['mapping'] in ['content_all', 'url_all', 'images_all', 'internal_all']:
            e['mapping'] = {'content_all':'content_m2','url_all':'url_m2','images_all':'images_m2','internal_all':'internal_m2'}[e['mapping']]
    onpage += [entry('bulk:Content:Near Duplicates', 'content_near_duplicates'), entry('bulk:Images:Images Missing Size Attributes Inlinks', 'images_missing_sizes'), entry('report:SERP Summary')]
    for variant, kind, config in [('default', 'default', {}), ('options', 'default', {'sitemap_gen.lastmod.source':'server', 'sitemap_gen.priority.enabled':'true', 'sitemap_gen.changefreq.basis':'depth', 'sitemap_gen.images.enabled':'true'}), ('images', 'images', {})]:
        onpage.append(dict(ref_file='sitemap_' + variant + '.xml', os='sitemap:' + kind, mapping='sitemap_xml', os_file='images_sitemap.xml' if kind == 'images' else 'sitemap.xml', config=config))
    technical = common + [tab(t) for t in ['Canonicals', 'Pagination', 'Directives', 'Hreflang', 'Validation']]
    technical += selected(['Canonicals', 'Pagination', 'Directives', 'Hreflang', 'Validation'])
    technical.append(tab('Response Codes', 'Internal Redirection (HTTP Refresh)'))
    # Report schemas come from OUT-083/086/087 and OUT-090..096; chain exports share the baseline schema.
    for os in ['Redirects:Redirect & Canonical Chains', 'Canonicals:Canonical Chains', 'Canonicals:Non-Indexable Canonicals']:
        name = 'm2_' + SLUG(os)
        target = MAP / (name + '.toml')
        if not target.exists(): target.write_text((MAP / 'redirect_chains.toml').read_text())
        technical.append(entry('report:' + os, name))
    for name, schema in [('All Hreflang URLs','hreflang_urls'), ('Non-200 Hreflang URLs','hreflang_urls'), ('Unlinked Hreflang URLs','hreflang_urls'), ('Missing Return Links','hreflang_missing_returns'), ('Inconsistent Language & Region Return Links','hreflang_inconsistent_returns'), ('Non Canonical Return Links','hreflang_noncanonical_returns'), ('Noindex Return Links','hreflang_noindex_returns')]:
        technical.append(entry('report:Hreflang:' + name, schema))
    for group in ['canonicals', 'directives']:
        source = (ROOT / f'crates/os-export/src/catalog/bulk_{group}.rs').read_text()
        technical += [entry('bulk:' + name, 'all_inlinks') for name in re.findall(r'\bname: "([^"]+)"', source)]
    links = common + [tab('Links')] + selected(['Links'])
    for name in ['All Anchor Text', 'External Links', 'Internal Nofollow Outlinks', 'Internal Outlinks With No Anchor Text', 'Non-Descriptive Anchor Text In Internal Outlinks', 'Follow & Nofollow Internal Inlinks To Page', 'Internal Nofollow Inlinks Only', 'Outlinks To Localhost', 'Pages With Uncrawlable Internal Outlinks', 'Non-Indexable Page Inlinks Only']:
        links.append(entry('bulk:Links:' + name, 'links_anchor_text' if name == 'All Anchor Text' else 'all_inlinks'))
    links.append(entry('report:Crawl Path Report', 'crawl_path', ref_file='crawl_path_deep8.csv', select={'Target Address':'/deep/8.html'}))
    sec = common + [tab('Security')] + selected(['Security'])
    source = (ROOT / 'crates/os-export/src/catalog/bulk_security.rs').read_text()
    sec += [entry('bulk:' + name, 'all_inlinks') for name in re.findall(r'\bname: "([^"]+)"', source)]
    sec.append(entry('report:Insecure Content', 'all_inlinks'))
    listed = [tab('Internal', name='list_internal'), tab('Response Codes'), entry('report:Crawl Overview'), entry('report:Redirects:Redirect Chains', 'redirect_chains'), entry('report:Redirects:Redirect & Canonical Chains', 'm2_redirects_redirect_canonical_chains')]
    for e in listed:
        if e['os'].startswith('report:Redirects:'): e['mapping'] = 'list_chains'
    target = MAP / 'list_chains.toml'
    if not target.exists(): target.write_text((MAP / 'm2_redirects_redirect_canonical_chains.toml').read_text().replace('dedupe_reference = true', 'ordered = true').replace('key = ["Address"]', 'key = ["Address", "#"]'))
    for suffix, port, entries in [('onpage',8906,onpage),('technical',8907,technical),('links',8908,links),('security',8909,sec),('list',8910,listed)]:
        # Names/filenames are unique; repeated All exports from the table are collapsed.
        seen = set()
        entries = [e for e in entries if not (e['ref_file'] in seen or seen.add(e['ref_file']))]
        new_manifest('parity-m2-' + suffix, port, entries, 'list.txt' if suffix == 'list' else None)
        print(suffix, len(entries), 'Exporte')


if __name__ == '__main__': main()

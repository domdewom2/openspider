#!/usr/bin/env python3
"""Build per-export threshold ledger and conservative filter evidence from os-parity JSON."""
from collections import defaultdict
import json
from pathlib import Path
import sys
import tomllib

root=Path(__file__).resolve().parents[4]
out=Path(__file__).resolve().parent
summary=json.loads(Path(sys.argv[1]).read_text())
excluded_tabs={'JavaScript','Structured Data','PageSpeed','Mobile','Accessibility','Custom Search',
               'Custom Extraction','Custom JavaScript','Analytics','Search Console','Link Metrics','Sitemaps'}
excluded_ids={'ONPAGE-053','ONPAGE-057','ONPAGE-155','ONPAGE-156','ONPAGE-178','ONPAGE-180'}
by_tab=defaultdict(set)
excluded=defaultdict(set)
for milestone in ['M1','M2']:
    for tab in tomllib.loads((root/f'docs/plan/{milestone}-filters.toml').read_text())['tab']:
        for f in tab['filters']:
            if f.get('extension') or '<' in f['id']:
                continue
            if tab['id'] in excluded_tabs or f['spec'] in excluded_ids:
                excluded[f['spec']].add(tab['id']+':'+f['id'])
            else:
                by_tab[tab['id']].add(f['spec'])
referencable=set().union(*by_tab.values())
exports=defaultdict(set)
reports=defaultdict(list)
for site in summary['sites']:
    for e in site['exports']:
        if e['os'].startswith('tab:') and e['status']=='present':
            exports[e['os'].split(':')[1]].add(e['mapping'])
        if e['outcome']=='compared':
            reports[e['mapping']].append((site['site'],e['ref_file'],e['report']))

original=tomllib.loads((root/'tests/parity/evidence.toml').read_text())['evidence']
# Preserve original granular entries; incomplete Q02 tabs stay candidate_evidence.
# The coverage loader does not itself reject missing/unmapped columns (Q02-P02).
original=[e for e in original if not e.get('q02',False)]
for e in original:
    if e['specs']==['ONPAGE-012']:
        e['exports']=['internal_all','internal_m2','list_internal']
lines=['# M2-Q02: Belegregeln wie coverage/evidence.rs: URL-Menge identisch, null Zellfehler,',
       '# keine angewandte REF-FREE-Ausnahme. Schwellen allein ergeben kein verified.',
       '# export_result ist das additive M2-B02-Laufjournal je Manifest-Export.', '']
for e in original:
    lines.append('[[evidence]]')
    for k,v in e.items(): lines.append(k+' = '+json.dumps(v,ensure_ascii=False))
    lines.append('')
entries=[]
for tab,specs in sorted(by_tab.items()):
    if not exports[tab]: continue
    entry=dict(specs=sorted(specs),exports=sorted(exports[tab]))
    entries.append((tab,entry))
    lines+=['[[candidate_evidence]]','q02 = true','# Tab: '+tab,
            'specs = '+json.dumps(entry['specs']), 'exports = '+json.dumps(entry['exports']), '']

def check(e):
    for slug in e['exports']:
        if not reports[slug]: return False,slug+': nicht verglichen'
        for site,file,r in reports[slug]:
            if not r['url_set_identical']: return False,site+'/'+file+': URL-Menge'
            if any(c['errors'] for c in r['columns']): return False,site+'/'+file+': Zellfehler'
            if any(d['id'].startswith('REF-FREE-') for d in r['applied_deviations']): return False,site+'/'+file+': REF-FREE'
            # Stricter than current coverage loader: unmapped/missing columns never prove a complete tab.
            if r['missing_ref_columns'] or r['missing_os_columns'] or r['unmapped_ref_columns']:
                return False,site+'/'+file+': Spaltenabdeckung unvollstaendig'
    return True,'vollstaendig fehlerfrei verglichen'

states={}
for tab,e in entries:
    passed,why=check(e)
    for id in e['specs']:
        states.setdefault(id,[]).append(dict(tab=tab,passed=passed,reason=why))
verified=sorted(id for id,checks in states.items() if all(c['passed'] for c in checks))
# Only complete candidates can be consumed as verified evidence by coverage.
for tab,e in entries:
    if check(e)[0]:
        specs_line='specs = '+json.dumps(e['specs'])
        index=lines.index(specs_line, next(i for i,v in enumerate(lines) if v=='# Tab: '+tab))
        lines[index-3]='[[evidence]]'
for site in summary['sites']:
    for e in site['exports']:
        lines+=['[[export_result]]','site = '+json.dumps(site['site']),
                'file = '+json.dumps(e['ref_file']),'mapping = '+json.dumps(e['mapping']),
                'outcome = '+json.dumps(e['outcome'])]
        if e['outcome']=='compared':
            r=e['report']
            for key in ['agreement_pct','threshold_pct','passed','url_set_identical','cells_compared','cells_equal']:
                lines.append(key+' = '+json.dumps(r[key]))
            lines.append('complete_columns = '+json.dumps(not(r['missing_ref_columns'] or r['missing_os_columns'] or r['unmapped_ref_columns'])))
        lines.append('')
(root/'tests/parity/evidence.toml').write_text('\n'.join(lines))
result=dict(denominator=len(referencable),referencable_ids=sorted(referencable),
            verified_ids=verified, verified=len(verified),pct=round(100*len(verified)/len(referencable),3),target_pct=60,
            excluded_ids={k:sorted(v) for k,v in sorted(excluded.items())},checks=states,
            definition='Eindeutige Spec-IDs der M1+M2-Filterkataloge, ohne Erweiterungen, dynamische Filter und K2-14-Funktionsgruppen; keine Verkleinerung wegen roter Exporte.')
(out/'verified.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
print('verified',verified, 'of',len(referencable), 'pct',result['pct'])

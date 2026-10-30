#!/usr/bin/env python3
"""Assemble actual run journals; does not compute verified or change measured reports."""
import json
from pathlib import Path
import shutil
import sys

out = Path(__file__).resolve().parent
full_path, technical_path, response_path = map(Path, sys.argv[1:4])
full = json.loads(full_path.read_text())
technical = json.loads(technical_path.read_text())
assert len(full['sites']) == 10 and len(technical['sites']) == 1
replacement = technical['sites'][0]
assert replacement['site'] == 'parity-m2-technical'
for index, site in enumerate(full['sites']):
    if site['site'] == replacement['site']:
        full['sites'][index] = replacement
        break
else:
    raise ValueError('Site missing from full run')
full['failures'] = [f for f in full['failures'] if not f.startswith('parity-m2-technical:') and not f.startswith('parity-m2-technical/')]
full['failures'].extend(technical['failures'])
response = json.loads(response_path.read_text())
assert response['export'] == 'redirects_to_errors' and response['site'] == 'parity-response'
for site in full['sites']:
    if site['site'] == 'parity-response':
        for export in site['exports']:
            if export['ref_file'] == 'redirects_to_error.csv':
                assert export['outcome'] == 'compared'
                export['report'] = response
                break
full['failures'] = [f for f in full['failures'] if not f.startswith('parity-response/redirects_to_error.csv:')]
if not response['passed']:
    full['failures'].append('parity-response/redirects_to_error.csv: ' + '; '.join(response['failures']))
full['passed'] = not full['failures']
full['source_reports'] = ['lauf-n06-voll.json', 'lauf-n06-technical.json', 'diff-n06-redirects-to-errors.json']
full['replaced_site'] = replacement['site']
full['replaced_export'] = 'parity-response/redirects_to_error.csv'
shutil.copyfile(response_path, out / 'roh/diff-n06-redirects-to-errors.json')
shutil.copyfile(full_path, out / 'roh/lauf-n06-voll.json')
shutil.copyfile(technical_path, out / 'roh/lauf-n06-technical.json')
(out / 'roh/lauf-n06.json').write_text(json.dumps(full, ensure_ascii=False, indent=2) + '\n')
print('10 Sites; technical rerun plus Redirects-to-Errors recompare; measured reports copied unchanged')

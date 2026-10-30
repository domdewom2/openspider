#!/usr/bin/env python3
"""Create a temporary Q02 mapping overlay without changing repository mappings."""
from pathlib import Path
import shutil
import tempfile

root = Path(__file__).resolve().parents[4]
target = Path(tempfile.mkdtemp(prefix='m2-q02-'))
(target/'tests/parity').mkdir(parents=True)
for name in ['fixtures', 'golden']:
    (target/'tests'/name).symlink_to(root/'tests'/name)
(target/'docs').symlink_to(root/'docs')
for name in ['deviations.toml', 'thresholds.toml', 'profiles']:
    (target/'tests/parity'/name).symlink_to(root/'tests/parity'/name)
shutil.copytree(root/'tests/parity/mappings', target/'tests/parity/mappings')
p = target/'tests/parity/mappings/crawl_overview.toml'
p.write_text(p.read_text().replace('\n[blocks]\n', '\n[blocks]\nfilters = ["docs/plan/M2-filters.toml"]\n'))
p = target/'tests/parity/mappings/links_all.toml'
lines = p.read_text().splitlines()
for column in ['Link Score', 'Inlinks']:
    lines = [f'"{column}" = "{column}"' if line.startswith(f'"{column}" =') else line for line in lines]
p.write_text('\n'.join(lines)+'\n')
print(target)

#!/usr/bin/env python3
"""Normalize volatile fields and the test site's ephemeral port, then sort CSV rows."""
import csv
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

source = Path(sys.argv[1])
dest = Path(sys.argv[2])
dest.mkdir(parents=True, exist_ok=True)
for name in ('internal_all.csv', 'links_all_inlinks.csv'):
    with (source / name).open(encoding='utf-8-sig', newline='') as inp:
        reader = csv.reader(inp)
        header = next(reader)
        keep = [i for i, col in enumerate(header) if col not in ('Response Time', 'Crawl Timestamp')]
        with tempfile.NamedTemporaryFile(mode='w', encoding='utf-8', newline='', dir=dest, delete=False) as tmp:
            for row in reader:
                tmp.write(json.dumps([re.sub(r'127\.0\.0\.1:\d+', '127.0.0.1:PORT', row[i]) for i in keep], ensure_ascii=False, separators=(',', ':')) + '\n')
            temp = tmp.name
        try:
            with (dest / name).open('w', encoding='utf-8', newline='') as out:
                out.write(json.dumps([header[i] for i in keep], ensure_ascii=False, separators=(',', ':')) + '\n')
                out.flush()
                subprocess.run(['sort', temp], stdout=out, env={**os.environ, 'LC_ALL': 'C'}, check=True, timeout=5400)
        finally:
            os.unlink(temp)

"""Fünf deterministische Seiten für Duplicate Details (UI-035, ONPAGE-145)."""
from pathlib import Path
import hashlib
import json
root = Path(__file__).parent
words = [f'word{n:03}' for n in range(102)]
d = words.copy()
d[35] = 'replacement'
e = words.copy()
for n in [0, 20, 50, 80, 101]:
    e[n] = f'variant{n}'
texts = {'A': 'A separate exact duplicate with unrelated content.',
         'B': 'A separate exact duplicate with unrelated content.',
         'C': ' '.join(words), 'D': ' '.join(d), 'E': ' '.join(e)}
pages = []
for name, text in texts.items():
    title = 'Exact' if name in ['A', 'B'] else name
    html = f'<!doctype html><html lang="en"><head><title>{title}</title></head><body><main>{text}</main></body></html>\n'
    root.joinpath(name + '.html').write_text(html)
    pages.append({'name': name, 'text': text, 'hash': hashlib.md5(html.encode()).hexdigest()})
root.joinpath('expected.json').write_text(json.dumps({
    'pages': pages,
    'exact_pairs': [['A', 'B']],
    'near_pairs_at_90': [['C', 'D']],
    'display_similarity_C_D': 94,
    'display_similarity_C_E': 80
}, ensure_ascii=False, indent=2) + '\n')

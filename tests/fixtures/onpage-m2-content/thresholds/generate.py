"""Own deterministic corpus: disjoint replacements, exact Jaccard and word diffs."""
import json
from pathlib import Path

root = Path(__file__).parent
base = [f'word{i:03}' for i in range(300)]
pages = {'base': base}
diffs = {}
for name, start, length in [('95', 25, 6), ('91', 80, 12), ('89', 140, 15), ('80', 210, 31)]:
    words = base.copy()
    words[start:start + length] = [f'changed{name}x{i}' for i in range(length)]
    pages[name] = words
    diffs[name] = list(range(start, start + length))
for name, words in pages.items():
    (root / f'near-{name}.html').write_text('<!doctype html><html><body><main>' + ' '.join(words) + '</main></body></html>\n')

def shingles(words):
    return {tuple(words[i:i + 3]) for i in range(len(words) - 2)}

expected = {}
for a, words in pages.items():
    for b, other in pages.items():
        if a < b:
            x, y = shingles(words), shingles(other)
            expected[f'{a}:{b}'] = len(x & y) / len(x | y)
(root / 'expected_jaccard.json').write_text(json.dumps(expected, indent=2) + '\n')
(root / 'expected_diff.json').write_text(json.dumps(diffs, indent=2) + '\n')

#!/usr/bin/env python3
"""OUT-263: byte equality against installed Windows x64, plus emulation throughput."""
import argparse
import json
from pathlib import Path


def compare(root):
    reference = root / 'windows-latest' / 'msi'
    expected = [(platform, kind) for platform, kinds in (
        ('macos-15', ('dmg',)), ('macos-15-intel', ('dmg',)),
        ('windows-latest', ('msi', 'nsis')), ('windows-11-arm', ('msi', 'nsis')))
        for kind in kinds]
    lines = ['### Installed smoke (OUT-263)', '',
             '| Runner / installer | Text + JS export | URLs/s (median) |', '|---|---|---:|']
    for platform, kind in expected:
        folder = root / platform / kind
        report = json.loads((folder / 'result.json').read_text(encoding='utf-8'))
        assert report['dry_run'] is False, f'Dry run is not native evidence: {folder}'
        for mode in ('text', 'javascript'):
            assert (folder / f'{mode}.csv').read_bytes() == (reference / f'{mode}.csv').read_bytes(), \
                f'{platform}/{kind}/{mode}: export differs from installed x64 reference'
        lines.append(f"| {platform} / {kind} | byte-identical | {report['median_urls_per_second']:.2f} |")
    lines += ['', 'K3-10: Richtwert ≥ 50 % gilt nur bei vergleichbarer Hardware. GitHub-Runner haben '
              'unterschiedliche Hardware; die folgenden Quotienten sind Beobachtungen, kein Hardware-Gate.']
    for kind in ('msi', 'nsis'):
        native = json.loads((root / 'windows-latest' / kind / 'result.json').read_text())
        arm = json.loads((root / 'windows-11-arm' / kind / 'result.json').read_text())
        ratio = arm['median_urls_per_second'] / native['median_urls_per_second']
        lines.append(f"\n{kind}: ARM emuliert / x64 = {ratio:.1%}; Richtwert "
                     f"{'erreicht' if ratio >= .5 else 'unterschritten (Hardware prüfen)'}. "
                     f"x64: {native['cpus']} CPUs, {native['processor']}; ARM: {arm['cpus']} CPUs, {arm['processor']}.")
    return '\n'.join(lines) + '\n'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results', type=Path)
    parser.add_argument('--summary', type=Path)
    options = parser.parse_args()
    text = compare(options.results)
    print(text)
    if options.summary:
        with options.summary.open('a', encoding='utf-8') as target:
            target.write(text)

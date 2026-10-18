#!/usr/bin/env python3
"""M2-P01: ausschließlich neue Fixtures erzeugen; --check vergleicht Bytes ohne Schreiben.

python3 -B tests/parity/gen/m2_sites.py [--check]
Die M1-Generatoren und vorhandenen Sites werden niemals aufgerufen oder verändert.
"""
import argparse
import html
import json
import re
import sys
from pathlib import Path

sys.dont_write_bytecode = True
import lib

ROOT = Path(__file__).resolve().parents[3]
Q = lambda value: json.dumps(value, ensure_ascii=False)


class Site:
    def __init__(self, suffix, port):
        self.name = 'parity-m2-' + suffix
        self.port = port
        self.files = {}
        self.routes = []
        self.links = []
        self.cases = []

    def url(self, path, external=False):
        return f'http://{"localhost" if external else "127.0.0.1"}:{self.port}{path}'

    def add(self, path, spec, *, head='', body='', title=None, linked=True, status=200, headers=None, raw=None, **route):
        if title is None:
            title = 'Fixture ' + path
        content = raw if raw is not None else lib.doc('<title>' + html.escape(title) + '</title>\n' + head,
            '<main>' + body + '</main>')
        self.files[path.lstrip('/') or 'index.html'] = content.encode()
        if status != 200 or headers or route:
            self.routes.append(dict(path=path, status=status, headers=headers or {}, **route))
        if linked:
            self.links.append(path)
        self.cases.append(dict(path=path, spec=spec, status=status))
        return content

    def finish(self):
        self.add('/', 'CRAWL-001', linked=False, body=lib.link_list((p, p) for p in self.links),
                 headers={'Last-Modified': 'Thu, 01 Oct 2026 10:00:00 GMT'})
        self.files['robots.txt'] = b'User-agent: *\nAllow: /\n'
        lines = ['# M2-P01: generiert durch tests/parity/gen/m2_sites.py.']
        for route in self.routes:
            lines += ['\n[[route]]']
            for key, value in route.items():
                if key == 'headers':
                    continue
                lines.append(f'{key} = {value if isinstance(value, int) else Q(value)}')
            if route['headers']:
                lines.append('[route.headers]')
                lines.extend(Q(k) + ' = ' + Q(v) for k, v in route['headers'].items())
        self.files['scenario.toml'] = ('\n'.join(lines) + '\n').encode()
        self.files['cases.json'] = (json.dumps(self.cases, ensure_ascii=False, indent=2) + '\n').encode()
        return self


def pixels(width, profile):
    # Eigene kalibrierte Engine lesen, keine Font-/Referenzressourcen öffnen.
    source = (ROOT / 'crates/os-parse/src/pixel/calibrated.rs').read_text(encoding="utf-8")
    table = source.split('const ' + profile + ':', 1)[1].split('];', 1)[0]
    widths = dict((c, int(n)) for c, n in re.findall(r"\('([^']+)', (\d+)\)", table))
    for a in range(width // widths['a'], -1, -1):
        remaining = width - a * widths['a']
        if remaining % widths['i'] == 0:
            return 'a' * a + 'i' * (remaining // widths['i'])
    raise AssertionError(width)


def linked_pdf(url):
    # Eigenes minimales PDF mit echtem Text und einer URI-Link-Annotation.
    objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 200] /Contents 4 0 R /Annots [6 0 R] /Resources << /Font << /F1 5 0 R >> >> >>']
    stream = b'BT /F1 12 Tf 10 100 Td (A short local PDF has words and a link.) Tj ET'
    objects += [b'<< /Length ' + str(len(stream)).encode() + b' >>\nstream\n' + stream + b'\nendstream',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
        ('<< /Type /Annot /Subtype /Link /Rect [10 50 180 70] /A << /S /URI /URI (' + url + ') >> >>').encode()]
    data, offsets = b'%PDF-1.4\n', []
    for index, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data += f'{index} 0 obj\n'.encode() + obj + b'\nendobj\n'
    start = len(data)
    data += f'xref\n0 {len(objects)+1}\n'.encode() + b'0000000000 65535 f \n'
    data += b''.join(f'{off:010d} 00000 n \n'.encode() for off in offsets)
    return data + f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{start}\n%%EOF\n'.encode()


def onpage():
    s = Site('onpage', 8906)
    for name, content in [('missing', ''), ('empty', '<meta name="keywords" content="">'),
        ('duplicate-a', '<meta name="keywords" content="river, stone">'),
        ('duplicate-b', '<meta name="keywords" content="river, stone">'),
        ('multiple', '<meta name="keywords" content="one"><meta name="keywords" content="two">'),
        ('long', '<meta name="keywords" content="' + 'landscape ' * 100 + '">'),
        ('unique', '<meta name="keywords" content="distinct, garden">')]:
        s.add('/keywords/' + name + '.html', 'ONPAGE-110', head=content, body='<h1>Keywords case</h1>')
    boundaries = []
    for field, limits, profile, spec in [('title', [200, 561], 'TITLE_20', 'ONPAGE-095'), ('description', [400, 985], 'DESCRIPTION_14', 'ONPAGE-105')]:
        for limit in limits:
            for delta in [-1, 0, 1]:
                width = limit + delta
                value = pixels(width, profile)
                path = f'/pixels/{field}-{width}.html'
                if field == 'title':
                    s.add(path, spec, title=value, body='<h1>Pixel boundary</h1>')
                else:
                    s.add(path, spec, head=f'<meta name="description" content="{value}">', body='<h1>Pixel boundary</h1>')
                boundaries.append(dict(path=path, field=field, pixels=width, value=value))
    s.files['pixels.json'] = (json.dumps(boundaries, indent=2) + '\n').encode()
    s.add('/same-h1.html', 'ONPAGE-097', title='River garden', body='<h1>River garden</h1>')
    s.add('/outside-head.html', 'ONPAGE-099', raw='<html><head></head><body><p>Body before metadata.</p><title>Outside title</title><meta name="description" content="Outside description"><h1>Outside metadata</h1></body></html>')
    s.add('/alt-h1.html', 'ONPAGE-125', body='<h1><img src="/picture.png" alt="River landscape"></h1>')
    s.add('/headings-h1.html', 'ONPAGE-126', body='<h2>Early second level</h2><h1>Late first level</h1>')
    s.add('/headings-h2.html', 'ONPAGE-135', body='<h1>Start</h1><h3>Skipped second level</h3><h2>Late second level</h2>')
    for path in ['/catalog/catalog/catalog.html', '/search?q=river', '/?s=stone', '/tracking.html?utm_source=fixture&utm_medium=local&utm_campaign=m2']:
        s.add(path.split('?')[0] or '/', 'ONPAGE-075', linked=False, body='<h1>URL case</h1>') if path.split('?')[0] not in ['/', ''] else None
        s.links.append(path)
    s.add('/bookmarks.html', 'ONPAGE-079', body='<a href="/bookmark-target.html#fehlt">Broken bookmark</a><a href="/bookmark-target.html#exists">Valid bookmark</a>')
    s.add('/bookmark-target.html', 'ONPAGE-079', body='<h1 id="exists">Existing fragment</h1>')
    s.add('/lorem.html', 'ONPAGE-148', body='<h1>Placeholder</h1><p>Lorem ipsum dolor sit amet.</p>')
    for i, phrase in enumerate(['Page not found', 'Error 404', 'Seite nicht gefunden']):
        s.add(f'/soft404-{i}.html', 'ONPAGE-147', body='<h1>' + phrase + '</h1>')
    s.add('/real404.html', 'ONPAGE-147', status=404, body='<h1>Page not found</h1>')
    texts = {'easy': 'The cat sat on a mat. The dog ran to the pond. We can see the sun.',
             'difficult': 'The professor explained the process while the students studied each example and discussed the important results in their small groups.',
             'very-difficult': 'Interdisciplinary institutionalization systematically operationalizes multidimensional epistemological conceptualizations, notwithstanding methodological incompatibilities and disproportionate organizational responsibilities.'}
    for label, text in texts.items():
        s.add('/readability-' + label + '.html', 'ONPAGE-149', body='<p>' + text + '</p>')
    # Unique alphabetic tokens prevent repeated shingles. Prefix truncation yields exact 95/85 %.
    def token(i):
        return 'word' + chr(97 + i // 26) + chr(97 + i % 26)
    tokens = [token(i) for i in range(302)]
    for name, length in [('base', 302), ('95', 287), ('85', 257)]:
        s.add('/near-' + name + '.html', 'ONPAGE-144', body='<p>' + ' '.join(tokens[:length]) + '</p>')
    s.files['expected_jaccard.json'] = b'{"base/95": 95.0, "base/85": 85.0, "95/85": 89.47368421052632}\n'
    s.add('/images.html', 'ONPAGE-179', body='<h1>Image sizes</h1><img src="/picture.png" alt="no size"><img src="/wide.png" width="8" alt="no height"><img src="/sized.png" width="8" height="8" alt="complete"><div style="background-image:url(/background.png)">Backdrop</div>')
    for path in ['picture.png', 'wide.png', 'sized.png', 'background.png']:
        s.files[path] = lib.png()
    for path, data, ctype in [('fixture.swf', b'FWS\x09\x08\x00\x00\x00', 'application/x-shockwave-flash'), ('fixture.mp4', b'\x00\x00\x00\x14ftypisom', 'video/mp4'), ('fixture.woff2', b'wOF2\x00\x00\x00\x00', 'font/woff2'), ('fixture.xml', b'<?xml version="1.0"?><fixture/>', 'application/xml')]:
        s.files[path] = data
        s.links.append('/' + path)
        s.routes.append(dict(path='/' + path, content_type=ctype, headers={}))
    s.files['.gitattributes'] = b'text-links.pdf binary\n'
    s.files['text-links.pdf'] = linked_pdf(s.url('/pdf-target.html'))
    s.links.append('/text-links.pdf')
    s.add('/pdf-target.html', 'ONPAGE-166', linked=False, body='<h1>PDF link target</h1>')
    for i, title in enumerate(['Straße', 'STRASSE', 'é', 'e\u0301', 'Σ', 'σ', 'ς']):
        s.add(f'/unicode-{i}.html', 'ONPAGE-092', title=title, body='<h1>Unicode comparison fixture</h1>')
    s.add('/list-item-text.html', 'ONPAGE-141', body='<h1>Content</h1><ul><li>Standalone words without any hyperlink</li><li><a href="/">Linked item</a></li></ul>')
    s.routes.append(dict(path='/search', content_type='text/html', headers={}))
    return s.finish()


def technical():
    s = Site('technical', 8907)
    for name, href, extra in [('relative', 'canonical-target.html', ''), ('unlinked', '/canonical-unlinked-target.html', ''),
        ('invalid-attribute', '/canonical-target.html', ' hreflang="en"'), ('fragment', '/canonical-target.html#fragment', '')]:
        s.add('/canonical-' + name + '.html', 'TECH-010', head=f'<link rel="canonical" href="{href}"{extra}>')
    s.add('/canonical-target.html', 'TECH-010', body='<h1 id="fragment">Canonical target</h1>')
    s.add('/canonical-unlinked-target.html', 'TECH-019', linked=False)
    s.add('/canonical-outside.html', 'TECH-023', body='<p>Before annotation</p><link rel="canonical" href="/canonical-target.html">')
    s.add('/canonical-chain-a.html', 'TECH-026', head='<link rel="canonical" href="/canonical-chain-b.html">')
    s.add('/canonical-chain-b.html', 'TECH-026', linked=False, head='<link rel="canonical" href="/canonical-target.html">')
    s.routes.append(dict(path='/redirect-canonical', status=301, location='/canonical-chain-a.html', headers={}))
    s.links.append('/redirect-canonical')
    s.add('/canonical-to-noindex.html', 'TECH-018', head='<link rel="canonical" href="/canonical-noindex.html">')
    s.add('/canonical-noindex.html', 'TECH-018', head='<meta name="robots" content="noindex">')
    for i in range(1, 5):
        head = (f'<link rel="prev" href="/page-{i-1}.html">' if i > 1 else '') + (f'<link rel="next" href="/page-{i+1}.html">' if i < 4 else '')
        s.add(f'/page-{i}.html', 'TECH-031', head=head, linked=i == 1, body=(f'<a href="/page-{i+1}.html">Next</a>' if i < 4 else ''))
    for label, target in [('only-link', '/pagination-unlinked.html'), ('404', '/pagination-404-target.html'), ('noindex', '/pagination-noindex.html')]:
        s.add('/pagination-' + label + '.html', 'TECH-035', head=f'<link rel="next" href="{target}">')
    s.add('/pagination-unlinked.html', 'TECH-037', linked=False)
    s.add('/pagination-404-target.html', 'TECH-036', status=404)
    s.add('/pagination-noindex.html', 'TECH-038', head='<meta name="robots" content="noindex">')
    s.add('/pagination-multiple.html', 'TECH-039', head='<link rel="next" href="/page-2.html"><link rel="next" href="/page-3.html">')
    for a, b in [('a', 'b'), ('b', 'a')]:
        s.add('/pagination-loop-' + a + '.html', 'TECH-040', head=f'<link rel="next" href="/pagination-loop-{b}.html">')
    s.add('/pagination-sequence.html', 'TECH-041', head='<link rel="next" href="/page-3.html">')
    directives = ['noarchive', 'nosnippet', 'max-snippet:20', 'max-image-preview:large', 'max-video-preview:10', 'noimageindex', 'notranslate', 'unavailable_after: 01 Jan 2000 00:00:00 GMT']
    for i, value in enumerate(directives):
        s.add(f'/directive-{i}.html', 'TECH-050', head=f'<meta name="robots" content="{value}">')
    s.add('/robots-outside.html', 'TECH-065', body='<p>Before robots</p><meta name="robots" content="noarchive">')
    for label, header in [('noindex', 'noindex'), ('bot', 'googlebot: nosnippet'), ('all', 'noarchive, max-snippet:40')]:
        s.add('/xrobots-' + label + '.html', 'TECH-050', headers={'X-Robots-Tag': header})
    # All annotation targets exist. Unlinked targets stay outside the root hub (K2-59: store only).
    def ann(code, target): return f'<link rel="alternate" hreflang="{code}" href="{target}">'
    for label, code, extra in [('valid', 'en', ''), ('inconsistent', 'fr', ''), ('noindex', 'en', '<meta name="robots" content="noindex">'), ('noncanonical', 'en', '<link rel="canonical" href="/canonical-target.html">')]:
        source, target = f'/lang-{label}.html', f'/lang-{label}-return.html'
        s.add(source, 'TECH-070', head=ann('en', source) + ann('de', target) + ann('x-default', source))
        s.add(target, 'TECH-077', head=ann('fr' if label == 'inconsistent' else 'de', target) + ann(code, source) + extra)
    s.add('/lang-unlinked.html', 'TECH-075', head=ann('en', '/lang-unlinked.html') + ann('de', '/lang-unlinked-target.html'))
    s.add('/lang-unlinked-target.html', 'TECH-075', linked=False, head=ann('en', '/lang-unlinked.html'))
    s.add('/lang-to-404.html', 'TECH-074', head=ann('en', '/lang-to-404.html') + ann('de', '/pagination-404-target.html'))
    s.add('/lang-multiple.html', 'TECH-081', head=ann('en', '/lang-valid.html') + ann('en', '/lang-inconsistent.html'))
    s.add('/lang-missing-self.html', 'TECH-082', head=ann('de', '/lang-valid.html'))
    s.add('/lang-not-canonical.html', 'TECH-083', head=ann('en', '/lang-not-canonical.html') + '<link rel="canonical" href="/canonical-target.html">')
    s.add('/lang-missing-default.html', 'TECH-084', head=ann('en', '/lang-missing-default.html'))
    s.add('/lang-outside.html', 'TECH-086', body='<p>Before annotation</p>' + ann('en', '/lang-valid.html'))
    malformed = {'img-head': '<html><head><title>Bad head</title><img src="/head.png"></head><body>Image in head</body></html>',
        'head-not-first': '<html><p>Before head</p><head><title>Late head</title></head><body>Body</body></html>',
        'head-missing': '<html><title>No explicit head</title><body>Body</body></html>',
        'head-multiple': '<html><head><title>First</title></head><head><title>Second</title></head><body>Body</body></html>',
        'body-missing': '<html><head><title>No explicit body</title></head><p>Text</p></html>',
        'body-multiple': '<html><head><title>Two bodies</title></head><body>First</body><body>Second</body></html>',
        'body-before-html': '<body>Early body</body><html><head><title>Late html</title></head></html>'}
    for i, (name, raw) in enumerate(malformed.items()):
        s.add('/validation-' + name + '.html', 'TECH-' + str(282 + i), raw=raw)
    s.files['head.png'] = lib.png()
    s.add('/large.html', 'TECH-289', body='<h1>Large HTML</h1>', body_size=2 * 1024 * 1024 + 1)
    s.routes.append(dict(path='/large.css', content_type='text/css', body='/* Large resource */', body_size=2 * 1024 * 1024 + 1, headers={}))
    s.links.append('/large.css')
    s.add('/refresh.html', 'ONPAGE-059', headers={'Refresh': '0; url=/canonical-target.html'})
    s.add('/refresh-self.html', 'ONPAGE-059', headers={'Refresh': '0; url=/refresh-self.html'})
    for path, location in [('/tail', '/loop-a'), ('/loop-a', '/loop-b'), ('/loop-b', '/loop-a')]:
        s.routes.append(dict(path=path, status=302, location=location, headers={}))
    s.links.append('/tail')
    s.add('/ext-noindex.html', 'TECH-063', linked=False, head='<meta name="robots" content="noindex">')
    s.links.append(s.url('/ext-noindex.html', True))
    return s.finish()


def links():
    s = Site('links', 8908)
    s.add('/anchors.html', 'TECH-136', body='<a href="/target.html">click here</a><a href="/target.html">read more</a><a href="/target.html"></a><a href="/target.html">River guide</a>')
    s.add('/target.html', 'TECH-136', body='<h1>Target</h1>')
    for count in [999, 1001]:
        s.add(f'/internal-{count}.html', 'TECH-138', body='<a href="/target.html">Target</a>' * count)
    for count in [99, 101]:
        s.add(f'/external-{count}.html', 'TECH-137', body=f'<a href="{s.url("/external.html", True)}">External</a>' * count)
    s.add('/external.html', 'TECH-137', linked=False)
    s.add('/mixed-source.html', 'TECH-139', body='<a href="/mixed.html">Follow</a><a rel="nofollow" href="/mixed.html">Nofollow</a>')
    s.add('/mixed.html', 'TECH-139', linked=False)
    s.add('/nofollow-source.html', 'TECH-140', body='<a rel="nofollow" href="/nofollow-only.html">Only nofollow</a>')
    s.add('/nofollow-only.html', 'TECH-140', linked=False)
    s.add('/noindex-source.html', 'TECH-142', head='<meta name="robots" content="noindex">', body='<a href="/noindex-inlinks-only.html">Only non-indexable source</a>')
    s.add('/noindex-inlinks-only.html', 'TECH-142', linked=False)
    s.add('/uncrawlable.html', 'TECH-143', body='<a href="javascript:void(0)">Script pseudo-link</a><a href="invalid:fixture">Invalid scheme</a>')
    s.add('/positions.html', 'TECH-144', raw=lib.doc('<title>Link positions</title>', ''.join(f'<{tag}><a href="/target.html">{tag}</a></{tag}>' for tag in ['nav', 'header', 'footer', 'aside', 'main'])))
    for depth in range(1, 9):
        s.add(f'/deep/{depth}.html', 'OUT-021', linked=depth == 1, body=(f'<a href="/deep/{depth+1}.html">Next depth</a>' if depth < 8 else '<h1>Deepest page</h1>'))
    # A moderately sized graph for Link Score. High-outlink cases repeat targets to stay below 500 URLs.
    for i in range(120):
        s.add(f'/graph/{i}.html', 'TECH-145', body=f'<a href="/graph/{(i+1)%120}.html">Next node</a><a href="/target.html">Shared target</a>')
    return s.finish()


def security():
    s = Site('security', 8909)
    s.add('/form-http.html', 'ONPAGE-034', body=f'<form action="{s.url("/submit")}" method="post"><input name="q"><button>Submit</button></form>')
    s.add('/form-relative.html', 'ONPAGE-035', body='<form action="/submit"><input name="q"><button>Submit</button></form>')
    s.add('/submit', 'ONPAGE-035', body='<h1>Form target</h1>')
    for name, rel in [('unsafe', ''), ('noopener', 'rel="noopener"'), ('noreferrer', 'rel="noreferrer"')]:
        s.add('/blank-' + name + '.html', 'ONPAGE-036', body=f'<a href="{s.url("/external.html", True)}" target="_blank" {rel}>External page</a>')
    s.add('/external.html', 'ONPAGE-036', linked=False)
    s.add('/protocol-relative.html', 'ONPAGE-037', body=f'<img src="//localhost:{s.port}/picture.png" alt="External image">')
    s.files['picture.png'] = lib.png()
    policies = {'Content-Security-Policy': "default-src 'self'", 'X-Content-Type-Options': 'nosniff', 'X-Frame-Options': 'DENY', 'Referrer-Policy': 'strict-origin-when-cross-origin'}
    s.add('/headers-all.html', 'ONPAGE-039', headers=policies)
    s.add('/headers-none.html', 'ONPAGE-039')
    for name in policies:
        s.add('/without-' + name.lower() + '.html', 'ONPAGE-040', headers={k: v for k, v in policies.items() if k != name})
    s.add('/bad-types.html', 'ONPAGE-043', head='<link rel="stylesheet" href="/wrong.css">', body='<script src="/wrong.js"></script>')
    # No executable JavaScript, even if a browser opens the fixture. Bodies are empty comments.
    s.files['wrong.js'], s.files['wrong.css'] = b'/* fixture */\n', b'/* fixture */\n'
    s.routes += [dict(path='/wrong.js', content_type='text/plain', headers={}), dict(path='/wrong.css', content_type='text/html', headers={})]
    return s.finish()


def list_site():
    s = Site('list', 8910)
    for i in range(40):
        s.add(f'/item-{i:02}.html', 'CRAWL-029', body=f'<h1>Uploaded item {i}</h1>')
    # Route bodies distinguish URL case even on case-insensitive macOS filesystems.
    for path in ['/Case.html', '/case.html']:
        s.routes.append(dict(path=path, content_type='text/html', body=lib.doc('<title>' + path + '</title>', '<h1>' + path + '</h1>'), headers={}))
        s.links.append(path)
        s.cases.append(dict(path=path, spec='CRAWL-030', status=200))
    s.add('/canonical.html', 'CRAWL-030')
    s.add('/canonical-source.html', 'CRAWL-030', head='<link rel="canonical" href="/canonical.html">')
    for path, location in [('/redirect', '/item-00.html'), ('/chain', '/redirect')]:
        s.routes.append(dict(path=path, status=301, location=location, headers={}))
    # Intentionally not sorted; duplicate uploads remain separate rows in Internal:All (OUT-015).
    rows = [s.url(f'/item-{i:02}.html') for i in range(39, -1, -1)]
    rows += [s.url('/item-00.html'), s.url('/item-39.html'), s.url('/Case.html'), s.url('/case.html'), s.url('/redirect'), s.url('/chain'), s.url('/canonical-source.html'), s.url('/canonical.html')]
    rows += ['not a URL', '', 'http://', '://invalid', s.url('/item-05.html'), s.url('/item-05.html'), s.url('/item-06.html'), s.url('/item-06.html'), s.url('/item-07.html'), s.url('/item-07.html'), s.url('/item-08.html'), s.url('/item-08.html')]
    assert len(rows) == 60
    s.files['list.txt'] = ('\n'.join(rows) + '\n').encode()
    return s.finish()


def generated():
    out = {}
    for site in [onpage(), technical(), links(), security(), list_site()]:
        for rel, data in site.files.items():
            out[Path('tests/fixtures') / site.name / rel] = data
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    files = generated()
    index = Path('tests/parity/gen/generated_m2.txt')
    files[index] = ('\n'.join(sorted(str(p) for p in files)) + '\n').encode()
    if args.check:
        bad = [str(p) for p, data in files.items() if not (ROOT / p).is_file() or (ROOT / p).read_bytes() != data]
        if bad:
            raise SystemExit('Abweichende generierte Dateien: ' + ', '.join(bad))
    else:
        for path, data in files.items():
            (ROOT / path).parent.mkdir(parents=True, exist_ok=True)
            (ROOT / path).write_bytes(data)
    print(f'{len(files)} M2-Dateien {"geprüft" if args.check else "geschrieben"}')


if __name__ == '__main__':
    main()

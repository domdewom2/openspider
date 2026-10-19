"""Deterministische PDF-Fixture mit Text und URI-Annotationen, ohne Fremdressourcen."""
from pathlib import Path
objects = [
    b'<< /Type /Catalog /Pages 2 0 R >>',
    b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R /Annots [6 0 R 7 0 R] >>',
    b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
]
text = b'BT /F1 12 Tf 72 720 Td (The cat sat on the mat.) Tj ET'
objects += [b'<< /Length ' + str(len(text)).encode() + b' >>\nstream\n' + text + b'\nendstream',
    b'<< /Type /Annot /Subtype /Link /Rect [72 710 210 735] /A << /S /URI /URI (https://site.test/next) >> >>',
    b'<< /Type /Annot /Subtype /Link /Rect [72 610 210 635] /A << /S /URI /URI (relative.html) >> >>',
    b'<< /Title (Test document) /Keywords (cats) >>']
out = bytearray(b'%PDF-1.5\n')
offsets = [0]
for i, obj in enumerate(objects, 1):
    offsets.append(len(out))
    out += str(i).encode() + b' 0 obj\n' + obj + b'\nendobj\n'
xref = len(out)
out += b'xref\n0 9\n0000000000 65535 f \n'
for offset in offsets[1:]:
    out += f'{offset:010d} 00000 n \n'.encode()
out += b'trailer\n<< /Size 9 /Root 1 0 R /Info 8 0 R >>\nstartxref\n' + str(xref).encode() + b'\n%%EOF\n'
Path(__file__).with_name('text.pdf').write_bytes(out)

"""Deterministische PDF-Fixture `fehler.pdf` (ONPAGE-153/166): Textlayer mit einem Rechtschreibfehler, Sprache im
Dokumentkatalog (`/Lang`), Titel in den Dokumenteigenschaften. Ohne Fremdressourcen; Aufruf: python3 generate_pdf.py"""
from pathlib import Path
text = b'BT /F1 12 Tf 72 720 Td (Das ist ein Rechtschreibfeler.) Tj ET'
objects = [
    b'<< /Type /Catalog /Pages 2 0 R /Lang (de-DE) >>',
    b'<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>',
    b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
    b'<< /Length ' + str(len(text)).encode() + b' >>\nstream\n' + text + b'\nendstream',
    b'<< /Title (Test) >>',
]
out = bytearray(b'%PDF-1.5\n')
offsets = [0]
for i, obj in enumerate(objects, 1):
    offsets.append(len(out))
    out += str(i).encode() + b' 0 obj\n' + obj + b'\nendobj\n'
xref = len(out)
out += f'xref\n0 {len(objects) + 1}\n0000000000 65535 f \n'.encode()
for offset in offsets[1:]:
    out += f'{offset:010d} 00000 n \n'.encode()
out += f'trailer\n<< /Size {len(objects) + 1} /Root 1 0 R /Info {len(objects)} 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode()
Path(__file__).with_name('fehler.pdf').write_bytes(out)

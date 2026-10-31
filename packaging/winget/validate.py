"""Offline validation against the vendored, unmodified Microsoft schemas (MIT)."""
import json
import sys
from pathlib import Path
import jsonschema
import yaml


def validate(directory):
    files = list(Path(directory).glob('*.yaml'))
    assert len(files) == 3, 'Exactly three winget manifests required'
    types = set()
    identity = set()
    for path in files:
        doc = yaml.safe_load(path.read_text(encoding='utf-8'))
        kind = doc['ManifestType']
        schema = json.loads((Path(__file__).parent / 'schemas' / f'{kind}.json').read_text())
        jsonschema.Draft7Validator(schema).validate(doc)
        assert kind not in types, f'Duplicate manifest type: {kind}'
        types.add(kind)
        identity.add((doc['PackageIdentifier'], doc['PackageVersion']))
    assert types == {'installer', 'version', 'defaultLocale'}
    assert len(identity) == 1, 'Mixed package identity/version'
    print('ok: winget schema 1.9.0, identity and document set')


if __name__ == '__main__':
    validate(sys.argv[1])

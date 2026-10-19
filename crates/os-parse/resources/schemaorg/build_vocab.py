#!/usr/bin/env python3
"""Erzeugt `vocabulary.json` aus der offiziellen schema.org-Veröffentlichung (TECH-187).

Aufruf (manuell bei einem Release, nicht im Build):

    python3 -I build_vocab.py <schemaorg-current-https.jsonld> <version> <datum> > vocabulary.json

Quelle: https://schema.org/version/<version>/schemaorg-current-https.jsonld (Kern und `pending`, ohne `attic`).
Übernommen werden nur Namen, Oberklassen, Datentypen sowie Domain und Range der Eigenschaften; Kommentare und
Beschreibungen entfallen. Die Ausgabe ist eine Bearbeitung des Vokabulars und steht wie dieses unter CC BY-SA 3.0
(siehe LICENSE.md in diesem Ordner).
"""

import json
import sys

PREFIX = "schema:"


def ids(value):
    if value is None:
        return []
    if not isinstance(value, list):
        value = [value]
    out = []
    for v in value:
        i = v.get("@id") if isinstance(v, dict) else v
        if isinstance(i, str) and i.startswith(PREFIX):
            out.append(i[len(PREFIX):])
    return out


def types_of(node):
    t = node.get("@type", [])
    return t if isinstance(t, list) else [t]


def main():
    source, version, released = sys.argv[1], sys.argv[2], sys.argv[3]
    with open(source, encoding="utf-8") as f:
        graph = json.load(f)["@graph"]
    classes, datatypes, properties = {}, set(), {}
    for node in graph:
        name = node["@id"]
        if not name.startswith(PREFIX):
            continue
        name = name[len(PREFIX):]
        kinds = types_of(node)
        if "rdfs:Class" in kinds:
            classes[name] = sorted(set(ids(node.get("rdfs:subClassOf"))))
            if "schema:DataType" in kinds:
                datatypes.add(name)
        elif "rdf:Property" in kinds:
            properties[name] = [
                sorted(set(ids(node.get("schema:domainIncludes")))),
                sorted(set(ids(node.get("schema:rangeIncludes")))),
            ]
    # Untertypen eines Datentyps (z. B. Integer, URL) sind ebenfalls Datentypen.
    changed = True
    while changed:
        changed = False
        for name, parents in classes.items():
            if name not in datatypes and any(p in datatypes for p in parents):
                datatypes.add(name)
                changed = True
    out = {
        "version": version,
        "released": released,
        "source": f"https://schema.org/version/{version}/schemaorg-current-https.jsonld",
        "license": "CC BY-SA 3.0 (https://creativecommons.org/licenses/by-sa/3.0/), schema.org",
        "datatypes": sorted(datatypes),
        "types": dict(sorted(classes.items())),
        "properties": dict(sorted(properties.items())),
    }
    # Eine Zeile je Typ und Eigenschaft, damit Versionswechsel als lesbarer Diff erscheinen.
    def line(key, value):
        return json.dumps(key, ensure_ascii=False) + ":" + json.dumps(value, ensure_ascii=False, separators=(",", ":"))

    w = sys.stdout.write
    w("{\n")
    for key in ("version", "released", "source", "license", "datatypes"):
        w(line(key, out[key]) + ",\n")
    for section in ("types", "properties"):
        w(json.dumps(section) + ":{\n")
        entries = list(out[section].items())
        w(",\n".join(line(k, v) for k, v in entries))
        w("\n}" + ("," if section == "types" else "") + "\n")
    w("}\n")


if __name__ == "__main__":
    main()

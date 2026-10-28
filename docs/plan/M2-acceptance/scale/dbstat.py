#!/usr/bin/env python3
"""Summarize a completed crawl's SQLite storage after a TRUNCATE checkpoint.

Usage: python3 dbstat.py crawl.ospider > storage.json
The crawl must be stopped and all other connections closed before running this.
"""
import json
import os
import sqlite3
import sys


def measure(path):
    db = sqlite3.connect(path)
    try:
        busy, remaining, checkpointed = db.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()
        if busy or remaining > 0 or checkpointed > 0:
            raise RuntimeError("TRUNCATE checkpoint did not complete; close other connections")
        objects = []
        for name, kind, table in db.execute(
            "SELECT name, type, tbl_name FROM sqlite_schema "
            "WHERE type IN ('table', 'index') AND name NOT LIKE 'sqlite_%' ORDER BY name"
        ):
            size, unused = db.execute(
                "SELECT coalesce(sum(pgsize), 0), coalesce(sum(unused), 0) FROM dbstat WHERE name = ?",
                (name,),
            ).fetchone()
            count = None
            if kind == "table":
                quoted = '"' + name.replace('"', '""') + '"'
                count = db.execute(f"SELECT count(*) FROM {quoted}").fetchone()[0]
            objects.append(dict(name=name, type=kind, table=table, bytes=size, unused=unused, rows=count))
        urls = next((o["rows"] for o in objects if o["name"] == "urls"), None)
        size = os.path.getsize(path)
        return dict(analysis_state=(db.execute("SELECT value FROM crawl_meta WHERE key='analysis_state'").fetchone() or (None,))[0],
                    analysis_steps=dict(db.execute("SELECT step,state FROM analysis_steps")),
                    unused_bytes=db.execute("SELECT coalesce(sum(unused),0) FROM dbstat").fetchone()[0],
                    file_bytes=size, wal_bytes=os.path.getsize(path + "-wal") if os.path.exists(path + "-wal") else 0,
                    page_size=db.execute("PRAGMA page_size").fetchone()[0],
                    freelist_count=db.execute("PRAGMA freelist_count").fetchone()[0],
                    url_rows=urls, bytes_per_url_row=size / urls if urls else None, objects=objects)
    finally:
        db.close()


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("Usage: dbstat.py crawl.ospider")
    print(json.dumps(measure(sys.argv[1]), indent=2, ensure_ascii=False))

-- Das Wörterbuch speichert jeden Schlüssel nur einmal. Die Sicht bewahrt die bisherige SQL-Schnittstelle.
CREATE TABLE ext_keys (id INTEGER PRIMARY KEY, key TEXT NOT NULL UNIQUE);
INSERT INTO ext_keys(key) SELECT DISTINCT key FROM url_ext ORDER BY key;
CREATE TABLE url_ext_data (
  url_id INTEGER NOT NULL, key_id INTEGER NOT NULL,
  v_int INTEGER, v_real REAL, v_text TEXT, change_seq INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (url_id, key_id)
) WITHOUT ROWID;
-- WITHOUT ROWID lagert Text ab etwa 1 KiB auf fast leere Overflow-Seiten aus.
-- Lange Werte bleiben in einem Rowid-Baum (bis knapp 4 KiB lokal je Zelle).
CREATE TABLE ext_texts (
  id INTEGER PRIMARY KEY, url_id INTEGER NOT NULL, key_id INTEGER NOT NULL,
  value TEXT NOT NULL, UNIQUE(url_id,key_id)
);
INSERT INTO ext_texts(url_id,key_id,value)
SELECT e.url_id,k.id,e.v_text FROM url_ext e JOIN ext_keys k ON k.key=e.key
WHERE length(CAST(e.v_text AS BLOB)) > 768 ORDER BY e.url_id,k.id;
INSERT INTO url_ext_data SELECT e.url_id, k.id, e.v_int, e.v_real, CASE WHEN length(CAST(e.v_text AS BLOB)) > 768 THEN NULL ELSE e.v_text END, e.change_seq
FROM url_ext e JOIN ext_keys k ON k.key=e.key ORDER BY e.url_id, k.id;
DROP TABLE url_ext;
CREATE INDEX url_ext_data_change_seq ON url_ext_data(change_seq);
CREATE VIEW url_ext AS SELECT e.url_id, k.key, e.v_int, e.v_real,
  COALESCE(t.value,e.v_text) AS v_text, e.change_seq
FROM url_ext_data e JOIN ext_keys k ON k.id=e.key_id
LEFT JOIN ext_texts t ON t.url_id=e.url_id AND t.key_id=e.key_id;
CREATE TRIGGER url_ext_data_delete AFTER DELETE ON url_ext_data BEGIN
  DELETE FROM ext_texts WHERE url_id=OLD.url_id AND key_id=OLD.key_id;
END;
CREATE TRIGGER url_ext_insert INSTEAD OF INSERT ON url_ext BEGIN
  INSERT INTO ext_keys(key) SELECT NEW.key WHERE NOT EXISTS (SELECT 1 FROM ext_keys WHERE key=NEW.key);
  DELETE FROM ext_texts WHERE url_id=NEW.url_id AND key_id=(SELECT id FROM ext_keys WHERE key=NEW.key);
  INSERT INTO url_ext_data(url_id,key_id,v_int,v_real,v_text,change_seq)
  VALUES (NEW.url_id,(SELECT id FROM ext_keys WHERE key=NEW.key),NEW.v_int,NEW.v_real,CASE WHEN length(CAST(NEW.v_text AS BLOB)) > 768 THEN NULL ELSE NEW.v_text END,COALESCE(NEW.change_seq,0));
  INSERT INTO ext_texts(url_id,key_id,value)
  SELECT NEW.url_id,(SELECT id FROM ext_keys WHERE key=NEW.key),NEW.v_text
  WHERE length(CAST(NEW.v_text AS BLOB)) > 768;
END;
CREATE TRIGGER url_ext_update INSTEAD OF UPDATE ON url_ext BEGIN
  INSERT INTO ext_keys(key) SELECT NEW.key WHERE NOT EXISTS (SELECT 1 FROM ext_keys WHERE key=NEW.key);
  DELETE FROM ext_texts WHERE url_id=OLD.url_id AND key_id=(SELECT id FROM ext_keys WHERE key=OLD.key);
  UPDATE url_ext_data SET url_id=NEW.url_id,key_id=(SELECT id FROM ext_keys WHERE key=NEW.key),
    v_int=NEW.v_int,v_real=NEW.v_real,
    v_text=CASE WHEN length(CAST(NEW.v_text AS BLOB)) > 768 THEN NULL ELSE NEW.v_text END,
    change_seq=NEW.change_seq
    WHERE url_id=OLD.url_id AND key_id=(SELECT id FROM ext_keys WHERE key=OLD.key);
  INSERT INTO ext_texts(url_id,key_id,value)
  SELECT NEW.url_id,(SELECT id FROM ext_keys WHERE key=NEW.key),NEW.v_text
  WHERE length(CAST(NEW.v_text AS BLOB)) > 768;
END;
CREATE TRIGGER url_ext_delete INSTEAD OF DELETE ON url_ext BEGIN
  DELETE FROM url_ext_data WHERE url_id=OLD.url_id AND key_id=(SELECT id FROM ext_keys WHERE key=OLD.key);
END;

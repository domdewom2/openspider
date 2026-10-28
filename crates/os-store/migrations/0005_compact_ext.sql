-- Das Wörterbuch speichert jeden Schlüssel nur einmal. Die Sicht bewahrt die bisherige SQL-Schnittstelle.
CREATE TABLE ext_keys (id INTEGER PRIMARY KEY, key TEXT NOT NULL UNIQUE);
INSERT INTO ext_keys(key) SELECT DISTINCT key FROM url_ext ORDER BY key;
CREATE TABLE url_ext_data (
  url_id INTEGER NOT NULL, key_id INTEGER NOT NULL,
  v_int INTEGER, v_real REAL, v_text TEXT, change_seq INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (url_id, key_id)
) WITHOUT ROWID;
INSERT INTO url_ext_data SELECT e.url_id, k.id, e.v_int, e.v_real, e.v_text, e.change_seq
FROM url_ext e JOIN ext_keys k ON k.key=e.key ORDER BY e.url_id, k.id;
DROP TABLE url_ext;
CREATE INDEX url_ext_data_change_seq ON url_ext_data(change_seq);
CREATE VIEW url_ext AS SELECT e.url_id, k.key, e.v_int, e.v_real, e.v_text, e.change_seq
FROM url_ext_data e JOIN ext_keys k ON k.id=e.key_id;
CREATE TRIGGER url_ext_insert INSTEAD OF INSERT ON url_ext BEGIN
  INSERT OR IGNORE INTO ext_keys(key) VALUES (NEW.key);
  INSERT INTO url_ext_data(url_id,key_id,v_int,v_real,v_text,change_seq)
  VALUES (NEW.url_id,(SELECT id FROM ext_keys WHERE key=NEW.key),NEW.v_int,NEW.v_real,NEW.v_text,COALESCE(NEW.change_seq,0));
END;
CREATE TRIGGER url_ext_update INSTEAD OF UPDATE ON url_ext BEGIN
  INSERT OR IGNORE INTO ext_keys(key) VALUES (NEW.key);
  UPDATE url_ext_data SET url_id=NEW.url_id,key_id=(SELECT id FROM ext_keys WHERE key=NEW.key),
    v_int=NEW.v_int,v_real=NEW.v_real,v_text=NEW.v_text,change_seq=NEW.change_seq
    WHERE url_id=OLD.url_id AND key_id=(SELECT id FROM ext_keys WHERE key=OLD.key);
END;
CREATE TRIGGER url_ext_delete INSTEAD OF DELETE ON url_ext BEGIN
  DELETE FROM url_ext_data WHERE url_id=OLD.url_id AND key_id=(SELECT id FROM ext_keys WHERE key=OLD.key);
END;

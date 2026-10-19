CREATE TABLE urls(id INTEGER PRIMARY KEY, address TEXT, status_code INTEGER);
INSERT INTO urls VALUES (1, 'https://example.test/', 200), (2, 'https://example.test/missing', 404), (3, 'https://example.test/old', 301);
CREATE VIEW q_urls AS SELECT id AS url_id, address, status_code FROM urls;

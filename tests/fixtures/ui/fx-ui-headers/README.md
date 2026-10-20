# Web export fixture (M2-D18)

Clean-room inputs for OUT-045/046/048/049/110/111 and ONPAGE-165.
`index.html` and `scenario.toml` test a real crawl, source compression and text selection.
`data.sql` supplies twelve fetched URLs and a queued URL, mixed header casing,
repeated headers and cookies, credentials, session/persistent cookies, sibling
subdomains and third-party domains. HTTP and JavaScript cookie origins are distinct.
Golden CSV files document complete outputs; `index.csv` files refer to stable URL-ID filenames.

// CI only: serve downloaded draft assets on an ephemeral loopback port.
import { createServer } from 'node:http';
import { readFileSync, writeFileSync, realpathSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
const [directory, ready] = process.argv.slice(2);
const root = realpathSync(directory);
createServer((req, res) => {
  try {
    const name = decodeURIComponent(new URL(req.url, 'http://127.0.0.1').pathname).slice(1);
    const path = realpathSync(resolve(root, name));
    if (dirname(path) !== root) throw Error('Not an asset');
    const data = readFileSync(path);
    res.writeHead(200, { 'Content-Length': data.length, 'Content-Type': 'application/octet-stream' });
    res.end(data);
  } catch { res.writeHead(404); res.end(); }
}).listen(0, '127.0.0.1', function () {
  writeFileSync(ready, `http://127.0.0.1:${this.address().port}`);
});

import { strict as assert } from "node:assert";
import { test } from "node:test";
import { summarizeMappings } from "./n23-provenance.ts";

// spec: UI-144
test("kernel smaps fixture retains anonymous and shared-file RSS without double counting PSS", () => {
  const fixture = `1000-2000 rw-p 00000000 00:00 0
Rss:                   8 kB
Pss:                   8 kB
Private_Dirty:         8 kB
2000-3000 r--p 00000000 08:01 42 /fixture/shared library.so
Rss:                  12 kB
Pss:                   3 kB
Private_Dirty:         0 kB
3000-4000 r-xp 00001000 08:01 42 /fixture/shared library.so
Rss:                   4 kB
Pss:                   1 kB
Private_Dirty:         0 kB
`;
  const result = summarizeMappings(fixture);
  assert.equal(result.rssBytes, 24 * 1024);
  assert.equal(result.pssBytes, 12 * 1024);
  assert.equal(result.privateDirtyBytes, 8 * 1024);
  assert.deepEqual(result.mappings.map(m => [m.name, m.rssBytes]), [
    ["/fixture/shared library.so", 16 * 1024], ["[anonymous]", 8 * 1024],
  ]);
});

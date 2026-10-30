import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { allowedExpression, validate, rustEntries, licenseFiles, renderNotices } from './generate.mjs';

const allowed = ['MIT', 'Apache-2.0', 'BSD-3-Clause', 'Apache-2.0 WITH LLVM-exception'];
test('policy: OR, AND, exceptions, unknown and malformed SPDX fail closed', () => {
  for (const expression of ['MIT', '(MIT OR GPL-3.0-only)', 'MIT AND Apache-2.0', 'Apache-2.0 WITH LLVM-exception']) assert.equal(allowedExpression(expression, allowed), true, expression);
  for (const expression of ['', 'Unknown', 'MIT AND GPL-3.0-only', 'MIT nonsense', '(MIT', 'MIT)', 'MIT OR', 'MIT WITH', 'MIT OR (GPL-3.0-only', 'Apache-2.0 WITH unknown']) assert.equal(allowedExpression(expression, allowed), false, expression);
});
test('missing metadata, forbidden license and empty text fail the build', () => {
  const good = { name: 'fixture', version: '1.2.3', license: 'MIT', text: 'Copyright fixture\nPermission is hereby granted' };
  assert.equal(validate(good, allowed), good);
  for (const change of [{ name: '' }, { version: '' }, { license: '' }, { license: 'GPL-3.0-only' }, { text: ' \n' }]) assert.throws(() => validate({ ...good, ...change }, allowed));
});
test('every external Rust crate needs its own sourced text; canonical fallback is rejected', () => {
  const pkg = { id: 'fixture@1', name: 'fixture', version: '1', source: 'registry' };
  const data = { crates: [{ package: pkg, license: 'MIT' }], licenses: [{ id: 'MIT', source_path: 'LICENSE', text: 'Copyright fixture\nPermission is hereby granted', used_by: [{ crate: pkg }] }] };
  assert.equal(rustEntries(data, allowed, { 'fixture@1': 'Copyright fixture\nPermission is hereby granted' })[0].name, 'fixture');
  assert.throws(() => rustEntries(data, allowed));
  assert.throws(() => rustEntries({ ...data, licenses: [] }, allowed));
  assert.throws(() => rustEntries({ ...data, licenses: [{ ...data.licenses[0], source_path: null }] }, allowed));
});
test('npm LICENSE and NOTICE are preserved; empty license files are rejected', () => {
  const directory = mkdtempSync(join(tmpdir(), 'l01-test-'));
  try {
    writeFileSync(join(directory, 'LICENSE'), 'Copyright fixture\nPermission is hereby granted');
    writeFileSync(join(directory, 'NOTICE.txt'), 'Additional attribution');
    const text = licenseFiles(directory);
    assert.match(text, /Copyright fixture/); assert.match(text, /Additional attribution/);
    writeFileSync(join(directory, 'LICENSE'), ' ');
    assert.throws(() => licenseFiles(directory));
  } finally { rmSync(directory, { recursive: true }); }
});
test('output contains name, version, expression and complete text in deterministic order', () => {
  const entries = ['z', 'a'].map((name) => ({ ecosystem: 'npm', name, version: '1.2', license: 'MIT', text: 'Copyright fixture\nPermission is hereby granted\nFull conditions' }));
  assert.equal(renderNotices(entries), renderNotices([...entries].reverse()));
  assert.match(renderNotices(entries), /Copyright fixture\nPermission is hereby granted\nFull conditions/);
  assert.equal((renderNotices(entries).match(/Full conditions/g) ?? []).length, 1);
  assert.match(renderNotices(entries), /npm: a\nVersion: 1.2\nLicense: MIT\nLicense texts: L0001/);
});
test('pinned upstream source rejects changed bytes', async () => {
  const { verifiedSourceText } = await import('./generate.mjs');
  const { createHash } = await import('node:crypto');
  const bytes = Buffer.from('Copyright fixture\nMIT conditions');
  const source = { url: 'https://example.invalid/pinned/LICENSE', sha256: createHash('sha256').update(bytes).digest('hex') };
  assert.match(verifiedSourceText(bytes, source), /Copyright fixture/);
  assert.throws(() => verifiedSourceText(Buffer.from('changed'), source));
});

test('canonical terms require exact reviewed version, expression and author attribution', () => {
  const pkg = { id: 'fixture@1', name: 'fixture', version: '1', source: 'registry', license: 'MIT', authors: ['Fixture Author'] };
  const data = { crates: [{ package: pkg, license: 'MIT' }], licenses: [{ id: 'MIT', source_path: null, text: 'Full canonical MIT terms', used_by: [{ crate: pkg }] }] };
  assert.match(rustEntries(data, allowed, {}, { 'fixture@1': { license: 'MIT', reason: 'fixture review' } })[0].attribution, /Fixture Author/);
  assert.throws(() => rustEntries(data, allowed, {}, { 'fixture@2': { license: 'MIT' } }));
  assert.throws(() => rustEntries(data, allowed, {}, { 'fixture@1': { license: 'Apache-2.0' } }));
});

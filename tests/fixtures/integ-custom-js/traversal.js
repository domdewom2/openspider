for (const path of ['a/.. /escape', 'a/.../escape', 'a/trailing./escape', 'a/trailing /escape']) {
  let rejected = false;
  try { await openspider.saveText('escape', path); } catch { rejected = true; }
  if (!rejected) throw new Error('Unsafe output path accepted');
}
await openspider.saveText('escape', '../../x');

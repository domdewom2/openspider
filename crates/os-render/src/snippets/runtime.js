(async () => {
  const tagged = new WeakMap();
  const api = Object.freeze({
    data(value) { const token = {}; tagged.set(token, {value}); return token; },
    error(message) { const token = {}; tagged.set(token, {error: String(message)}); return token; },
    // No host file bindings in P1. In particular, traversal can never reach disk.
    saveText() { throw new Error('File access is not enabled'); },
    saveUrls() { throw new Error('File access is not enabled'); }
  });
  Object.defineProperty(window, 'openspider', {value: api, configurable: true});
  Object.defineProperty(window, 'seoSpider', {value: api, configurable: true});
  try {
    const result = await (async function(openspider, seoSpider) {
      /* USER_CODE */
    }).call(window, api, api);
    if (/* ACTION */) return {values: [], error: null};
    const wrapped = result && typeof result === 'object' && tagged.get(result);
    if (wrapped && 'error' in wrapped) return {values: [], error: wrapped.error};
    if (result === undefined) return {values: [], error: 'Snippet returned no value'};
    const value = wrapped ? wrapped.value : result;
    const values = Array.isArray(value) ? value : [value];
    return {values: values.map(v => v == null ? null : typeof v === 'object' ? JSON.stringify(v) : String(v)), error: null};
  } catch (error) {
    return {values: [], error: String(error && error.message || error)};
  }
})()

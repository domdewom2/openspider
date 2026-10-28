(async () => {
  const tagged = new WeakMap();
  const files = [];
  function path(value) {
    if (!/* FILE_ACCESS */) throw new Error('File access is not enabled');
    const normalized = String(value).replaceAll('\\', '/');
    if (!normalized || normalized.startsWith('/') || normalized.includes(':') ||
        normalized.split('/').some(p => p === '..' || (p !== '.' && (p.endsWith('.') || p.endsWith(' ')))) || normalized.includes('\0')) {
      throw new Error('Invalid output path');
    }
    const parts = normalized.split('/').filter(p => p && p !== '.');
    if (!parts.length) throw new Error('Invalid output path');
    return parts.join('/');
  }
  const api = Object.freeze({
    data(value) { const token = {}; tagged.set(token, {value}); return token; },
    error(message) { const token = {}; tagged.set(token, {error: String(message)}); return token; },
    async saveText(text, destination, append = false) {
      files.push({path: path(destination), bytes: Array.from(new TextEncoder().encode(String(text))), append: !!append});
    },
    async saveUrls(urls, directory) {
      const root = path(directory);
      for (const address of urls) {
        const url = new URL(address, location.href);
        if (!['http:', 'https:'].includes(url.protocol)) throw new Error('Invalid download URL');
        const destination = path(root + '/' + url.hostname + '/' + decodeURIComponent(url.pathname));
        const response = await fetch(url.href);
        if (!response.ok) throw new Error('Download failed: ' + response.status);
        files.push({path: destination, bytes: Array.from(new Uint8Array(await response.arrayBuffer())), append: false});
      }
    }
  });
  Object.defineProperty(window, 'openspider', {value: api, configurable: true});
  Object.defineProperty(window, 'seoSpider', {value: api, configurable: true});
  const run = async () => {
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
  };
  return {...await run(), files};
})()

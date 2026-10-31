await openspider.saveText('first\n', './nested//text.txt');
await openspider.saveText('second\n', 'nested/text.txt', true);
await openspider.saveUrls([location.origin + '/api'], 'downloads');
return openspider.data(typeof window.__TAURI__);

#!/usr/bin/env node
// Dependency-free checks for the static website; run: node website/check-site.mjs
import {readFile, readdir, stat} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.resolve(fileURLToPath(new URL('./site/',import.meta.url)));
const origin='https://openspider.de';
const errors=[];
const fail=(file,message)=>errors.push(`${file}: ${message}`);
async function walk(dir){const result=[];for(const e of await readdir(dir,{withFileTypes:true})){const p=path.join(dir,e.name);result.push(...(e.isDirectory()?await walk(p):[p]));}return result;}
const files=await walk(root);
const texts=new Map();
for(const f of files.filter(f=>/\.(html|css|svg|xml|txt)$/.test(f)))texts.set(f,await readFile(f,'utf8'));
function attrs(s){return Object.fromEntries([...s.matchAll(/([^\s=<>/]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/g)].map(m=>[m[1].toLowerCase(),m[2]??m[3]??m[4]]));}
function tags(text){return [...text.replace(/<!--[\s\S]*?-->/g,'').matchAll(/<([a-z][\w:-]*)\b([^>]*?)>/gi)].map(m=>({name:m[1].toLowerCase(),a:attrs(m[2])}));}
const pageURL=f=>origin+'/'+path.relative(root,f).split(path.sep).join('/').replace(/index\.html$/,'');
async function resolve(ref,file,fragment=true){
    if(ref.startsWith('data:'))return null;
    let url;try{url=new URL(ref,pageURL(file));}catch{fail(file,`invalid URL ${ref}`);return null;}
    if(url.origin!==origin)return null;
    let p;try{p=decodeURIComponent(url.pathname);}catch{fail(file,`invalid URL encoding ${ref}`);return null;}
    let target=path.resolve(root,'.'+p);
    if(target!==root&&!target.startsWith(root+path.sep)){fail(file,`path escapes site: ${ref}`);return null;}
    try{if((await stat(target)).isDirectory())target=path.join(target,'index.html');await stat(target);}catch{fail(file,`missing target ${ref}`);return null;}
    if(fragment&&url.hash){const ids=tags(texts.get(target)??'').map(t=>t.a.id);if(!ids.includes(decodeURIComponent(url.hash.slice(1))))fail(file,`missing fragment ${ref}`);}
    return target;
}
function checkExternal(ref,file,isLink=false){
    if(ref.startsWith('data:')){if(!ref.startsWith('data:image/'))fail(file,'unexpected inline resource');return;}
    let url;try{url=new URL(ref,origin);}catch{fail(file,`invalid resource ${ref}`);return;}
    if(url.origin===origin)return;
    if(isLink&&url.protocol==='https:'&&url.hostname==='github.com')return;
    fail(file,`external resource or disallowed link ${ref}`);
}
const titles=new Set(),descriptions=new Set(),pages=new Map();
for(const [file,text] of texts){
    const ts=tags(text);
    for(const t of ts){
        for(const attr of ['href','src','poster','action','data'])if(t.a[attr]){
            // SVG namespace declarations are not resource requests.
            const ref=t.a[attr];checkExternal(ref,file,t.name==='a'&&attr==='href');await resolve(ref,file);
        }
        if(t.a.srcset)for(const candidate of t.a.srcset.split(',')){const ref=candidate.trim().split(/\s+/)[0];checkExternal(ref,file);await resolve(ref,file);}
        if(t.a.style&&/url\s*\(|@import/i.test(t.a.style))fail(file,'put resource styles in checked CSS');
        if(t.name==='img'&&!Object.hasOwn(t.a,'alt'))fail(file,'img missing alt');
        if(t.name==='script'&&t.a.type!=='application/ld+json')fail(file,'unexpected executable script');
    }
    if(file.endsWith('.css')){
        if(/@import/i.test(text))fail(file,'CSS @import is not allowed');
        for(const m of text.matchAll(/url\(\s*['"]?([^)'"\s]+)['"]?\s*\)/gi)){checkExternal(m[1],file);await resolve(m[1],file);}
    }
    if(!file.endsWith('.html'))continue;
    const metas=ts.filter(t=>t.name==='meta').map(t=>t.a);
    const title=[...text.matchAll(/<title>([^<]+)<\/title>/g)];
    const ds=metas.filter(a=>a.name==='description'&&a.content?.trim());
    if(title.length!==1||!title[0][1].trim())fail(file,'expected one nonempty title');
    else if(titles.has(title[0][1]))fail(file,'duplicate title');else titles.add(title[0][1]);
    if(ds.length!==1)fail(file,'expected one description');else if(descriptions.has(ds[0].content))fail(file,'duplicate description');else descriptions.add(ds[0].content);
    const canon=ts.filter(t=>t.name==='link'&&t.a.rel==='canonical');
    if(canon.length!==1||canon[0].a.href!==pageURL(file))fail(file,'canonical must match page URL');
    if(ts.filter(t=>t.name==='h1').length!==1)fail(file,'expected exactly one h1');
    const lang=ts.find(t=>t.name==='html')?.a.lang;
    if(!['de','en'].includes(lang))fail(file,'expected de/en lang');
    const visible=text.replace(/<!--[\s\S]*?-->/g,'');
    if(/TODO\(|\b(?:OUT-\d+|M3-[A-Z0-9]+|A00)\b|Projektverantwortlich|project owner/i.test(visible))fail(file,'internal placeholder or project identifier on public page');
    const guide=lang==='en'?'/en/manual/':'/handbuch/';
    const footer=text.match(/<footer\b[^>]*>([\s\S]*?)<\/footer>/)?.[1]??'';
    if(!tags(footer).some(t=>t.name==='a'&&t.a.href===guide))fail(file,'footer missing user guide');
    if(['/download/','/en/download/'].includes(new URL(pageURL(file)).pathname)){
        const header=text.match(/<header\b[^>]*>([\s\S]*?)<\/header>/)?.[1]??'';
        const current=new URL(pageURL(file)).pathname;
        if(!tags(header).some(t=>t.name==='a'&&t.a.href===current&&t.a['aria-current']==='page'))fail(file,'download navigation missing aria-current');
        if(ts.some(t=>t.name==='a'&&t.a.href?.includes('VERSION')))fail(file,'download URL templates must be plain text');
    }
    const alternates=ts.filter(t=>t.name==='link'&&t.a.rel==='alternate');
    for(const l of ['de','en'])if(alternates.filter(t=>t.a.hreflang===l).length!==1)fail(file,`expected one hreflang ${l}`);
    if(alternates.find(t=>t.a.hreflang===lang)?.a.href!==pageURL(file))fail(file,'hreflang self-reference missing');
    const other=lang==='de'?'en':'de';
    if(!ts.some(t=>t.name==='a'&&t.a.hreflang===other&&t.a.href===new URL(alternates.find(t=>t.a.hreflang===other)?.a.href??origin).pathname))fail(file,'missing paired language switch');
    if(!ts.some(t=>t.name==='a'&&t.a.href==='#main'))fail(file,'missing skip link');
    let level=0;for(const t of ts.filter(t=>/^h[1-6]$/.test(t.name))){const n=+t.name[1];if(n>level+1)fail(file,'heading level skipped');level=n;}
    for(const name of ['og:title','og:description','og:url','og:image','og:image:alt','twitter:card','twitter:title','twitter:description','twitter:image'])if(!metas.some(a=>(a.property===name||a.name===name)&&a.content))fail(file,`missing ${name}`);
    for(const m of metas.filter(m=>['og:image','twitter:image'].includes(m.property??m.name))){checkExternal(m.content,file);await resolve(m.content,file);}
    for(const m of text.matchAll(/<script\b[^>]*type="application\/ld\+json"[^>]*>([\s\S]*?)<\/script>/g))try{const s=JSON.parse(m[1]);if(s['@type']!=='SoftwareApplication'||s.name!=='OpenSpider'||s['@context']!=='https://schema.org')fail(file,'unexpected structured data');}catch{fail(file,'invalid JSON-LD');}
    if(['/','/en/','/download/','/en/download/','/funktionen/','/en/features/'].includes(new URL(pageURL(file)).pathname)&&!text.includes('application/ld+json'))fail(file,'missing SoftwareApplication');
    pages.set(pageURL(file),{file,alternates,lang});
}
for(const [url,p] of pages)for(const a of p.alternates.filter(t=>['de','en'].includes(t.a.hreflang))){const q=pages.get(a.a.href);if(!q||q.lang!==a.a.hreflang||!q.alternates.some(t=>t.a.hreflang===p.lang&&t.a.href===url))fail(p.file,'hreflang pair not reciprocal');}
const sm=texts.get(path.join(root,'sitemap.xml'))??'';
for(const m of sm.matchAll(/<loc>([^<]+)<\/loc>/g)){if(!pages.has(m[1])||m[1].endsWith('404.html'))fail('sitemap.xml',`invalid page ${m[1]}`);}
for(const url of pages.keys())if(!url.endsWith('404.html')&&!sm.includes(`<loc>${url}</loc>`))fail('sitemap.xml',`missing ${url}`);
if(!(texts.get(path.join(root,'robots.txt'))??'').includes('Sitemap: '+origin+'/sitemap.xml'))fail('robots.txt','missing sitemap');
const og=await readFile(path.join(root,'assets/og.png'));
if(og.toString('hex',0,8)!=='89504e470d0a1a0a'||og.readUInt32BE(16)!==1200||og.readUInt32BE(20)!==630)fail('og.png','expected 1200×630 PNG');
// Count uncompressed HTML, CSS and every referenced image/icon once. Social preview
// metadata is not downloaded during a page visit and is deliberately excluded.
for(const home of ['index.html','en/index.html']){
    const seen=new Set();
    async function collect(file){if(seen.has(file))return;seen.add(file);const text=texts.get(file)??'';
        for(const t of tags(text)){const ref=t.a.src??(t.name==='link'&&['stylesheet','icon','apple-touch-icon'].includes(t.a.rel)?t.a.href:null);if(ref){const target=await resolve(ref,file,false);if(target)await collect(target);}}
        for(const m of text.matchAll(/url\(\s*['"]?([^)'"\s]+)['"]?\s*\)/gi)){const target=await resolve(m[1],file,false);if(target)await collect(target);}
    }
    await collect(path.join(root,home));let bytes=0;for(const f of seen)bytes+=(await stat(f)).size;
    console.log(`${home}: ${bytes.toLocaleString('en')} bytes (uncompressed, including icons)`);
    if(bytes>=200_000)fail(home,'homepage must be below 200 KB');
}
if(errors.length){console.error(errors.join('\n'));process.exitCode=1;}else console.log(`OK: ${pages.size} pages; links, assets, metadata, languages, headings, alt text and homepage weight.`);

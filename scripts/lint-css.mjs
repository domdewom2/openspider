// K2-75a/K2-77: Feature CSS uses central design tokens and a four-pixel grid.
import { readdirSync, readFileSync } from 'node:fs';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
function walk(dir) { return readdirSync(dir, { withFileTypes: true }).flatMap(e => e.isDirectory() ? walk(resolve(dir, e.name)) : e.name.endsWith('.css') ? [resolve(dir, e.name)] : []); }
const failures = [];
const tokenCss = readFileSync(resolve(root, 'apps/desktop/src/ui/tokens/base.css'), 'utf8');
for (const m of tokenCss.matchAll(/--os-(?:size|space)-[\w-]+:\s*([\d.]+)px/g)) {
  if (Number(m[1]) % 4) failures.push(`ui/tokens/base.css: ${m[0]} outside 4-px grid`);
}
for (const file of walk(resolve(root, 'apps/desktop/src/features'))) {
  // Compare is explicitly outside U35 ownership; tracked exception in u35.md.
  if (relative(root, file).includes('/compare/')) continue;
  let css = readFileSync(file, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  // CSS custom properties are unsupported in media/container conditions: literal breakpoints must still follow the grid.
  css = css.replace(/@(media|container)[^{]+/g, rule => {
    for (const m of rule.matchAll(/([\d.]+)px/g)) if (Number(m[1]) % 4) failures.push(`${relative(root, file)}: breakpoint ${m[0]} outside grid`);
    return '';
  });
  for (const m of css.matchAll(/-?[\d.]+(?:px|rem|em)\b|#[\da-f]{3,8}\b|\b(?:rgb|hsl)a?\([^)]*\)|(?<=:\s)(?:white|black|gray|red|blue|green|yellow)\b/gi)) failures.push(`${relative(root, file)}: literal ${m[0]}; use ui/tokens`);
  for (const m of css.matchAll(/--os-[\w-]+\s*:/g)) failures.push(`${relative(root, file)}: local token definition ${m[0]}`);
}
if (failures.length) { console.error(failures.join('\n')); process.exitCode = 1; }
else console.log('CSS: feature tokens and 4-px breakpoints OK (compare excluded, u35.md).');

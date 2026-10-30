// K2-75a/K2-77: Feature CSS uses central design tokens and a four-pixel grid.
import { readdirSync, readFileSync } from 'node:fs';
import { resolve, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = resolve(fileURLToPath(new URL('..', import.meta.url)));
function walk(dir) { return readdirSync(dir, { withFileTypes: true }).flatMap(e => e.isDirectory() ? walk(resolve(dir, e.name)) : e.name.endsWith('.css') ? [resolve(dir, e.name)] : []); }
// Standard named colours are literals too; transparent/currentColor/system colours are semantic exceptions.
const namedColours = new Set(`aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen`.split(/\s+/));
const failures = [];
const tokenCss = readFileSync(resolve(root, 'apps/desktop/src/ui/tokens/base.css'), 'utf8');
for (const m of tokenCss.matchAll(/--os-(?:size|space)-[\w-]+:\s*([\d.]+)px/g)) {
  if (Number(m[1]) % 4) failures.push(`ui/tokens/base.css: ${m[0]} outside 4-px grid`);
}
const featureRoot = resolve(root, 'apps/desktop/src/features');
for (const file of walk(featureRoot)) {
  // Compare is explicitly outside U35 ownership; tracked exception in u35.md.
  if (relative(featureRoot, file).split(sep)[0] === 'compare') continue;
  let css = readFileSync(file, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  // CSS custom properties are unsupported in media/container conditions: literal breakpoints must still follow the grid.
  css = css.replace(/@(media|container)[^{]+/g, rule => {
    for (const m of rule.matchAll(/([\d.]+)px/g)) if (Number(m[1]) % 4) failures.push(`${relative(root, file)}: breakpoint ${m[0]} outside grid`);
    return '';
  });
  for (const m of css.matchAll(/-?[\d.]+(?:px|rem|em)\b|#[\da-f]{3,8}\b|\b(?:rgba?|hsla?|oklab|oklch|lab|lch|color)\([^)]*\)|(?<![\w-])(?:white|black|gray|red|blue|green|yellow)(?![\w-])/gi)) failures.push(`${relative(root, file)}: literal ${m[0]}; use ui/tokens`);
  for (const m of css.matchAll(/(?:gap|padding|margin)[\w-]*:[^;{}]*var\(--os-(?:line|focus-width)\)/g)) failures.push(`${relative(root, file)}: stroke token used as spacing: ${m[0]}`);
  for (const m of css.matchAll(/(?<![\w-])[a-z]+(?![\w-])/gi)) if (namedColours.has(m[0].toLowerCase())) failures.push(`${relative(root, file)}: named colour ${m[0]}; use ui/tokens`);
  for (const m of css.matchAll(/--os-[\w-]+\s*:/g)) failures.push(`${relative(root, file)}: local token definition ${m[0]}`);
}
if (failures.length) { console.error(failures.join('\n')); process.exitCode = 1; }
else console.log('CSS: feature tokens and 4-px breakpoints OK (compare excluded, u35.md).');

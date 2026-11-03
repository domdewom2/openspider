// WebGL2-Zeichnung der 3D-Graphen (UI-098, UI-146): alle Knoten in einem Zeichenaufruf als Punkte (Kreis im
// Fragment-Shader, Größe mit Perspektive), Baumkanten und eingeblendete Links (UI-097) als Linien aus Indexpuffern.
// Keine Bibliothek (M3-V02, Lizenzprüfung): nur WebGL2 der WebView (WKWebView ab Safari 15, WebView2).

export interface GlColors {
  backgroundCenter: [number, number, number];
  backgroundEdge: [number, number, number];
  edge: [number, number, number, number];
  inlink: [number, number, number, number];
  outlink: [number, number, number, number];
  halo: [number, number, number, number];
}

export interface RenderOptions3D {
  dimEdges: boolean;
  /** Tiefen-Abblendung von `fogNear` (voll) bis `fogFar` (stark abgeblendet), Kameraabstand in Welteinheiten. */
  fogNear: number;
  fogFar: number;
  /** Grenzen der Punktgröße in Bildpunkten des Zeichenpuffers (Ober-/Untergrenze trotz Perspektive). */
  minPx: number;
  maxPx: number;
}

// Kugeln als Impostor: Normale aus der Punktkoordinate, Licht von oben links, weiches Glanzlicht und dezenter Rand;
// Größe mit Perspektive, aber auf [u_minPx, u_maxPx] begrenzt; Tiefen-Abblendung als Alpha (Nebel zum Hintergrund).
// Ausgabe vormultipliziert (Zeichenfläche mit Alpha über dem CSS-Hintergrundverlauf).
const POINT_VS = `#version 300 es
in vec3 a_pos;
in float a_size;
in vec4 a_color;
uniform mat4 u_view;
uniform mat4 u_proj;
uniform float u_scale;
uniform float u_minPx;
uniform float u_maxPx;
uniform float u_fogNear;
uniform float u_fogFar;
uniform float u_halo;
out vec4 v_color;
out float v_px;
void main() {
  vec4 mv = u_view * vec4(a_pos, 1.0);
  gl_Position = u_proj * mv;
  float depth = max(-mv.z, 0.0001);
  float px = clamp(2.0 * a_size * u_scale / depth, u_minPx, u_maxPx);
  gl_PointSize = u_halo > 0.5 ? px * 2.2 + 6.0 : px;
  // Tiefen-Abblendung im Alpha (ein Varying weniger je Bildpunkt).
  v_color = vec4(a_color.rgb, a_color.a * (1.0 - 0.72 * smoothstep(u_fogNear, u_fogFar, depth)));
  v_px = gl_PointSize;
}`;

const POINT_FS = `#version 300 es
precision highp float;
in vec4 v_color;
in float v_px;
uniform float u_halo;
uniform vec4 u_haloColor;
out vec4 o_color;
void main() {
  vec2 c = gl_PointCoord * 2.0 - 1.0;
  float r2 = dot(c, c);
  if (r2 > 1.0) discard;
  float r = sqrt(r2);
  if (u_halo > 0.5) {
    float a = u_haloColor.a * (1.0 - smoothstep(0.38, 1.0, r));
    o_color = vec4(u_haloColor.rgb * a, a);
    return;
  }
  // Kleine Punkte (bis 6 px): flache Farbe – die Schattierung wäre kaum sichtbar und kostet nur Rechenzeit.
  if (v_px < 6.0) {
    float fa = v_color.a;
    o_color = vec4(v_color.rgb * 0.92 * fa, fa);
    return;
  }
  vec3 n = vec3(c.x, -c.y, sqrt(1.0 - r2));
  vec3 l = normalize(vec3(-0.5, 0.62, 0.6));
  float diffuse = max(dot(n, l), 0.0);
  float s1 = max(dot(reflect(-l, n), vec3(0.0, 0.0, 1.0)), 0.0);
  float s4 = s1 * s1 * s1 * s1;
  float spec = s4 * s4 * s4 * s4 * s4 * s4 * s4;
  vec3 col = v_color.rgb * (0.48 + 0.62 * diffuse) + vec3(0.22 * spec);
  col = mix(col, v_color.rgb * 0.62, 0.55 * smoothstep(0.8, 1.0, r));
  float edge = 1.0 - smoothstep(1.0 - 2.5 / max(v_px, 2.5), 1.0, r);
  float a = v_color.a * edge;
  o_color = vec4(col * a, a);
}`;

const LINE_VS = `#version 300 es
in vec3 a_pos;
uniform mat4 u_view;
uniform mat4 u_proj;
uniform float u_fogNear;
uniform float u_fogFar;
out float v_fog;
void main() {
  vec4 mv = u_view * vec4(a_pos, 1.0);
  gl_Position = u_proj * mv;
  v_fog = 1.0 - 0.85 * smoothstep(u_fogNear, u_fogFar, -mv.z);
}`;

const LINE_FS = `#version 300 es
precision mediump float;
uniform vec4 u_color;
in float v_fog;
out vec4 o_color;
void main() {
  float a = u_color.a * v_fog;
  o_color = vec4(u_color.rgb * a, a);
}`;

// Linien ohne Tiefen-Abblendung für sehr viele Kanten (ab FLAT_LINES_FROM): die interpolierte Abblendung kostet je
// Bildpunkt Rechenzeit; bei so dichten Graphen trägt die Kantenfarbe ohnehin nur noch eine Fläche.
const LINE_FLAT_VS = `#version 300 es
in vec3 a_pos;
uniform mat4 u_view;
uniform mat4 u_proj;
void main() {
  gl_Position = u_proj * u_view * vec4(a_pos, 1.0);
}`;

const LINE_FLAT_FS = `#version 300 es
precision mediump float;
uniform vec4 u_color;
out vec4 o_color;
void main() {
  o_color = vec4(u_color.rgb * u_color.a, u_color.a);
}`;

/** Multisampling des Zeichenpuffers (A/B-Schalter für die Messung auf dem Mac mini). */
export const GL_ANTIALIAS = false;

/** Ab so vielen Kanten ohne Tiefen-Abblendung (Leistung, UI-098). */
export const FLAT_LINES_FROM = 20_000;

// Hintergrund als Vollbild-Dreieck mit weichem radialem Verlauf (Mitte → Rand), damit die Zeichenfläche deckend bleibt
// (eine transparente WebGL-Fläche müsste der Browser in jedem Bild über den CSS-Hintergrund mischen).
const BG_VS = `#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}`;

const BG_FS = `#version 300 es
precision mediump float;
uniform vec2 u_size;
uniform vec3 u_center;
uniform vec3 u_edge;
out vec4 o_color;
void main() {
  vec2 p = gl_FragCoord.xy / u_size;
  float d = length((p - vec2(0.5, 0.58)) / vec2(0.75, 0.8));
  o_color = vec4(mix(u_center, u_edge, smoothstep(0.0, 1.0, d)), 1.0);
}`;

function compile(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const program = gl.createProgram();
  if (!program) throw new Error("program");
  for (const [type, src] of [
    [gl.VERTEX_SHADER, vs],
    [gl.FRAGMENT_SHADER, fs],
  ] as const) {
    const shader = gl.createShader(type);
    if (!shader) throw new Error("shader");
    gl.shaderSource(shader, src);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(shader) ?? "shader");
    gl.attachShader(program, shader);
    gl.deleteShader(shader);
  }
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(program) ?? "program");
  return program;
}

/** Ob die Umgebung WebGL2 bietet (sonst zeigt das Fenster einen Hinweis statt einer leeren Fläche). */
export function hasWebGL2(doc: Document = document): boolean {
  try {
    const c = doc.createElement("canvas");
    const gl = c.getContext("webgl2");
    const ok = !!gl;
    gl?.getExtension("WEBGL_lose_context")?.loseContext();
    return ok;
  } catch {
    return false;
  }
}

export class GraphGl {
  private readonly gl: WebGL2RenderingContext;
  private readonly points: WebGLProgram;
  private readonly flatLines: WebGLProgram;
  private readonly flatVao: WebGLVertexArrayObject;
  private readonly background: WebGLProgram;
  private readonly bgVao: WebGLVertexArrayObject;
  private readonly lines: WebGLProgram;
  private readonly pointVao: WebGLVertexArrayObject;
  private readonly lineVao: WebGLVertexArrayObject;
  private readonly posBuf: WebGLBuffer;
  private readonly sizeBuf: WebGLBuffer;
  private readonly colorBuf: WebGLBuffer;
  private readonly edgeBuf: WebGLBuffer;
  private readonly inBuf: WebGLBuffer;
  private readonly outBuf: WebGLBuffer;
  private readonly haloBuf: WebGLBuffer;
  private haloCount = 0;
  private n = 0;
  private edgeCount = 0;
  private inCount = 0;
  private outCount = 0;
  private maxPoint = 64;
  private uniforms: Record<string, WebGLUniformLocation | null> = {};

  private constructor(gl: WebGL2RenderingContext) {
    this.gl = gl;
    this.points = compile(gl, POINT_VS, POINT_FS);
    this.lines = compile(gl, LINE_VS, LINE_FS);
    this.background = compile(gl, BG_VS, BG_FS);
    this.flatLines = compile(gl, LINE_FLAT_VS, LINE_FLAT_FS);
    const buf = () => {
      const b = gl.createBuffer();
      if (!b) throw new Error("buffer");
      return b;
    };
    this.posBuf = buf();
    this.sizeBuf = buf();
    this.colorBuf = buf();
    this.edgeBuf = buf();
    this.inBuf = buf();
    this.outBuf = buf();
    this.haloBuf = buf();
    const vao = () => {
      const v = gl.createVertexArray();
      if (!v) throw new Error("vao");
      return v;
    };
    this.bgVao = vao();
    this.pointVao = vao();
    gl.bindVertexArray(this.pointVao);
    const attr = (program: WebGLProgram, name: string, buffer: WebGLBuffer, size: number, type: number, normalized: boolean) => {
      const loc = gl.getAttribLocation(program, name);
      if (loc < 0) return;
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, size, type, normalized, 0, 0);
    };
    attr(this.points, "a_pos", this.posBuf, 3, gl.FLOAT, false);
    attr(this.points, "a_size", this.sizeBuf, 1, gl.FLOAT, false);
    attr(this.points, "a_color", this.colorBuf, 4, gl.UNSIGNED_BYTE, true);
    this.lineVao = vao();
    gl.bindVertexArray(this.lineVao);
    attr(this.lines, "a_pos", this.posBuf, 3, gl.FLOAT, false);
    this.flatVao = vao();
    gl.bindVertexArray(this.flatVao);
    attr(this.flatLines, "a_pos", this.posBuf, 3, gl.FLOAT, false);
    gl.bindVertexArray(null);
    for (const name of ["u_view", "u_proj", "u_scale", "u_minPx", "u_maxPx", "u_fogNear", "u_fogFar", "u_halo", "u_haloColor"]) this.uniforms[`p${name}`] = gl.getUniformLocation(this.points, name);
    for (const name of ["u_view", "u_proj", "u_color", "u_fogNear", "u_fogFar"]) this.uniforms[`l${name}`] = gl.getUniformLocation(this.lines, name);
    for (const name of ["u_size", "u_center", "u_edge"]) this.uniforms[`b${name}`] = gl.getUniformLocation(this.background, name);
    for (const name of ["u_view", "u_proj", "u_color"]) this.uniforms[`f${name}`] = gl.getUniformLocation(this.flatLines, name);
    const range = gl.getParameter(gl.ALIASED_POINT_SIZE_RANGE) as Float32Array | null;
    this.maxPoint = Math.min(256, range?.[1] ?? 64);
  }

  /** WebGL2-Kontext mit erhaltenem Zeichenpuffer (PNG-Export des aktuellen Bilds); `null` ohne WebGL2. */
  static create(canvas: HTMLCanvasElement): GraphGl | null {
    let gl: WebGL2RenderingContext | null = null;
    try {
      // Ohne `preserveDrawingBuffer` (der PNG-Export zeichnet und liest im selben Durchlauf) und ohne Multisampling
      // (Kugeln und Plaketten glätten ihre Ränder selbst) – weniger Speicher und Bandbreite (Fable-Fixrunde, A/B im Bericht).
      gl = canvas.getContext("webgl2", { antialias: GL_ANTIALIAS, preserveDrawingBuffer: false, alpha: false });
    } catch {
      gl = null;
    }
    // Ein verlorener Kontext (z. B. nach einem früheren `loseContext`) liefert keine Programme mehr.
    if (!gl || gl.isContextLost()) return null;
    try {
      return new GraphGl(gl);
    } catch {
      return null;
    }
  }

  /** Knoten: Radius in Welteinheiten und Farbe (RGBA, 0–255) je Knoten. */
  setNodes(sizes: Float32Array, colors: Uint8Array): void {
    const gl = this.gl;
    this.n = sizes.length;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.sizeBuf);
    gl.bufferData(gl.ARRAY_BUFFER, sizes, gl.STATIC_DRAW);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.colorBuf);
    gl.bufferData(gl.ARRAY_BUFFER, colors, gl.DYNAMIC_DRAW);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.posBuf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(this.n * 3), gl.DYNAMIC_DRAW);
  }

  setColors(colors: Uint8Array): void {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.colorBuf);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, colors);
  }

  setPositions(pos: Float32Array): void {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, this.posBuf);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, pos.subarray(0, this.n * 3));
  }

  private indices(buffer: WebGLBuffer, idx: Uint32Array): number {
    const gl = this.gl;
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, idx, gl.STATIC_DRAW);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, null);
    return idx.length;
  }

  /** Baumkanten als Paare von Knotenindizes. */
  setEdges(idx: Uint32Array): void {
    this.edgeCount = this.indices(this.edgeBuf, idx);
  }

  /** Hervorgehobene Knoten mit Lichtschein (Zeiger, Suchtreffer, Auswahl). */
  setHalo(idx: Uint32Array): void {
    this.haloCount = this.indices(this.haloBuf, idx);
  }

  /** Eingeblendete Inlinks und Outlinks (UI-097). */
  setOverlay(inlinks: Uint32Array, outlinks: Uint32Array): void {
    this.inCount = this.indices(this.inBuf, inlinks);
    this.outCount = this.indices(this.outBuf, outlinks);
  }

  /**
   * Ein Bild auf transparentem Grund (der Hintergrundverlauf liegt im CSS bzw. wird beim PNG-Export darunter gelegt):
   * Kanten dünn und nach Tiefe abgeblendet (gedämpft, wenn Links eingeblendet sind), Zusatzkanten kräftiger, Knoten als
   * schattierte Kugeln, zuletzt der Lichtschein hervorgehobener Knoten. `scale` = Bildpunkte je Welteinheit im Abstand 1.
   */
  render(view: Float32Array, proj: Float32Array, width: number, height: number, scale: number, colors: GlColors, o: RenderOptions3D): void {
    const gl = this.gl;
    gl.viewport(0, 0, width, height);
    gl.clear(gl.DEPTH_BUFFER_BIT);
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.BLEND);
    gl.useProgram(this.background);
    gl.uniform2f(this.uniforms["bu_size"] ?? null, width, height);
    gl.uniform3fv(this.uniforms["bu_center"] ?? null, colors.backgroundCenter);
    gl.uniform3fv(this.uniforms["bu_edge"] ?? null, colors.backgroundEdge);
    gl.bindVertexArray(this.bgVao);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    if (this.n === 0) return;
    const flat = this.edgeCount / 2 >= FLAT_LINES_FROM;
    const prefix = flat ? "f" : "l";
    gl.useProgram(flat ? this.flatLines : this.lines);
    gl.uniformMatrix4fv(this.uniforms[`${prefix}u_view`] ?? null, false, view);
    gl.uniformMatrix4fv(this.uniforms[`${prefix}u_proj`] ?? null, false, proj);
    if (!flat) {
      gl.uniform1f(this.uniforms["lu_fogNear"] ?? null, o.fogNear);
      gl.uniform1f(this.uniforms["lu_fogFar"] ?? null, o.fogFar);
    }
    gl.bindVertexArray(flat ? this.flatVao : this.lineVao);
    gl.depthMask(false);
    const lines = (buffer: WebGLBuffer, count: number, color: [number, number, number, number]) => {
      if (!count) return;
      gl.uniform4fv(this.uniforms[`${prefix}u_color`] ?? null, color);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffer);
      gl.drawElements(gl.LINES, count, gl.UNSIGNED_INT, 0);
    };
    const e = colors.edge;
    lines(this.edgeBuf, this.edgeCount, o.dimEdges ? [e[0], e[1], e[2], e[3] * 0.4] : e);
    gl.disable(gl.DEPTH_TEST);
    lines(this.inBuf, this.inCount, colors.inlink);
    lines(this.outBuf, this.outCount, colors.outlink);
    gl.enable(gl.DEPTH_TEST);
    gl.depthMask(true);
    gl.useProgram(this.points);
    gl.uniformMatrix4fv(this.uniforms["pu_view"] ?? null, false, view);
    gl.uniformMatrix4fv(this.uniforms["pu_proj"] ?? null, false, proj);
    gl.uniform1f(this.uniforms["pu_scale"] ?? null, scale);
    gl.uniform1f(this.uniforms["pu_minPx"] ?? null, o.minPx);
    gl.uniform1f(this.uniforms["pu_maxPx"] ?? null, Math.min(o.maxPx, this.maxPoint));
    gl.uniform1f(this.uniforms["pu_fogNear"] ?? null, o.fogNear);
    gl.uniform1f(this.uniforms["pu_fogFar"] ?? null, o.fogFar);
    gl.uniform1f(this.uniforms["pu_halo"] ?? null, 0);
    gl.bindVertexArray(this.pointVao);
    gl.drawArrays(gl.POINTS, 0, this.n);
    if (this.haloCount) {
      gl.depthMask(false);
      gl.uniform1f(this.uniforms["pu_halo"] ?? null, 1);
      gl.uniform4fv(this.uniforms["pu_haloColor"] ?? null, colors.halo);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.haloBuf);
      gl.drawElements(gl.POINTS, this.haloCount, gl.UNSIGNED_INT, 0);
      gl.depthMask(true);
    }
    gl.bindVertexArray(null);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, null);
  }

  dispose(): void {
    const gl = this.gl;
    for (const b of [this.posBuf, this.sizeBuf, this.colorBuf, this.edgeBuf, this.inBuf, this.outBuf, this.haloBuf]) gl.deleteBuffer(b);
    gl.deleteVertexArray(this.pointVao);
    gl.deleteVertexArray(this.bgVao);
    gl.deleteVertexArray(this.flatVao);
    gl.deleteProgram(this.flatLines);
    gl.deleteProgram(this.background);
    gl.deleteVertexArray(this.lineVao);
    gl.deleteProgram(this.points);
    gl.deleteProgram(this.lines);
    // Kein `loseContext`: derselbe Canvas bekommt beim erneuten Einhängen (StrictMode) denselben Kontext zurück.
  }
}

/** Hex-Farbe (#rrggbb) als RGB in [0, 1]. */
export function rgbOf(hex: string): [number, number, number] {
  const n = parseInt(hex.replace("#", "").padEnd(6, "0").slice(0, 6), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

/** Alles, was nach einem wiederhergestellten Kontext (`webglcontextrestored`) neu hochgeladen werden muss. */
export interface SceneUpload {
  radius: Float32Array;
  colors: Uint8Array | null;
  edges: Uint32Array;
  inlinks: Uint32Array;
  outlinks: Uint32Array;
  halo: Uint32Array;
  positions: Float32Array;
}

export type SceneSink = Pick<GraphGl, "setNodes" | "setEdges" | "setColors" | "setOverlay" | "setHalo" | "setPositions">;

/** Lädt eine Szene vollständig in einen (neuen) Kontext: Knoten, Kanten, Farben, Zusatzkanten, Lichtschein, Positionen. */
export function uploadScene(g: SceneSink, s: SceneUpload): void {
  const n = s.radius.length;
  g.setNodes(s.radius, s.colors ?? new Uint8Array(n * 4));
  g.setEdges(s.edges);
  if (s.colors) g.setColors(s.colors);
  g.setOverlay(s.inlinks, s.outlinks);
  g.setHalo(s.halo);
  g.setPositions(s.positions);
}

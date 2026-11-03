// WebGL2-Zeichnung der 3D-Graphen (UI-098, UI-146): alle Knoten in einem Zeichenaufruf als Punkte (Kreis im
// Fragment-Shader, Größe mit Perspektive), Baumkanten und eingeblendete Links (UI-097) als Linien aus Indexpuffern.
// Keine Bibliothek (M3-V02, Lizenzprüfung): nur WebGL2 der WebView (WKWebView ab Safari 15, WebView2).

export interface GlColors {
  background: [number, number, number];
  edge: [number, number, number, number];
  inlink: [number, number, number, number];
  outlink: [number, number, number, number];
}

const POINT_VS = `#version 300 es
in vec3 a_pos;
in float a_size;
in vec4 a_color;
uniform mat4 u_view;
uniform mat4 u_proj;
uniform float u_scale;
uniform float u_max;
out vec4 v_color;
out float v_px;
void main() {
  vec4 mv = u_view * vec4(a_pos, 1.0);
  gl_Position = u_proj * mv;
  float px = 2.0 * a_size * u_scale / max(-mv.z, 0.0001);
  gl_PointSize = clamp(px, 2.0, u_max);
  v_color = a_color;
  v_px = gl_PointSize;
}`;

const POINT_FS = `#version 300 es
precision mediump float;
in vec4 v_color;
in float v_px;
out vec4 o_color;
void main() {
  vec2 c = gl_PointCoord * 2.0 - 1.0;
  float r2 = dot(c, c);
  if (r2 > 1.0) discard;
  float rim = smoothstep(1.0 - 3.0 / max(v_px, 3.0), 1.0, r2);
  o_color = vec4(v_color.rgb * (1.0 - 0.35 * rim), v_color.a);
}`;

const LINE_VS = `#version 300 es
in vec3 a_pos;
uniform mat4 u_view;
uniform mat4 u_proj;
void main() {
  gl_Position = u_proj * u_view * vec4(a_pos, 1.0);
}`;

const LINE_FS = `#version 300 es
precision mediump float;
uniform vec4 u_color;
out vec4 o_color;
void main() {
  o_color = u_color;
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
  private readonly lines: WebGLProgram;
  private readonly pointVao: WebGLVertexArrayObject;
  private readonly lineVao: WebGLVertexArrayObject;
  private readonly posBuf: WebGLBuffer;
  private readonly sizeBuf: WebGLBuffer;
  private readonly colorBuf: WebGLBuffer;
  private readonly edgeBuf: WebGLBuffer;
  private readonly inBuf: WebGLBuffer;
  private readonly outBuf: WebGLBuffer;
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
    const vao = () => {
      const v = gl.createVertexArray();
      if (!v) throw new Error("vao");
      return v;
    };
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
    gl.bindVertexArray(null);
    for (const name of ["u_view", "u_proj", "u_scale", "u_max"]) this.uniforms[`p${name}`] = gl.getUniformLocation(this.points, name);
    for (const name of ["u_view", "u_proj", "u_color"]) this.uniforms[`l${name}`] = gl.getUniformLocation(this.lines, name);
    const range = gl.getParameter(gl.ALIASED_POINT_SIZE_RANGE) as Float32Array | null;
    this.maxPoint = Math.min(256, range?.[1] ?? 64);
  }

  /** WebGL2-Kontext mit erhaltenem Zeichenpuffer (PNG-Export des aktuellen Bilds); `null` ohne WebGL2. */
  static create(canvas: HTMLCanvasElement): GraphGl | null {
    let gl: WebGL2RenderingContext | null = null;
    try {
      gl = canvas.getContext("webgl2", { antialias: true, preserveDrawingBuffer: true, alpha: false });
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

  /** Eingeblendete Inlinks und Outlinks (UI-097). */
  setOverlay(inlinks: Uint32Array, outlinks: Uint32Array): void {
    this.inCount = this.indices(this.inBuf, inlinks);
    this.outCount = this.indices(this.outBuf, outlinks);
  }

  /**
   * Ein Bild: Hintergrund, Kanten (gedimmt, wenn Links eingeblendet sind), Zusatzkanten, Knoten. `scale` = Bildpunkte je
   * Welteinheit im Abstand 1 (Höhe / (2·tan(fov/2)) · Pixelverhältnis).
   */
  render(view: Float32Array, proj: Float32Array, width: number, height: number, scale: number, colors: GlColors, dimEdges: boolean): void {
    const gl = this.gl;
    gl.viewport(0, 0, width, height);
    gl.clearColor(colors.background[0], colors.background[1], colors.background[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    if (this.n === 0) return;
    gl.useProgram(this.lines);
    gl.uniformMatrix4fv(this.uniforms["lu_view"] ?? null, false, view);
    gl.uniformMatrix4fv(this.uniforms["lu_proj"] ?? null, false, proj);
    gl.bindVertexArray(this.lineVao);
    gl.depthMask(false);
    const lines = (buffer: WebGLBuffer, count: number, color: [number, number, number, number]) => {
      if (!count) return;
      gl.uniform4fv(this.uniforms["lu_color"] ?? null, color);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, buffer);
      gl.drawElements(gl.LINES, count, gl.UNSIGNED_INT, 0);
    };
    const e = colors.edge;
    lines(this.edgeBuf, this.edgeCount, dimEdges ? [e[0], e[1], e[2], e[3] * 0.35] : e);
    lines(this.inBuf, this.inCount, colors.inlink);
    lines(this.outBuf, this.outCount, colors.outlink);
    gl.depthMask(true);
    gl.useProgram(this.points);
    gl.uniformMatrix4fv(this.uniforms["pu_view"] ?? null, false, view);
    gl.uniformMatrix4fv(this.uniforms["pu_proj"] ?? null, false, proj);
    gl.uniform1f(this.uniforms["pu_scale"] ?? null, scale);
    gl.uniform1f(this.uniforms["pu_max"] ?? null, this.maxPoint);
    gl.bindVertexArray(this.pointVao);
    gl.drawArrays(gl.POINTS, 0, this.n);
    gl.bindVertexArray(null);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, null);
  }

  dispose(): void {
    const gl = this.gl;
    for (const b of [this.posBuf, this.sizeBuf, this.colorBuf, this.edgeBuf, this.inBuf, this.outBuf]) gl.deleteBuffer(b);
    gl.deleteVertexArray(this.pointVao);
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

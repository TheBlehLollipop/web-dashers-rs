// The level layer (background, every normal-layer object, the end wall, the end rays) drawn with WebGL, as Phaser does:
// one textured quad per sprite, the tint is a vertex colour, and sprites that share an atlas and a blend mode go out in one draw call.
// Canvas 2D has no per-sprite tint, so it had to build a coloured copy of the image for every sprite and every colour step.
export function createGL(canvas) {
  const gl = canvas.getContext("webgl", { alpha: false, antialias: false, premultipliedAlpha: true, preserveDrawingBuffer: false, powerPreference: "high-performance" });
  if (!gl) return null;
  const compile = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s)); return s; };
  const prog = gl.createProgram();
  gl.attachShader(prog, compile(gl.VERTEX_SHADER, `
    attribute vec2 a_pos; attribute vec2 a_uv; attribute vec4 a_col;
    uniform vec2 u_size; uniform vec2 u_off;
    varying vec2 v_uv; varying vec4 v_col;
    void main() { vec2 p = (a_pos + u_off) / u_size; gl_Position = vec4(p.x * 2.0 - 1.0, 1.0 - p.y * 2.0, 0.0, 1.0); v_uv = a_uv; v_col = a_col; }`));
  gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, `
    precision mediump float;
    uniform sampler2D u_tex; varying vec2 v_uv; varying vec4 v_col;
    void main() { gl_FragColor = texture2D(u_tex, v_uv) * v_col; }`));
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
  gl.useProgram(prog);
  const aPos = gl.getAttribLocation(prog, "a_pos"), aUv = gl.getAttribLocation(prog, "a_uv"), aCol = gl.getAttribLocation(prog, "a_col");
  const uSize = gl.getUniformLocation(prog, "u_size"), uOff = gl.getUniformLocation(prog, "u_off");
  gl.uniform1i(gl.getUniformLocation(prog, "u_tex"), 0);

  const IDX = [0, 1, 2, 0, 2, 3];
  // Every quad of the frame goes into one array and is uploaded once in `end()`; the draw calls (one per run of sprites that share a texture and
  // blend mode) just point into it. Uploading after each batch made the driver wait for the GPU to finish with the buffer.
  const MAX = 16384, STRIDE = 8; // sprites per frame; floats per vertex: x y u v r g b a
  const data = new Float32Array(MAX * 6 * STRIDE);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, data.byteLength, gl.DYNAMIC_DRAW);
  const runs = []; // { tex, blend, start, count } in quads
  let runStart = 0;
  for (const [loc, n, off] of [[aPos, 2, 0], [aUv, 2, 2], [aCol, 4, 4]]) { gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, n, gl.FLOAT, false, STRIDE * 4, off * 4); }
  gl.disable(gl.DEPTH_TEST); gl.disable(gl.CULL_FACE); gl.enable(gl.BLEND);

  const textures = new WeakMap();
  const texFor = (img) => {
    let t = textures.get(img);
    if (t) return t;
    t = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, t);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    t = { tex: t, w: img.naturalWidth || img.width, h: img.naturalHeight || img.height };
    textures.set(img, t);
    return t;
  };
  const white = (() => { // 1x1 white, for flat-coloured polygons
    const t = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D, t);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([255, 255, 255, 255]));
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST); gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    return { tex: t, w: 1, h: 1 };
  })();

  let n = 0, curTex = null, curBlend = -1, W = canvas.width, H = canvas.height, offX = 0, offY = 0;
  const closeRun = () => { if (n > runStart && curTex) runs.push({ tex: curTex, blend: curBlend, start: runStart, count: n - runStart }); runStart = n; };
  const flush = () => { // sends what was collected so far (the array is full)
    closeRun();
    if (!runs.length) return;
    gl.bufferData(gl.ARRAY_BUFFER, data.subarray(0, n * 6 * STRIDE), gl.DYNAMIC_DRAW); // a fresh store each time, so the GPU may still read the old one
    let bl = -1;
    for (const r of runs) {
      if (r.blend !== bl) { if (r.blend === 1) gl.blendFunc(gl.ONE, gl.ONE); else gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA); bl = r.blend; }
      gl.bindTexture(gl.TEXTURE_2D, r.tex.tex);
      gl.drawArrays(gl.TRIANGLES, r.start * 6, r.count * 6);
    }
    runs.length = 0; n = 0; runStart = 0;
  };
  const setState = (tex, blend) => {
    if (tex !== curTex || blend !== curBlend) { closeRun(); curTex = tex; curBlend = blend; }
    if (n >= MAX) { const t = curTex, b = curBlend; flush(); curTex = t; curBlend = b; }
  };
  // one quad from four corners (x y u v each), premultiplied colour
  const push = (p, r, g, b, a) => {
    let o = n * 6 * STRIDE; n++;
    for (let q = 0; q < 6; q++) {
      const k = IDX[q];
      data[o++] = p[k * 4]; data[o++] = p[k * 4 + 1]; data[o++] = p[k * 4 + 2]; data[o++] = p[k * 4 + 3];
      data[o++] = r * a; data[o++] = g * a; data[o++] = b * a; data[o++] = a;
    }
  };
  const P = new Float64Array(16);
  let cx = 0, cy = 0, cc = 1, cs = 0, cSx = 1, cSy = 1, ck = 0, ctw = 1, cth = 1;
  let vOn = false, vA = 1, vB = 0, vX = 0, vY = 0;
  const corner = (px, py, u, v) => { // local point -> screen, with its texture coordinate
    const ax = px * cSx, ay = py * cSy;
    let x = cx + ax * cc - ay * cs, y = cy + ax * cs + ay * cc;
    if (vOn) { const dx = x - vX, dy = y - vY; x = vX + dx * vA - dy * vB; y = vY + dx * vB + dy * vA; }
    P[ck++] = x; P[ck++] = y; P[ck++] = u / ctw; P[ck++] = v / cth;
  };

  const api = {
    /** the world camera's zoom and rotation (degrees) around the screen centre; `view(1, 0)` turns it off */
    view(zoom, deg) {
      vOn = zoom !== 1 || deg !== 0;
      const r = deg * Math.PI / 180;
      vA = zoom * Math.cos(r); vB = zoom * Math.sin(r); vX = W / 2; vY = H / 2;
    },
    begin(shakeX, shakeY) {
      offX = shakeX || 0; offY = shakeY || 0; n = 0; runStart = 0; runs.length = 0; curTex = null; curBlend = -1;
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.uniform2f(uSize, W, H); gl.uniform2f(uOff, offX, offY);
      gl.clearColor(0, 0, 0, 1); gl.clear(gl.COLOR_BUFFER_BIT);
    },
    end() { flush(); },
    /** `drawFrame` of main.js: a frame of an atlas (`fr = { img, d }`) with the same arguments */
    frame(fr, x, y, rot, sx, sy, alpha, tint, blend, flips) {
      const d = fr.d, f = d.frame, fw = f.w, fh = f.h, t = texFor(fr.img);
      const src = d.sourceSize || { w: fw, h: fh }, ss = d.spriteSourceSize || { x: 0, y: 0 };
      const ox = -src.w / 2 + ss.x, oy = -src.h / 2 + ss.y;
      const ins = (fw < t.w || fh < t.h) ? 0.5 : 0; // the original insets the UVs of every atlas frame by half a texel (core/main.js `insetFrameUVs`)
      const Sx = (flips & 1 ? -1 : 1) * sx, Sy = (flips & 2 ? -1 : 1) * sy;
      cx = x; cy = y; cc = rot ? Math.cos(rot) : 1; cs = rot ? Math.sin(rot) : 0; cSx = Sx; cSy = Sy; ck = 0; ctw = t.w; cth = t.h;
      if (d.rotated) { // stored turned by 90 degrees: the source rectangle is fh wide and fw high
        const u0 = -oy - fh, u1 = -oy, v0 = ox, v1 = ox + fw;
        // rotate(-90deg): (u, v) -> (v, -u)
        corner(v0, -u0, f.x, f.y); corner(v0, -u1, f.x + fh, f.y); corner(v1, -u1, f.x + fh, f.y + fw); corner(v1, -u0, f.x, f.y + fw);
      } else {
        corner(ox, oy, f.x + ins, f.y + ins); corner(ox + fw, oy, f.x + fw - ins, f.y + ins); corner(ox + fw, oy + fh, f.x + fw - ins, f.y + fh - ins); corner(ox, oy + fh, f.x + ins, f.y + fh - ins);
      }
      setState(t, blend === 1 ? 1 : 0);
      const tn = tint >= 0 && tint !== 0xffffff;
      push(P, tn ? ((tint >> 16) & 255) / 255 : 1, tn ? ((tint >> 8) & 255) / 255 : 1, tn ? (tint & 255) / 255 : 1, Math.max(0, Math.min(1, alpha)));
    },
    /** a whole image (or canvas) as a rectangle, optionally mirrored vertically */
    image(img, x, y, w, h, tint, flipV) {
      const t = texFor(img);
      let k = 0;
      const v0 = flipV ? 1 : 0, v1 = flipV ? 0 : 1;
      for (const [px, py, u, v] of [[x, y, 0, v0], [x + w, y, 1, v0], [x + w, y + h, 1, v1], [x, y + h, 0, v1]]) { P[k++] = px; P[k++] = py; P[k++] = u; P[k++] = v; }
      setState(t, 0);
      const tn = tint >= 0 && tint !== 0xffffff;
      push(P, tn ? ((tint >> 16) & 255) / 255 : 1, tn ? ((tint >> 8) & 255) / 255 : 1, tn ? (tint & 255) / 255 : 1, 1);
    },
    /** a flat-coloured quad given by four corners [x0, y0, ...] */
    quad(pts, color, alpha, additive) {
      setState(white, additive ? 1 : 0);
      for (let i = 0; i < 4; i++) { P[i * 4] = pts[i * 2]; P[i * 4 + 1] = pts[i * 2 + 1]; P[i * 4 + 2] = 0.5; P[i * 4 + 3] = 0.5; }
      push(P, ((color >> 16) & 255) / 255, ((color >> 8) & 255) / 255, (color & 255) / 255, Math.max(0, Math.min(1, alpha)));
    },
  };
  return api;
}

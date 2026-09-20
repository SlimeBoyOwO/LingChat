import { invoke } from "@tauri-apps/api/core";

/** Mount the fly's meadow-life scene inside an isolated UI root; all resources belong to this mount. */
export async function mountFlyBrain(root, options) {
  "use strict";
  const $ = (s) => root.querySelector(s);

  /* ================= 生命周期 ================= */
  const lifetime = new AbortController();
  const on = (target, name, callback, opts) =>
    target.addEventListener(name, callback, { ...opts, signal: lifetime.signal });
  let destroyed = false,
    frameId = 0,
    pollTimer = 0;
  let brain = null;
  function destroy() {
    if (destroyed) return;
    destroyed = true;
    cancelAnimationFrame(frameId);
    clearInterval(pollTimer);
    lifetime.abort();
    brain?.lose();
    if (gl) gl.getExtension("WEBGL_lose_context")?.loseContext();
  }

  /* ================= WebGL2 基础 ================= */
  const canvas = $("#scene");
  const gl = canvas.getContext("webgl2", { antialias: true, alpha: false });
  const veil = $("#loadingVeil");
  if (!gl) {
    veil.querySelector(".loading-text").textContent = "⚠️ 此环境不支持 WebGL2";
    return { destroy };
  }

  function makeProg(vsSrc, fsSrc) {
    const compile = (type, src) => {
      const s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
      return s;
    };
    const p = gl.createProgram();
    gl.attachShader(p, compile(gl.VERTEX_SHADER, vsSrc));
    gl.attachShader(p, compile(gl.FRAGMENT_SHADER, fsSrc));
    gl.linkProgram(p);
    if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
    return p;
  }
  const uniforms = (prog, names) =>
    Object.fromEntries(names.map((n) => [n, gl.getUniformLocation(prog, n)]));

  /* ================= mat4 小工具（列主序，主场景与大脑小窗共用） ================= */
  function persp(fov, asp, n, f) {
    const t = 1 / Math.tan(fov / 2),
      d = 1 / (n - f);
    return new Float32Array([
      t / asp,
      0,
      0,
      0,
      0,
      t,
      0,
      0,
      0,
      0,
      (f + n) * d,
      -1,
      0,
      0,
      2 * f * n * d,
      0,
    ]);
  }
  function matMul(a, b) {
    const o = new Float32Array(16);
    for (let c = 0; c < 4; c++)
      for (let r = 0; r < 4; r++)
        o[c * 4 + r] =
          a[r] * b[c * 4] +
          a[4 + r] * b[c * 4 + 1] +
          a[8 + r] * b[c * 4 + 2] +
          a[12 + r] * b[c * 4 + 3];
    return o;
  }
  function rotX(a) {
    const c = Math.cos(a),
      s = Math.sin(a);
    return new Float32Array([1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1]);
  }
  function rotY(a) {
    const c = Math.cos(a),
      s = Math.sin(a);
    return new Float32Array([c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]);
  }
  function trans(x, y, z) {
    return new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1]);
  }
  function lookAt(eye, c) {
    let zx = eye[0] - c[0],
      zy = eye[1] - c[1],
      zz = eye[2] - c[2];
    let l = Math.hypot(zx, zy, zz) || 1;
    zx /= l;
    zy /= l;
    zz /= l;
    // x = normalize(cross(up=(0,1,0), z))
    let xx = zz,
      xy = 0,
      xz = -zx;
    l = Math.hypot(xx, xy, xz) || 1;
    xx /= l;
    xy /= l;
    xz /= l;
    // y = cross(z, x)
    const yx = zy * xz - zz * xy,
      yy = zz * xx - zx * xz,
      yz = zx * xy - zy * xx;
    return new Float32Array([
      xx,
      yx,
      zx,
      0,
      xy,
      yy,
      zy,
      0,
      xz,
      yz,
      zz,
      0,
      -(xx * eye[0] + xy * eye[1] + xz * eye[2]),
      -(yx * eye[0] + yy * eye[1] + yz * eye[2]),
      -(zx * eye[0] + zy * eye[1] + zz * eye[2]),
      1,
    ]);
  }
  function base64ToF32(b64) {
    const bin = atob(b64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return new Float32Array(bytes.buffer);
  }

  /* ================= 地形高度（CPU 与着色器共用同一函数） ================= */
  function hash2(ix, iz) {
    let h = (ix * 374761393 + iz * 668265263) | 0;
    h = (h ^ (h >>> 13)) | 0;
    h = (h * 1274126177) | 0;
    h = h ^ (h >>> 16);
    return (h >>> 0) / 4294967295;
  }
  function vnoise(x, z) {
    const ix = Math.floor(x),
      iz = Math.floor(z),
      fx = x - ix,
      fz = z - iz;
    const sx = fx * fx * (3 - 2 * fx),
      sz = fz * fz * (3 - 2 * fz);
    const a = hash2(ix, iz),
      b = hash2(ix + 1, iz),
      c = hash2(ix, iz + 1),
      d = hash2(ix + 1, iz + 1);
    return a + (b - a) * sx + (c - a) * sz + (a - b - c + d) * sx * sz;
  }
  /* 牧场一角的池塘：地形在此处压出浅盆 */
  const POND = { x: 28, z: 22, r: 9 };
  function rawBase(x, z) {
    let h = Math.sin(x * 0.045) * Math.cos(z * 0.05) * 1.5 + Math.sin(x * 0.021 + z * 0.03) * 1.1;
    h += vnoise(x * 0.08, z * 0.08) * 2.0 + vnoise(x * 0.21, z * 0.21) * 0.45;
    const r = Math.hypot(x, z);
    h += Math.max(0, r - 38) * 0.07; // 牧场外缘缓缓隆起，围出圆盘感
    return h;
  }
  function heightAt(x, z) {
    let h = rawBase(x, z);
    const pd = Math.hypot(x - POND.x, z - POND.z);
    const t = Math.max(0, Math.min(1, (POND.r + 4 - pd) / 6));
    h -= t * t * (3 - 2 * t) * 2.2;
    return h;
  }
  const WATER_Y = rawBase(POND.x, POND.z) - 1.15;
  const WATER_R = POND.r + 0.4;

  /* ================= 天空穹顶 ================= */
  const SKY_R = 400;
  const skyProg = makeProg(
    `#version 300 es
    layout(location=0) in vec3 aPos;
    uniform mat4 uVP;
    out vec3 vDir;
    void main() {
      vDir = aPos;
      vec4 p = uVP * vec4(aPos, 1.0);
      gl_Position = p.xyww;  // 永远贴远平面
    }`,
    `#version 300 es
    precision mediump float;
    in vec3 vDir;
    uniform vec3 uZenith, uHorizon, uDuskCol, uSunDir, uSunTint, uMoonDir;
    uniform float uDuskAmt, uSunVis, uNight;
    out vec4 o;
    void main() {
      vec3 dir = normalize(vDir);
      float y = dir.y;
      vec3 col = mix(uHorizon, uZenith, pow(clamp(y, 0.0, 1.0), 0.55));
      col = mix(col, uHorizon * 0.92, clamp(-y * 5.0, 0.0, 0.6));
      float sunD = dot(dir, uSunDir);
      float azGlow = pow(clamp(sunD * 0.5 + 0.5, 0.0, 1.0), 3.0)
                   * pow(1.0 - clamp(abs(y) * 2.2, 0.0, 1.0), 2.0);
      col = mix(col, uDuskCol, azGlow * uDuskAmt);
      float disc = smoothstep(0.99935, 0.99965, sunD);
      float halo = pow(max(sunD, 0.0), 200.0) * 0.55 + pow(max(sunD, 0.0), 24.0) * 0.16;
      col += (disc * 1.35 + halo) * uSunTint * uSunVis;
      float moonD = dot(dir, uMoonDir);
      float mdisc = smoothstep(0.99955, 0.99985, moonD);
      float mhalo = pow(max(moonD, 0.0), 350.0) * 0.35;
      col += (mdisc * 0.85 + mhalo) * vec3(0.88, 0.92, 1.0) * uNight;
      o = vec4(col, 1.0);
    }`,
  );
  const skyU = uniforms(skyProg, [
    "uVP",
    "uZenith",
    "uHorizon",
    "uDuskCol",
    "uSunDir",
    "uSunTint",
    "uMoonDir",
    "uDuskAmt",
    "uSunVis",
    "uNight",
  ]);
  const skyVao = (() => {
    const stacks = 24,
      slices = 48;
    const pos = [];
    for (let i = 0; i <= stacks; i++) {
      const phi = (i / stacks) * Math.PI;
      for (let j = 0; j <= slices; j++) {
        const th = (j / slices) * Math.PI * 2;
        pos.push(
          SKY_R * Math.sin(phi) * Math.cos(th),
          SKY_R * Math.cos(phi),
          SKY_R * Math.sin(phi) * Math.sin(th),
        );
      }
    }
    const idx = [];
    for (let i = 0; i < stacks; i++)
      for (let j = 0; j < slices; j++) {
        const a = i * (slices + 1) + j,
          b = a + slices + 1;
        idx.push(a, b, a + 1, a + 1, b, b + 1);
      }
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const vb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vb);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(pos), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    const ib = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint16Array(idx), gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    return { vao, count: idx.length };
  })();

  /* ================= 星星 ================= */
  const STAR_N = 380;
  const starProg = makeProg(
    `#version 300 es
    layout(location=0) in vec3 aPos;
    layout(location=1) in vec2 aInfo;  // size, phase
    uniform mat4 uVP;
    out float vPhase;
    void main() {
      vPhase = aInfo.y;
      gl_Position = uVP * vec4(aPos, 1.0);
      gl_PointSize = aInfo.x;
    }`,
    `#version 300 es
    precision mediump float;
    in float vPhase;
    uniform float uNight, uTime;
    out vec4 o;
    void main() {
      vec2 d = gl_PointCoord - 0.5;
      float r2 = dot(d, d);
      if (r2 > 0.25) discard;
      float g = smoothstep(0.25, 0.02, r2);
      float tw = 0.45 + 0.55 * sin(uTime * 2.0 + vPhase);
      o = vec4(vec3(0.95, 0.97, 1.0) * g, g * uNight * tw);
    }`,
  );
  const starU = uniforms(starProg, ["uVP", "uNight", "uTime"]);
  const starVao = (() => {
    const data = new Float32Array(STAR_N * 5);
    for (let i = 0; i < STAR_N; i++) {
      const th = Math.random() * Math.PI * 2,
        y = 0.04 + Math.pow(Math.random(), 0.7) * 0.96;
      const r = Math.sqrt(Math.max(0, 1 - y * y));
      data[i * 5] = Math.cos(th) * r * (SKY_R - 18);
      data[i * 5 + 1] = y * (SKY_R - 18);
      data[i * 5 + 2] = Math.sin(th) * r * (SKY_R - 18);
      data[i * 5 + 3] = 1.6 + Math.random() * 2.4;
      data[i * 5 + 4] = Math.random() * Math.PI * 2;
    }
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const vb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vb);
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 20, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 2, gl.FLOAT, false, 20, 12);
    gl.bindVertexArray(null);
    return vao;
  })();

  /* ================= 光照着色（地形 / 远山 / 果蝇身体共用） ================= */
  const litProg = makeProg(
    `#version 300 es
    layout(location=0) in vec3 aPos;
    layout(location=1) in vec3 aNormal;
    layout(location=2) in vec3 aColor;
    uniform mat4 uVP;
    out vec3 vN; out vec3 vC; out float vFog;
    void main() {
      vN = aNormal; vC = aColor;
      vec4 cp = uVP * vec4(aPos, 1.0);
      vFog = cp.w;
      gl_Position = cp;
    }`,
    `#version 300 es
    precision mediump float;
    in vec3 vN; in vec3 vC; in float vFog;
    uniform vec3 uSunDir, uSunColor, uAmbient, uFogColor;
    uniform float uFogK;
    out vec4 o;
    void main() {
      float ndl = max(dot(normalize(vN), uSunDir), 0.0);
      vec3 col = vC * (uAmbient + uSunColor * ndl);
      float f = 1.0 - exp(-vFog * vFog * uFogK);
      o = vec4(mix(col, uFogColor, f), 1.0);
    }`,
  );
  const litU = uniforms(litProg, ["uVP", "uSunDir", "uSunColor", "uAmbient", "uFogColor", "uFogK"]);

  /* ================= 地形 ================= */
  const terrainVao = (() => {
    const N = 96,
      SIZE = 120;
    const pos = new Float32Array(N * N * 3),
      nor = new Float32Array(N * N * 3),
      col = new Float32Array(N * N * 3);
    const step = SIZE / (N - 1);
    for (let iz = 0; iz < N; iz++)
      for (let ix = 0; ix < N; ix++) {
        const i = iz * N + ix;
        const x = -SIZE / 2 + ix * step,
          z = -SIZE / 2 + iz * step;
        const y = heightAt(x, z);
        pos[i * 3] = x;
        pos[i * 3 + 1] = y;
        pos[i * 3 + 2] = z;
        const e = 0.6;
        const nx = heightAt(x - e, z) - heightAt(x + e, z),
          nz = heightAt(x, z - e) - heightAt(x, z + e);
        const nl = Math.hypot(nx, 2 * e, nz);
        nor[i * 3] = nx / nl;
        nor[i * 3 + 1] = (2 * e) / nl;
        nor[i * 3 + 2] = nz / nl;
        // 嫩青草色：低地偏黄绿，高处偏青绿
        const t = Math.min(1, Math.max(0, (y + 1.5) / 6));
        const v = 0.9 + 0.18 * hash2(ix, iz);
        let r = (0.36 + (0.55 - 0.36) * t) * v,
          g = (0.6 + (0.76 - 0.6) * t) * v,
          b = (0.34 + (0.42 - 0.34) * t) * v;
        // 塘底水草泥沙色
        const pd = Math.hypot(x - POND.x, z - POND.z);
        if (pd < POND.r + 2 && y < WATER_Y + 0.15) {
          const s = 0.55;
          r = r * (1 - s) + 0.45 * s;
          g = g * (1 - s) + 0.47 * s;
          b = b * (1 - s) + 0.3 * s;
        }
        col[i * 3] = r;
        col[i * 3 + 1] = g;
        col[i * 3 + 2] = b;
      }
    const idx = new Uint16Array((N - 1) * (N - 1) * 6);
    let k = 0;
    for (let iz = 0; iz < N - 1; iz++)
      for (let ix = 0; ix < N - 1; ix++) {
        const a = iz * N + ix,
          b = a + N;
        idx[k++] = a;
        idx[k++] = b;
        idx[k++] = a + 1;
        idx[k++] = a + 1;
        idx[k++] = b;
        idx[k++] = b + 1;
      }
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const bind = (loc, arr) => {
      const b = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, b);
      gl.bufferData(gl.ARRAY_BUFFER, arr, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 3, gl.FLOAT, false, 0, 0);
    };
    bind(0, pos);
    bind(1, nor);
    bind(2, col);
    const ib = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, idx, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    return { vao, count: idx.length };
  })();

  /* ================= 远山三层剪影（青绿山水：黛绿 → 石青 → 淡蓝灰） ================= */
  const mountainVao = (() => {
    const LAYERS = [
      { R: 140, amp: 15, base: 4, f: 3.1, seed: 11, col: [0.16, 0.3, 0.26] },
      { R: 205, amp: 26, base: 8, f: 2.6, seed: 47, col: [0.26, 0.42, 0.5] },
      { R: 290, amp: 40, base: 12, f: 2.1, seed: 83, col: [0.56, 0.64, 0.7] },
    ];
    const SEG = 160;
    const pos = [],
      nor = [],
      col = [],
      idx = [];
    let vb = 0;
    for (const L of LAYERS) {
      for (let i = 0; i <= SEG; i++) {
        const a = (i / SEG) * Math.PI * 2,
          ca = Math.cos(a),
          sa = Math.sin(a);
        const ridge =
          L.base +
          L.amp * (0.3 + 0.7 * vnoise(ca * L.f + L.seed, sa * L.f + L.seed)) +
          L.amp * 0.25 * vnoise(ca * L.f * 2.7 + L.seed * 3, sa * L.f * 2.7 + L.seed * 3);
        const rTop = L.R - ridge * 0.35; // 峰顶略内倾，出山坡剪影
        const nl = Math.hypot(1, 0.35);
        const n = [-ca / nl, 0.35 / nl, -sa / nl];
        pos.push(ca * L.R, -8, sa * L.R, ca * rTop, ridge, sa * rTop);
        nor.push(...n, ...n);
        col.push(
          L.col[0] * 0.5,
          L.col[1] * 0.5,
          L.col[2] * 0.5,
          Math.min(1, L.col[0] * 1.18),
          Math.min(1, L.col[1] * 1.18),
          Math.min(1, L.col[2] * 1.18),
        );
        if (i < SEG) {
          const b0 = vb + i * 2;
          idx.push(b0, b0 + 1, b0 + 2, b0 + 2, b0 + 1, b0 + 3);
        }
      }
      vb += (SEG + 1) * 2;
    }
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const bind = (loc, arr) => {
      const b = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, b);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(arr), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 3, gl.FLOAT, false, 0, 0);
    };
    bind(0, pos);
    bind(1, nor);
    bind(2, col);
    const ib = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint16Array(idx), gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    return { vao, count: idx.length };
  })();

  /* ================= 池塘水面 ================= */
  const waterProg = makeProg(
    `#version 300 es
    layout(location=0) in vec3 aPos;
    uniform mat4 uVP;
    out vec3 vW; out float vFog;
    void main() {
      vW = aPos;
      vec4 cp = uVP * vec4(aPos, 1.0);
      vFog = cp.w;
      gl_Position = cp;
    }`,
    `#version 300 es
    precision mediump float;
    in vec3 vW; in float vFog;
    uniform vec3 uDeep, uSky, uSunTint, uEye, uFogColor;
    uniform float uTime, uSunVis, uFogK;
    out vec4 o;
    void main() {
      vec3 vd = normalize(uEye - vW);
      float fres = pow(1.0 - max(vd.y, 0.0), 1.6) * 0.55 + 0.3;
      float wave = sin(vW.x * 2.1 + uTime * 1.4) * sin(vW.z * 1.7 - uTime * 1.1);
      vec3 col = mix(uDeep, uSky, clamp(fres + wave * 0.07, 0.0, 1.0));
      float sp = pow(max(0.0, sin(vW.x * 4.3 + uTime * 2.0) * sin(vW.z * 3.9 - uTime * 1.6)), 8.0);
      col += sp * uSunTint * uSunVis * 0.5;
      float f = 1.0 - exp(-vFog * vFog * uFogK);
      col = mix(col, uFogColor, f);
      o = vec4(col * 0.9, 0.9);
    }`,
  );
  const waterU = uniforms(waterProg, [
    "uVP",
    "uTime",
    "uDeep",
    "uSky",
    "uSunTint",
    "uEye",
    "uFogColor",
    "uSunVis",
    "uFogK",
  ]);
  const waterVao = (() => {
    const SEG = 48;
    const pos = [POND.x, WATER_Y, POND.z];
    const idx = [];
    for (let i = 0; i <= SEG; i++) {
      const a = (i / SEG) * Math.PI * 2;
      pos.push(POND.x + Math.cos(a) * WATER_R, WATER_Y, POND.z + Math.sin(a) * WATER_R);
      if (i < SEG) idx.push(0, i + 2, i + 1);
    }
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const b = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, b);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(pos), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    const ib = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint16Array(idx), gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    return { vao, count: idx.length };
  })();

  /* ================= 博丽神社（本殿/鸟居/参道/石灯笼，静态合并单次 draw） ================= */
  const lanternGlows = []; // 石灯笼火袋位置，夜里在 sprite 批次加暖光晕
  const shrineGeo = (() => {
    const P = [],
      Nr = [],
      C = [];
    const push = (p, n, col) => {
      P.push(p[0], p[1], p[2]);
      Nr.push(n[0], n[1], n[2]);
      C.push(col[0], col[1], col[2]);
    };
    const tri = (a, b, c, col) => {
      const ux = b[0] - a[0],
        uy = b[1] - a[1],
        uz = b[2] - a[2];
      const vx = c[0] - a[0],
        vy = c[1] - a[1],
        vz = c[2] - a[2];
      let nx = uy * vz - uz * vy,
        ny = uz * vx - ux * vz,
        nz = ux * vy - uy * vx;
      const l = Math.hypot(nx, ny, nz) || 1;
      const n = [nx / l, ny / l, nz / l];
      push(a, n, col);
      push(b, n, col);
      push(c, n, col);
    };
    const quad = (a, b, c, d, col) => {
      tri(a, b, c, col);
      tri(a, c, d, col);
    };
    const box = (cx, cy, cz, w, h, d, col) => {
      const x0 = cx - w / 2,
        x1 = cx + w / 2,
        y0 = cy - h / 2,
        y1 = cy + h / 2,
        z0 = cz - d / 2,
        z1 = cz + d / 2;
      quad([x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1], col);
      quad([x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0], col);
      quad([x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0], col);
      quad([x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1], col);
      quad([x0, y1, z1], [x1, y1, z1], [x1, y1, z0], [x0, y1, z0], col);
      quad([x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1], col);
    };
    const cylY = (cx, y0, cz, r, h, col, segs = 10) => {
      for (let j = 0; j < segs; j++) {
        const t0 = (j / segs) * Math.PI * 2,
          t1 = ((j + 1) / segs) * Math.PI * 2;
        const c0 = Math.cos(t0),
          s0 = Math.sin(t0),
          c1 = Math.cos(t1),
          s1 = Math.sin(t1);
        quad(
          [cx + c0 * r, y0, cz + s0 * r],
          [cx + c1 * r, y0, cz + s1 * r],
          [cx + c1 * r, y0 + h, cz + s1 * r],
          [cx + c0 * r, y0 + h, cz + s0 * r],
          col,
        );
        tri(
          [cx, y0 + h, cz],
          [cx + c1 * r, y0 + h, cz + s1 * r],
          [cx + c0 * r, y0 + h, cz + s0 * r],
          col,
        );
      }
    };
    const soupMerge = (arr) => {
      for (let i = 0; i < arr.length; i += 9)
        push(
          [arr[i], arr[i + 1], arr[i + 2]],
          [arr[i + 3], arr[i + 4], arr[i + 5]],
          [arr[i + 6], arr[i + 7], arr[i + 8]],
        );
    };
    /* 四坡屋顶（寄棟/歇山简化）：檐口四角微起翘，前后梯形坡 + 两侧三角坡 */
    const hipRoof = (cx, cy, cz, hw, hd, rh, rhalf, col) => {
      const lift = (x, z) =>
        0.3 * Math.pow(Math.abs(x) / hw, 2) + 0.12 * Math.pow(Math.abs(z) / hd, 2);
      const eave = (x, z) => [cx + x, cy + lift(x, z), cz + z];
      const ridge = (x) => [cx + Math.max(-rhalf, Math.min(rhalf, x)), cy + rh, cz];
      for (const zs of [-1, 1]) {
        for (let i = 0; i < 3; i++) {
          const x0 = -hw + (i / 3) * 2 * hw,
            x1 = -hw + ((i + 1) / 3) * 2 * hw;
          quad(eave(x0, zs * hd), eave(x1, zs * hd), ridge(x1), ridge(x0), col);
        }
      }
      for (const xs of [-1, 1]) tri(eave(xs * hw, hd), eave(xs * hw, -hd), ridge(xs * rhalf), col);
    };
    const VERM = [0.84, 0.2, 0.1],
      VERM_D = [0.66, 0.13, 0.07],
      REDWOOD = [0.48, 0.15, 0.1],
      WHITEW = [0.93, 0.92, 0.88],
      ROOF = [0.2, 0.22, 0.26],
      ROOF_R = [0.15, 0.16, 0.2],
      STONE = [0.6, 0.61, 0.59],
      STONE_D = [0.5, 0.51, 0.5],
      WOOD = [0.4, 0.28, 0.16],
      WOOD_D = [0.24, 0.16, 0.09],
      STRAW = [0.85, 0.74, 0.45],
      WARMW = [1.0, 0.78, 0.5],
      SLAB = [0.72, 0.72, 0.69];

    /* 朱红鸟居（南侧参道入口，明神鸟居比例） */
    const tg = heightAt(0, 30);
    cylY(-1.9, tg - 0.3, 30, 0.22, 4.9, VERM, 12);
    cylY(1.9, tg - 0.3, 30, 0.22, 4.9, VERM, 12);
    box(0, tg + 4.62, 30, 5.6, 0.3, 0.5, VERM); // 笠木
    box(0, tg + 4.9, 30, 5.9, 0.26, 0.58, VERM_D); // 岛木压顶
    box(0, tg + 3.55, 30, 4.5, 0.24, 0.3, VERM); // 贯
    box(0, tg + 4.08, 30, 0.28, 0.6, 0.26, VERM); // 额束
    box(-1.9, tg - 0.12, 30, 0.7, 0.35, 0.7, STONE);
    box(1.9, tg - 0.12, 30, 0.7, 0.35, 0.7, STONE);

    /* 参道石板（贴合地形逐块取高） */
    for (let z = 27.5; z >= -23.6; z -= 1.45)
      box(0, heightAt(0, z) + 0.04, z, 2.2, 0.12, 1.18, SLAB);

    /* 石灯笼（参道两侧成对，夜里火袋放暖光晕） */
    const lantern = (x, z) => {
      const g = heightAt(x, z);
      box(x, g + 0.11, z, 0.55, 0.22, 0.55, STONE_D); // 基座
      cylY(x, g + 0.22, z, 0.08, 0.72, STONE, 8); // 竿
      box(x, g + 0.98, z, 0.44, 0.1, 0.44, STONE); // 中台
      box(x, g + 1.19, z, 0.34, 0.32, 0.34, STONE_D); // 火袋
      box(x, g + 1.19, z - 0.172, 0.2, 0.18, 0.015, WARMW); // 火袋窗
      box(x, g + 1.19, z + 0.172, 0.2, 0.18, 0.015, WARMW);
      box(x, g + 1.42, z, 0.46, 0.1, 0.46, STONE); // 笠
      soupMerge(sphereSoup(x, g + 1.55, z, 0.09, 0.11, 0.09, 6, 4, () => STONE_D)); // 宝珠
      lanternGlows.push({
        x,
        y: g + 1.19,
        z,
        phase: hash2(Math.round(x * 10), Math.round(z * 10)) * 6.28,
      });
    };
    for (const lz of [24, 14, 4, -6, -16]) {
      lantern(-2.9, lz);
      lantern(2.9, lz);
    }

    /* 神社本殿（北侧高台）：石台基 + 红柱白墙 + 歇山顶 + 注连绳 + 赛钱箱 + 大铃铛 */
    const hg = heightAt(0, -30),
      plat = hg + 0.9;
    box(0, hg - 0.05, -30, 11, 1.9, 9, STONE); // 石台基
    for (let i = 0; i < 3; i++)
      box(0, hg + 0.3 * (i + 1) - 0.175, -24.4 - i * 0.55, 4.2, 0.35, 0.6, STONE_D); // 台阶
    for (const px of [-3.3, -1.1, 1.1, 3.3]) cylY(px, plat, -26.6, 0.16, 2.4, REDWOOD, 10); // 前柱
    for (const px of [-3.3, 3.3]) cylY(px, plat, -33.4, 0.16, 2.4, REDWOOD, 10);
    box(0, plat + 1.15, -30, 8, 2.3, 6, WHITEW); // 白墙身
    box(0, plat + 0.95, -26.94, 1.5, 1.9, 0.08, WOOD_D); // 正门
    box(0, plat + 0.35, -26.93, 2.2, 0.28, 0.1, WOOD); // 门槛
    hipRoof(0, plat + 2.55, -30, 5.0, 4.1, 1.75, 2.0, ROOF); // 歇山顶
    box(0, plat + 4.37, -30, 4.3, 0.24, 0.55, ROOF_R); // 屋脊
    box(-2.05, plat + 4.53, -30, 0.5, 0.3, 0.5, ROOF_R); // 脊端饰
    box(2.05, plat + 4.53, -30, 0.5, 0.3, 0.5, ROOF_R);
    box(0, plat + 2.0, -26.45, 6.6, 0.13, 0.13, STRAW); // 注连绳
    for (const px of [-2.4, -0.8, 0.8, 2.4]) {
      box(px, plat + 1.74, -26.45, 0.2, 0.4, 0.03, WHITEW); // 纸垂（折阶两段）
      box(px + 0.05, plat + 1.44, -26.45, 0.16, 0.24, 0.03, WHITEW);
    }
    box(0, plat + 0.42, -24.7, 1.7, 0.85, 1.0, WOOD); // 赛钱箱
    for (let i = -2; i <= 2; i++) box(i * 0.32, plat + 0.88, -24.7, 0.09, 0.07, 1.06, WOOD_D);
    cylY(-1.25, plat, -24.1, 0.07, 2.1, REDWOOD, 8); // 铃架
    cylY(1.25, plat, -24.1, 0.07, 2.1, REDWOOD, 8);
    box(0, plat + 2.12, -24.1, 2.8, 0.15, 0.18, REDWOOD);
    cylY(0, plat + 1.62, -24.1, 0.025, 0.5, STRAW, 6); // 铃绳
    soupMerge(sphereSoup(0, plat + 1.42, -24.1, 0.17, 0.19, 0.17, 8, 6, () => [0.26, 0.23, 0.18])); // 大铃铛

    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const bind = (loc, arr) => {
      const b = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, b);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(arr), gl.STATIC_DRAW);
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 3, gl.FLOAT, false, 0, 0);
    };
    bind(0, P);
    bind(1, Nr);
    bind(2, C);
    gl.bindVertexArray(null);
    return { vao, count: P.length / 3 };
  })();
  /* ================= 草叶（实例化） ================= */
  const GRASS_N = 4500;
  const grassProg = makeProg(
    `#version 300 es
    layout(location=0) in vec2 aCorner;
    layout(location=1) in vec3 iBase;
    layout(location=2) in vec4 iParam;  // scale, rot, phase, colorVar
    uniform mat4 uVP;
    uniform float uTime;
    out float vT; out float vVar; out float vFog;
    void main() {
      float scale = iParam.x, rot = iParam.y, phase = iParam.z;
      vVar = iParam.w; vT = aCorner.y;
      float width = 0.085 * scale * (1.0 - aCorner.y * 0.85);
      float c = cos(rot), s = sin(rot);
      vec3 wp = iBase + vec3((aCorner.x * width) * c, aCorner.y * 1.05 * scale, (aCorner.x * width) * s);
      float bend = aCorner.y * aCorner.y;
      float sway = sin(uTime * 1.8 + phase + iBase.x * 0.35 + iBase.z * 0.3) * 0.2;
      float lean = (fract(phase * 0.618) - 0.5) * 0.35;
      wp.x += (sway + lean) * bend;
      wp.z += sway * 0.55 * bend;
      vec4 cp = uVP * vec4(wp, 1.0);
      vFog = cp.w;
      gl_Position = cp;
    }`,
    `#version 300 es
    precision mediump float;
    in float vT; in float vVar; in float vFog;
    uniform vec3 uGrassLow, uGrassHigh, uSunColor, uAmbient, uFogColor;
    uniform float uFogK; uniform float uSunUp;
    out vec4 o;
    void main() {
      vec3 col = mix(uGrassLow, uGrassHigh, vT) * (0.88 + 0.24 * vVar);
      col *= uAmbient + uSunColor * max(uSunUp, 0.0);
      float f = 1.0 - exp(-vFog * vFog * uFogK);
      o = vec4(mix(col, uFogColor, f), 1.0);
    }`,
  );
  const grassU = uniforms(grassProg, [
    "uVP",
    "uTime",
    "uGrassLow",
    "uGrassHigh",
    "uSunColor",
    "uAmbient",
    "uFogColor",
    "uFogK",
    "uSunUp",
  ]);
  /* 草叶两张地图各一份实例缓冲（神社避让参道/高台，牧场只避池塘），切换瞬时无卡顿 */
  function buildGrassVao(excludeShrine) {
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const quad = new Float32Array([-0.5, 0, 0.5, 0, 0, 1]);
    const qb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, qb);
    gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    const inst = new Float32Array(GRASS_N * 7);
    let placed = 0,
      guard = 0;
    while (placed < GRASS_N && guard++ < GRASS_N * 20) {
      const x = (Math.random() * 2 - 1) * 46,
        z = (Math.random() * 2 - 1) * 46;
      const r = Math.hypot(x, z);
      if (r > 46 || Math.random() > 1 - (r / 46) * (r / 46) * 0.72) continue;
      if (Math.hypot(x - POND.x, z - POND.z) < POND.r + 1.5) continue; // 塘里不长草
      if (excludeShrine) {
        if (Math.abs(x) < 1.8 && Math.abs(z) < 33) continue; // 参道不长草
        if (Math.abs(x) < 7.5 && z < -22 && z > -37.5) continue; // 社殿高台不长草
      }
      const o = placed * 7;
      inst[o] = x;
      inst[o + 1] = heightAt(x, z) - 0.03;
      inst[o + 2] = z;
      inst[o + 3] = 0.65 + Math.random() * 0.85;
      inst[o + 4] = Math.random() * Math.PI;
      inst[o + 5] = Math.random() * Math.PI * 2;
      inst[o + 6] = Math.random();
      placed++;
    }
    const ib = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, ib);
    gl.bufferData(gl.ARRAY_BUFFER, inst, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 28, 0);
    gl.vertexAttribDivisor(1, 1);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 4, gl.FLOAT, false, 28, 12);
    gl.vertexAttribDivisor(2, 1);
    gl.bindVertexArray(null);
    return vao;
  }
  const grassVaoShrine = buildGrassVao(true),
    grassVaoPasture = buildGrassVao(false);

  /* ================= 程序化贴图集（团子/赛钱箱/竹/松/樱花/石头/雾带/荷叶/荷花/灯晕） ================= */
  const ATLAS = 512;
  const atlasCanvas = document.createElement("canvas");
  atlasCanvas.width = ATLAS;
  atlasCanvas.height = ATLAS;
  const REG = {
    dango: [0, 0, 128, 128],
    saisen: [128, 0, 128, 128],
    bamboo: [256, 0, 128, 128],
    pine: [384, 0, 128, 128],
    sakura: [0, 128, 128, 128],
    stone: [128, 128, 128, 128],
    lotus: [256, 128, 128, 128],
    glow: [384, 128, 64, 64],
    mist: [0, 256, 256, 64],
    lotusLeaf: [256, 256, 64, 64],
    peachTree: [0, 320, 128, 128], // 牧场地图用：094091f7 旧版普通粉花桃树
  };
  function uvRect(x, y, w, h) {
    return [x / ATLAS, 1 - (y + h) / ATLAS, (x + w) / ATLAS, 1 - y / ATLAS];
  }
  (function paintAtlas() {
    const c = atlasCanvas.getContext("2d");
    const softBlob = (x, y, rx, ry, color, alpha) => {
      const g = c.createRadialGradient(x, y, 0, x, y, Math.max(rx, ry));
      g.addColorStop(0, color.replace("A)", `${alpha})`));
      g.addColorStop(1, color.replace("A)", "0)"));
      c.fillStyle = g;
      c.save();
      c.translate(x, y);
      c.scale(1, ry / rx);
      c.translate(-x, -y);
      c.beginPath();
      c.arc(x, y, rx, 0, Math.PI * 2);
      c.fill();
      c.restore();
    };
    /* 团子串（蜜源 kind 0）：三色团子 */
    c.save();
    c.translate(0, 0);
    c.strokeStyle = "#c8a06a";
    c.lineWidth = 5;
    c.lineCap = "round";
    c.beginPath();
    c.moveTo(64, 126);
    c.lineTo(64, 22);
    c.stroke();
    const dango = (y, c1, c2) => {
      const g = c.createRadialGradient(58, y - 7, 4, 64, y, 19);
      g.addColorStop(0, c1);
      g.addColorStop(1, c2);
      c.fillStyle = g;
      c.beginPath();
      c.arc(64, y, 19, 0, Math.PI * 2);
      c.fill();
      c.fillStyle = "rgba(255,255,255,0.65)";
      c.beginPath();
      c.ellipse(57, y - 8, 5, 3.4, -0.6, 0, Math.PI * 2);
      c.fill();
    };
    dango(97, "#ffc9d9", "#f28cae"); // 粉
    dango(64, "#fff8f0", "#ead9c2"); // 白
    dango(31, "#cdebb6", "#92c47c"); // 绿
    c.restore();
    /* 赛钱箱（蜜源 kind 1）：小木箱 + 金币光点 */
    c.save();
    c.translate(128, 0);
    const boxG = c.createLinearGradient(0, 50, 0, 120);
    boxG.addColorStop(0, "#96622f");
    boxG.addColorStop(1, "#5d3a1a");
    c.fillStyle = boxG;
    c.beginPath();
    c.roundRect(22, 52, 84, 66, 6);
    c.fill();
    c.fillStyle = "#3d2712";
    for (let i = 0; i < 4; i++) c.fillRect(26, 58 + i * 9, 76, 3.5); // 箱顶木缝
    c.fillStyle = "#7a4c22";
    c.fillRect(22, 112, 84, 8); // 底沿
    const coin = c.createRadialGradient(58, 34, 2, 64, 40, 14);
    coin.addColorStop(0, "#fff3b8");
    coin.addColorStop(0.7, "#ffd76e");
    coin.addColorStop(1, "#d9a441");
    c.fillStyle = coin;
    c.beginPath();
    c.arc(64, 40, 13, 0, Math.PI * 2);
    c.fill();
    c.fillStyle = "rgba(255,255,255,0.9)";
    c.beginPath(); // 金币闪光（四角星）
    c.moveTo(84, 18);
    c.quadraticCurveTo(86, 26, 94, 28);
    c.quadraticCurveTo(86, 30, 84, 38);
    c.quadraticCurveTo(82, 30, 74, 28);
    c.quadraticCurveTo(82, 26, 84, 18);
    c.fill();
    c.restore();
    /* 竹丛：细高绿杆 + 叶簇 */
    c.save();
    c.translate(256, 0);
    const stalk = (x0, x1, w, lean) => {
      const g = c.createLinearGradient(x0, 0, x1, 0);
      g.addColorStop(0, "#5f9e52");
      g.addColorStop(0.5, "#8cc47a");
      g.addColorStop(1, "#4d8a44");
      c.fillStyle = g;
      c.beginPath();
      c.moveTo(x0, 128);
      c.lineTo(x0 + lean, 6);
      c.lineTo(x1 + lean, 6);
      c.lineTo(x1, 128);
      c.closePath();
      c.fill();
      c.strokeStyle = "rgba(46,92,40,0.8)";
      c.lineWidth = 2;
      for (let y = 26; y < 128; y += 24) {
        const t = y / 128;
        c.beginPath();
        c.moveTo(x0 + lean * (1 - t), y);
        c.lineTo(x1 + lean * (1 - t), y);
        c.stroke();
      }
    };
    const bambooLeaf = (x, y, ang, len) => {
      c.save();
      c.translate(x, y);
      c.rotate(ang);
      c.fillStyle = "#6db45e";
      c.beginPath();
      c.ellipse(len / 2, 0, len / 2, 3.4, 0, 0, Math.PI * 2);
      c.fill();
      c.restore();
    };
    stalk(30, 40, 10, 6);
    stalk(58, 68, 10, -4);
    stalk(88, 97, 9, 9);
    bambooLeaf(38, 22, -0.5, 26);
    bambooLeaf(38, 34, 0.35, 22);
    bambooLeaf(64, 16, 0.15, 28);
    bambooLeaf(64, 30, -0.4, 24);
    bambooLeaf(92, 26, 0.5, 22);
    bambooLeaf(92, 40, -0.2, 20);
    c.restore();
    /* 松树：层叠伞盖 */
    c.save();
    c.translate(384, 0);
    c.fillStyle = "#5d4634";
    c.fillRect(60, 96, 9, 32);
    const pineLayer = (y, w, col) => {
      c.fillStyle = col;
      c.beginPath();
      c.moveTo(64 - w / 2, y);
      c.quadraticCurveTo(64, y - w * 0.42, 64 + w / 2, y);
      c.quadraticCurveTo(64, y - w * 0.16, 64 - w / 2, y);
      c.fill();
    };
    pineLayer(108, 96, "#2e4d3a");
    pineLayer(84, 74, "#38604a");
    pineLayer(62, 52, "#487a58");
    pineLayer(44, 30, "#5c9468");
    c.restore();
    /* 樱花树：褐干 + 淡粉花团 */
    c.save();
    c.translate(0, 128);
    c.strokeStyle = "#6b4a36";
    c.lineWidth = 8;
    c.lineCap = "round";
    c.beginPath();
    c.moveTo(64, 128);
    c.quadraticCurveTo(60, 100, 64, 82);
    c.stroke();
    c.lineWidth = 5;
    c.beginPath();
    c.moveTo(64, 96);
    c.quadraticCurveTo(48, 88, 42, 74);
    c.stroke();
    c.beginPath();
    c.moveTo(64, 92);
    c.quadraticCurveTo(80, 86, 86, 72);
    c.stroke();
    const blossom = (x, y, r, col) => softBlob(x, y, r, r * 0.85, col, 0.95);
    blossom(46, 62, 22, "rgba(255,206,222,A)");
    blossom(82, 58, 24, "rgba(255,222,234,A)");
    blossom(64, 44, 24, "rgba(255,236,244,A)");
    blossom(56, 76, 18, "rgba(255,192,214,A)");
    blossom(78, 80, 16, "rgba(255,214,228,A)");
    c.fillStyle = "rgba(255,255,255,0.9)";
    for (let i = 0; i < 14; i++) {
      const a = hash2(i, 7) * Math.PI * 2,
        r = 8 + hash2(i, 13) * 22;
      c.beginPath();
      c.arc(64 + Math.cos(a) * r, 58 + (hash2(i, 29) - 0.5) * 36, 1.6, 0, Math.PI * 2);
      c.fill();
    }
    c.restore();
    /* 荷花（池塘装饰，蜜源已换成团子） */
    c.save();
    c.translate(256, 128);
    c.strokeStyle = "#4d7d46";
    c.lineWidth = 6;
    c.lineCap = "round";
    c.beginPath();
    c.moveTo(64, 126);
    c.quadraticCurveTo(60, 98, 64, 72);
    c.stroke();
    for (let i = 0; i < 8; i++) {
      const a = (i / 8) * Math.PI * 2;
      const g = c.createLinearGradient(64, 30, 64, 62);
      g.addColorStop(0, i % 2 ? "#ffe9f2" : "#ffd3e4");
      g.addColorStop(1, "#f78fb8");
      c.fillStyle = g;
      c.beginPath();
      c.ellipse(64 + Math.cos(a) * 14, 48 + Math.sin(a) * 12, 12, 6.5, a, 0, Math.PI * 2);
      c.fill();
    }
    c.fillStyle = "#ffd76e";
    c.beginPath();
    c.arc(64, 48, 8, 0, Math.PI * 2);
    c.fill();
    c.fillStyle = "#e8a83e";
    for (let i = 0; i < 5; i++) {
      c.beginPath();
      c.arc(
        64 + Math.cos((i / 5) * Math.PI * 2) * 4,
        48 + Math.sin((i / 5) * Math.PI * 2) * 4,
        1.4,
        0,
        Math.PI * 2,
      );
      c.fill();
    }
    c.restore();
    /* 石灯笼暖光晕（夜间 sprite） */
    c.save();
    c.translate(384, 128);
    softBlob(32, 32, 26, 26, "rgba(255,236,180,A)", 0.95);
    softBlob(32, 32, 12, 12, "rgba(255,250,230,A)", 0.95);
    c.restore();
    /* 桃树（牧场地图）：094091f7 旧版普通粉花 */
    c.save();
    c.translate(0, 320);
    c.strokeStyle = "#6b4a36";
    c.lineWidth = 8;
    c.lineCap = "round";
    c.beginPath();
    c.moveTo(64, 128);
    c.quadraticCurveTo(60, 100, 64, 82);
    c.stroke();
    c.lineWidth = 5;
    c.beginPath();
    c.moveTo(64, 96);
    c.quadraticCurveTo(48, 88, 42, 74);
    c.stroke();
    c.beginPath();
    c.moveTo(64, 92);
    c.quadraticCurveTo(80, 86, 86, 72);
    c.stroke();
    const peachBloom = (x, y, r, col) => softBlob(x, y, r, r * 0.85, col, 0.95);
    peachBloom(46, 62, 22, "rgba(255,178,204,A)");
    peachBloom(82, 58, 24, "rgba(255,194,214,A)");
    peachBloom(64, 44, 24, "rgba(255,214,228,A)");
    peachBloom(56, 76, 18, "rgba(255,158,196,A)");
    peachBloom(78, 80, 16, "rgba(255,178,204,A)");
    c.fillStyle = "rgba(255,255,255,0.85)";
    for (let i = 0; i < 14; i++) {
      const a = hash2(i, 7) * Math.PI * 2,
        r = 8 + hash2(i, 13) * 22;
      c.beginPath();
      c.arc(64 + Math.cos(a) * r, 58 + (hash2(i, 29) - 0.5) * 36, 1.6, 0, Math.PI * 2);
      c.fill();
    }
    c.restore();
    /* 石头（山水点缀） */
    c.save();
    c.translate(128, 128);
    const sg = c.createLinearGradient(0, 30, 0, 118);
    sg.addColorStop(0, "#c2c9cd");
    sg.addColorStop(1, "#75808a");
    c.fillStyle = sg;
    c.beginPath();
    c.moveTo(26, 112);
    c.quadraticCurveTo(20, 66, 52, 48);
    c.quadraticCurveTo(82, 32, 100, 60);
    c.quadraticCurveTo(112, 84, 102, 112);
    c.closePath();
    c.fill();
    c.strokeStyle = "rgba(255,255,255,0.5)";
    c.lineWidth = 4;
    c.beginPath();
    c.moveTo(48, 58);
    c.quadraticCurveTo(64, 46, 82, 54);
    c.stroke();
    c.restore();
    /* 雾带：横向软白带 */
    c.save();
    c.translate(0, 256);
    for (let i = 0; i < 7; i++) {
      const x = 24 + i * 34 + hash2(i, 3) * 14,
        y = 26 + (hash2(i, 9) - 0.5) * 16;
      softBlob(x, y, 44, 13 + hash2(i, 5) * 8, "rgba(255,255,255,A)", 0.5);
    }
    c.restore();
    /* 荷叶 */
    c.save();
    c.translate(256, 256);
    const lg = c.createRadialGradient(26, 24, 4, 32, 32, 30);
    lg.addColorStop(0, "#7cc46e");
    lg.addColorStop(1, "#3e7d44");
    c.fillStyle = lg;
    c.beginPath();
    c.arc(32, 32, 28, 0.35, Math.PI * 2 - 0.35);
    c.lineTo(32, 32);
    c.closePath();
    c.fill();
    c.strokeStyle = "rgba(46,92,52,0.8)";
    c.lineWidth = 1.5;
    for (let i = 0; i < 8; i++) {
      const a = 0.5 + (i / 8) * (Math.PI * 2 - 1);
      c.beginPath();
      c.moveTo(32, 32);
      c.lineTo(32 + Math.cos(a) * 26, 32 + Math.sin(a) * 26);
      c.stroke();
    }
    c.restore();
  })();
  const atlasTex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, atlasTex);
  gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, true);
  gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, atlasCanvas);
  gl.generateMipmap(gl.TEXTURE_2D);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
  gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false);

  /* ================= 广告牌精灵（实例化：植被/蜜源/雾带共用一个 draw call） ================= */
  const MAX_SPRITES = 96;
  const spriteProg = makeProg(
    `#version 300 es
    layout(location=0) in vec2 aCorner;
    layout(location=1) in vec3 iCenter;
    layout(location=2) in vec2 iSize;
    layout(location=3) in vec4 iUV;
    layout(location=4) in vec4 iMisc;  // swayPhase, alpha, rot, swayAmp
    uniform mat4 uVP;
    uniform vec3 uRight, uUp;
    uniform float uTime;
    out vec2 vUV; out float vAlpha; out float vFog;
    void main() {
      float c = cos(iMisc.z), s = sin(iMisc.z);
      vec2 rc = vec2(aCorner.x * c - aCorner.y * s, aCorner.x * s + aCorner.y * c);
      vec3 wp = iCenter + uRight * (rc.x * iSize.x) + uUp * (rc.y * iSize.y);
      float top = aCorner.y + 0.5;
      wp.x += sin(uTime * 1.6 + iMisc.x) * iMisc.w * top;
      wp.z += cos(uTime * 1.3 + iMisc.x * 1.7) * iMisc.w * top * 0.6;
      vUV = mix(iUV.xy, iUV.zw, aCorner + 0.5);
      vAlpha = iMisc.y;
      vec4 cp = uVP * vec4(wp, 1.0);
      vFog = cp.w;
      gl_Position = cp;
    }`,
    `#version 300 es
    precision mediump float;
    in vec2 vUV; in float vAlpha; in float vFog;
    uniform sampler2D uTex;
    uniform vec3 uFogColor;
    uniform float uFogK;
    out vec4 o;
    void main() {
      vec4 t = texture(uTex, vUV);
      if (t.a * vAlpha < 0.02) discard;
      float f = 1.0 - exp(-vFog * vFog * uFogK);
      // 预乘 alpha：雾色按覆盖率加权，保持 premultiplied 不变式
      vec3 col = mix(t.rgb, uFogColor * t.a, f);
      o = vec4(col, t.a * vAlpha);
    }`,
  );
  const spriteU = uniforms(spriteProg, [
    "uVP",
    "uRight",
    "uUp",
    "uTime",
    "uTex",
    "uFogColor",
    "uFogK",
  ]);
  const spriteVao = gl.createVertexArray();
  const spriteInstBuf = gl.createBuffer();
  {
    gl.bindVertexArray(spriteVao);
    const quad = new Float32Array([
      -0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, -0.5, 0.5, 0.5, -0.5, 0.5,
    ]);
    const qb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, qb);
    gl.bufferData(gl.ARRAY_BUFFER, quad, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, spriteInstBuf);
    gl.bufferData(gl.ARRAY_BUFFER, MAX_SPRITES * 13 * 4, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 52, 0);
    gl.vertexAttribDivisor(1, 1);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 2, gl.FLOAT, false, 52, 12);
    gl.vertexAttribDivisor(2, 1);
    gl.enableVertexAttribArray(3);
    gl.vertexAttribPointer(3, 4, gl.FLOAT, false, 52, 20);
    gl.vertexAttribDivisor(3, 1);
    gl.enableVertexAttribArray(4);
    gl.vertexAttribPointer(4, 4, gl.FLOAT, false, 52, 36);
    gl.vertexAttribDivisor(4, 1);
    gl.bindVertexArray(null);
  }
  const spriteData = new Float32Array(MAX_SPRITES * 13);

  /* 装饰：两张地图各建一份静态集合（神社避让参道/高台/鸟居，牧场只避池塘），初始化都建好，切换只换引用 */
  const UV = {};
  for (const k of Object.keys(REG)) UV[k] = uvRect(...REG[k]);
  let mapKind = "shrine"; // 神社（默认）/ 牧场（青绿牧场，094091f7 旧版装饰恢复）
  try {
    mapKind = localStorage.getItem("flyBrainMap") === "pasture" ? "pasture" : "shrine";
  } catch (_) {}
  const clearOfPond = (x, z, margin) => Math.hypot(x - POND.x, z - POND.z) > POND.r + margin;
  const inShrineZone = (x, z) =>
    (Math.abs(x) < 4.2 && z > -35 && z < 34) || (Math.abs(x) < 8.5 && z < -22); // 参道/高台/鸟居
  const scatter = (count, rMin, rMax, avoidShrine) => {
    const out = [];
    let guard = 0;
    while (out.length < count && guard++ < count * 30) {
      const a = Math.random() * Math.PI * 2,
        r = rMin + Math.random() * (rMax - rMin);
      const x = Math.cos(a) * r,
        z = Math.sin(a) * r;
      if (!clearOfPond(x, z, 3)) continue;
      if (avoidShrine && inShrineZone(x, z)) continue;
      out.push([x, z]);
    }
    return out;
  };
  const pushPlants = (out, sc, treeUv) => {
    for (const [i, [x, z]] of sc(4, 12, 40).entries()) {
      const h = 4.2 + Math.random();
      out.push({
        x,
        y: heightAt(x, z) + h * 0.48,
        z,
        w: 2.0,
        h,
        uv: UV.bamboo,
        phase: i * 1.7,
        sway: 0.07,
      });
    }
    for (const [i, [x, z]] of sc(3, 14, 42).entries()) {
      const h = 4.0 + Math.random() * 0.8;
      out.push({
        x,
        y: heightAt(x, z) + h * 0.47,
        z,
        w: 3.4,
        h,
        uv: UV.pine,
        phase: i * 2.3,
        sway: 0.03,
      });
    }
    for (const [i, [x, z]] of sc(3, 10, 38).entries()) {
      const h = 3.6 + Math.random() * 0.6;
      out.push({
        x,
        y: heightAt(x, z) + h * 0.46,
        z,
        w: 3.6,
        h,
        uv: treeUv,
        phase: i * 3.1,
        sway: 0.05,
      });
    }
    for (const [i, [x, z]] of sc(3, 8, 40).entries()) {
      const s = 1.0 + Math.random() * 0.9;
      out.push({
        x,
        y: heightAt(x, z) + s * 0.34,
        z,
        w: s,
        h: s * 0.72,
        uv: UV.stone,
        phase: i * 1.1,
        sway: 0,
      });
    }
  };
  const pushPondDecor = (out) => {
    // 塘面荷叶与一朵荷花（两张地图共享）
    for (let i = 0; i < 5; i++) {
      const a = hash2(i, 31) * Math.PI * 2,
        r = 1.5 + hash2(i, 37) * (WATER_R - 3);
      const x = POND.x + Math.cos(a) * r,
        z = POND.z + Math.sin(a) * r;
      out.push({
        x,
        y: WATER_Y + 0.06,
        z,
        w: 0.95,
        h: 0.95,
        uv: UV.lotusLeaf,
        phase: i * 1.9,
        sway: 0.015,
      });
    }
    out.push({
      x: POND.x + 2.2,
      y: WATER_Y + 0.42,
      z: POND.z - 1.4,
      w: 0.85,
      h: 0.95,
      uv: UV.lotus,
      phase: 4.4,
      sway: 0.03,
    });
  };
  function buildDecorShrine() {
    const out = [];
    pushPlants(out, (c, a, b) => scatter(c, a, b, true), UV.sakura);
    // 樱花树：参道入口两侧成对
    for (const [i, [x, z]] of [
      [-5.2, 26.5],
      [5.2, 26.5],
    ].entries()) {
      const h = 3.6 + i * 0.2;
      out.push({
        x,
        y: heightAt(x, z) + h * 0.46,
        z,
        w: 3.6,
        h,
        uv: UV.sakura,
        phase: 9 + i * 1.7,
        sway: 0.05,
      });
    }
    pushPondDecor(out);
    return out;
  }
  function buildDecorPasture() {
    const out = [];
    pushPlants(out, (c, a, b) => scatter(c, a, b, false), UV.peachTree); // 牧场用回旧版粉花桃树
    pushPondDecor(out);
    return out;
  }
  const decorSets = { shrine: buildDecorShrine(), pasture: buildDecorPasture() };
  let decor = decorSets[mapKind];
  /* 山间雾带（缓慢环场漂移，每帧更新位置） */
  const mists = [];
  for (let i = 0; i < 9; i++) {
    mists.push({
      ang: (i / 9) * Math.PI * 2 + hash2(i, 51),
      r: 92 + hash2(i, 57) * 120,
      y: 7 + hash2(i, 63) * 22,
      w: 42 + hash2(i, 69) * 30,
      h: 6 + hash2(i, 75) * 5,
      speed: 0.0035 + hash2(i, 81) * 0.004,
      phase: hash2(i, 87) * 6.28,
    });
  }

  /* 蜜源（后端 foods 驱动，吃掉消失/重生）：团子串与赛钱箱 */
  let foodSprites = [],
    foodsSig = "";
  function rebuildFoods(foods) {
    foodSprites = (foods || []).map((f) => {
      const isFruit = f.kind === 1;
      const w = isFruit ? 1.5 : 1.15,
        h = isFruit ? 1.35 : 1.5;
      return {
        x: f.x,
        y: heightAt(f.x, f.z) + h * 0.44,
        z: f.z,
        w,
        h,
        uv: isFruit ? UV.saisen : UV.dango,
        phase: (f.id * 2.39) % 6.28,
        sway: 0.09,
      };
    });
  }

  /* ================= 萤火虫（夜晚，亮度随脑活动呼吸） ================= */
  const FF_N = 14;
  const fireflyProg = makeProg(
    `#version 300 es
    layout(location=0) in vec4 iSeed;  // xyz base, w phase
    layout(location=1) in float iIdx;
    uniform mat4 uVP;
    uniform float uTime, uNight, uActivity;
    out float vA;
    void main() {
      float p = iSeed.w;
      vec3 wp = iSeed.xyz + vec3(sin(uTime * 0.45 + p) * 2.2, sin(uTime * 0.7 + p * 1.7) * 0.9, cos(uTime * 0.55 + p * 0.8) * 2.2);
      float pulse = 0.5 + 0.5 * sin(uTime * 2.0 + p * 3.1);
      float gate = clamp(uActivity * 1.7 - iIdx * 0.7, 0.05, 1.0);
      vA = uNight * gate * pulse;
      vec4 cp = uVP * vec4(wp, 1.0);
      gl_Position = cp;
      gl_PointSize = clamp(170.0 / cp.w, 3.0, 26.0);
    }`,
    `#version 300 es
    precision mediump float;
    in float vA;
    out vec4 o;
    void main() {
      vec2 d = gl_PointCoord - 0.5;
      float r2 = dot(d, d);
      if (r2 > 0.25) discard;
      float g = smoothstep(0.25, 0.0, r2);
      o = vec4(vec3(1.0, 0.85, 0.45) * g * vA, g * vA);
    }`,
  );
  const fireflyU = uniforms(fireflyProg, ["uVP", "uTime", "uNight", "uActivity"]);
  const fireflyVao = (() => {
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const data = new Float32Array(FF_N * 5);
    for (let i = 0; i < FF_N; i++) {
      const a = Math.random() * Math.PI * 2,
        r = 6 + Math.random() * 24;
      const x = Math.cos(a) * r,
        z = Math.sin(a) * r;
      data[i * 5] = x;
      data[i * 5 + 1] = heightAt(x, z) + 1.1 + Math.random() * 1.6;
      data[i * 5 + 2] = z;
      data[i * 5 + 3] = Math.random() * Math.PI * 2;
      data[i * 5 + 4] = FF_N > 1 ? i / (FF_N - 1) : 0;
    }
    const vb = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vb);
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 4, gl.FLOAT, false, 20, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 1, gl.FLOAT, false, 20, 16);
    gl.bindVertexArray(null);
    return vao;
  })();

  /* ================= Q 版博丽灵梦（程序化二头身巫女：刚体汤合并动态缓冲单 draw + 高光 emissive 小 draw） ================= */
  function sphereSoup(cx, cy, cz, rx, ry, rz, wSeg, hSeg, colorFn) {
    const out = [];
    const P = (phi, th) => {
      const sp = Math.sin(phi);
      const lx = sp * Math.cos(th),
        ly = Math.cos(phi),
        lz = sp * Math.sin(th);
      const nl = Math.hypot(lx / rx, ly / ry, lz / rz) || 1;
      return [cx + lx * rx, cy + ly * ry, cz + lz * rz, lx / rx / nl, ly / ry / nl, lz / rz / nl];
    };
    const push = (v, col) => out.push(v[0], v[1], v[2], v[3], v[4], v[5], col[0], col[1], col[2]);
    for (let i = 0; i < hSeg; i++) {
      const p0 = (i / hSeg) * Math.PI,
        p1 = ((i + 1) / hSeg) * Math.PI;
      for (let j = 0; j < wSeg; j++) {
        const t0 = (j / wSeg) * Math.PI * 2,
          t1 = ((j + 1) / wSeg) * Math.PI * 2;
        const a = P(p0, t0),
          b = P(p1, t0),
          d = P(p0, t1),
          e = P(p1, t1);
        const cA = colorFn(a),
          cB = colorFn(b),
          cD = colorFn(d),
          cE = colorFn(e);
        push(a, cA);
        push(b, cB);
        push(d, cD);
        push(d, cD);
        push(b, cB);
        push(e, cE);
      }
    }
    return out;
  }
  /* 单位细圆柱（半径 1，y∈[0,1]，侧面），袖/腿/缎带共用 */
  const TUBE = (() => {
    const out = [];
    const SEG = 6;
    for (let j = 0; j < SEG; j++) {
      const t0 = (j / SEG) * Math.PI * 2,
        t1 = ((j + 1) / SEG) * Math.PI * 2;
      const c0 = Math.cos(t0),
        s0 = Math.sin(t0),
        c1 = Math.cos(t1),
        s1 = Math.sin(t1);
      const a = [c0, 0, s0, c0, 0, s0],
        b = [c0, 1, s0, c0, 0, s0],
        d = [c1, 0, s1, c1, 0, s1],
        e = [c1, 1, s1, c1, 0, s1];
      out.push(...a, ...b, ...d, ...d, ...b, ...e);
    }
    return out;
  })();
  /* 只存位置的球汤（眼睛高光用） */
  function spherePos(cx, cy, cz, r, wSeg, hSeg) {
    const out = [];
    const P = (phi, th) => {
      const sp = Math.sin(phi);
      return [cx + sp * Math.cos(th) * r, cy + Math.cos(phi) * r, cz + sp * Math.sin(th) * r];
    };
    for (let i = 0; i < hSeg; i++) {
      const p0 = (i / hSeg) * Math.PI,
        p1 = ((i + 1) / hSeg) * Math.PI;
      for (let j = 0; j < wSeg; j++) {
        const t0 = (j / wSeg) * Math.PI * 2,
          t1 = ((j + 1) / wSeg) * Math.PI * 2;
        const a = P(p0, t0),
          b = P(p1, t0),
          d = P(p0, t1),
          e = P(p1, t1);
        out.push(...a, ...b, ...d, ...d, ...b, ...e);
      }
    }
    return out;
  }
  const C_HAIR = [0.1, 0.08, 0.09],
    C_SKIN = [1.0, 0.87, 0.75],
    C_WHITE = [0.96, 0.95, 0.93],
    C_RED = [0.78, 0.12, 0.16],
    C_BOW = [0.86, 0.1, 0.14],
    C_SHOE = [0.5, 0.2, 0.14],
    C_EYE = [0.16, 0.1, 0.12];
  /* 圆台汤（袴/袖/小腿）：底半径 rB、顶半径 rT、高 h，侧面 + 底盖，colorFn 按高度 0..1 分带 */
  function coneSoup(cx, cy, cz, rB, rT, h, segs, colorFn) {
    const out = [];
    const slope = (rB - rT) / h;
    const nl = Math.hypot(1, slope);
    const pushV = (px, py, pz, nx, ny, nz) => {
      const cc = colorFn((py - cy) / h);
      out.push(px, py, pz, nx, ny, nz, cc[0], cc[1], cc[2]);
    };
    for (let j = 0; j < segs; j++) {
      const t0 = (j / segs) * Math.PI * 2,
        t1 = ((j + 1) / segs) * Math.PI * 2;
      const c0 = Math.cos(t0),
        s0 = Math.sin(t0),
        c1 = Math.cos(t1),
        s1 = Math.sin(t1);
      // 侧面四边形（法线带锥度）
      pushV(cx + c0 * rB, cy, cz + s0 * rB, c0 / nl, slope / nl, s0 / nl);
      pushV(cx + c1 * rB, cy, cz + s1 * rB, c1 / nl, slope / nl, s1 / nl);
      pushV(cx + c0 * rT, cy + h, cz + s0 * rT, c0 / nl, slope / nl, s0 / nl);
      pushV(cx + c0 * rT, cy + h, cz + s0 * rT, c0 / nl, slope / nl, s0 / nl);
      pushV(cx + c1 * rB, cy, cz + s1 * rB, c1 / nl, slope / nl, s1 / nl);
      pushV(cx + c1 * rT, cy + h, cz + s1 * rT, c1 / nl, slope / nl, s1 / nl);
      // 底盖（朝下）
      pushV(cx, cy, cz, 0, -1, 0);
      pushV(cx + c1 * rB, cy, cz + s1 * rB, 0, -1, 0);
      pushV(cx + c0 * rB, cy, cz + s0 * rB, 0, -1, 0);
    }
    return out;
  }
  /* 蝴蝶结双耳：椭球建好后绕 Z 轴倾转（顶点与法线同转） */
  function rotZSoup(soup, cx, cy, ang) {
    const c = Math.cos(ang),
      s = Math.sin(ang);
    for (let i = 0; i < soup.length; i += 9) {
      const x = soup[i] - cx,
        y = soup[i + 1] - cy;
      soup[i] = cx + x * c - y * s;
      soup[i + 1] = cy + x * s + y * c;
      const nx = soup[i + 3],
        ny = soup[i + 4];
      soup[i + 3] = nx * c - ny * s;
      soup[i + 4] = nx * s + ny * c;
    }
    return soup;
  }
  const RIGID_SOUP = (() => {
    const bowL = rotZSoup(
      sphereSoup(0.155, 1.38, 0.03, 0.135, 0.065, 0.085, 8, 5, () => C_BOW),
      0.155,
      1.38,
      -0.5,
    );
    const bowR = rotZSoup(
      sphereSoup(-0.155, 1.38, 0.03, 0.135, 0.065, 0.085, 8, 5, () => C_BOW),
      -0.155,
      1.38,
      0.5,
    );
    return new Float32Array([
      ...coneSoup(0, 0.26, 0, 0.34, 0.2, 0.44, 12, (u) => (u < 0.16 ? C_WHITE : C_RED)), // 红袴白裾
      ...sphereSoup(0, 0.84, 0, 0.21, 0.19, 0.155, 10, 7, () => C_WHITE), // 白衣上身
      ...sphereSoup(0, 0.74, -0.14, 0.05, 0.06, 0.03, 6, 4, () => C_BOW), // 领口红领巾
      ...sphereSoup(0, 1.1, 0, 0.27, 0.26, 0.26, 12, 9, () => C_SKIN), // 头
      ...sphereSoup(0, 1.13, 0.045, 0.285, 0.275, 0.285, 12, 9, () => C_HAIR), // 发盖
      ...sphereSoup(0, 0.86, 0.17, 0.21, 0.4, 0.12, 10, 7, () => C_HAIR), // 后长发
      ...sphereSoup(0.245, 0.98, 0.02, 0.055, 0.2, 0.06, 6, 5, () => C_HAIR), // 侧发
      ...sphereSoup(-0.245, 0.98, 0.02, 0.055, 0.2, 0.06, 6, 5, () => C_HAIR),
      ...sphereSoup(0.105, 1.1, -0.238, 0.035, 0.05, 0.02, 6, 4, () => C_EYE), // 眼（暗底）
      ...sphereSoup(-0.105, 1.1, -0.238, 0.035, 0.05, 0.02, 6, 4, () => C_EYE),
      ...bowL,
      ...bowR, // 大红蝴蝶结双耳
      ...sphereSoup(0, 1.36, 0, 0.05, 0.05, 0.05, 6, 4, () => C_WHITE), // 结心
    ]);
  })();
  /* 眼睛高光点（emissive，不走光照）：vision 越强越亮，eating 双闪 */
  const EYE_HI_L = spherePos(0.117, 1.11, -0.262, 0.018, 5, 4),
    EYE_HI_R = spherePos(-0.117, 1.11, -0.262, 0.018, 5, 4);
  const eyeBuf = new Float32Array(((EYE_HI_L.length + EYE_HI_R.length) / 3) * 6);
  let eyeHiVerts = 0;
  const eyeProg = makeProg(
    `#version 300 es
    layout(location=0) in vec3 aPos;
    layout(location=1) in vec3 aColor;
    uniform mat4 uVP;
    out vec3 vC; out float vFog;
    void main() {
      vC = aColor;
      vec4 cp = uVP * vec4(aPos, 1.0);
      vFog = cp.w;
      gl_Position = cp;
    }`,
    `#version 300 es
    precision mediump float;
    in vec3 vC; in float vFog;
    uniform vec3 uFogColor;
    uniform float uFogK;
    out vec4 o;
    void main() {
      float f = 1.0 - exp(-vFog * vFog * uFogK);
      o = vec4(mix(vC, uFogColor, f), 1.0);
    }`,
  );
  const eyeU = uniforms(eyeProg, ["uVP", "uFogColor", "uFogK"]);
  const eyeVao = gl.createVertexArray();
  const eyeVbo = gl.createBuffer();
  {
    gl.bindVertexArray(eyeVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, eyeVbo);
    gl.bufferData(gl.ARRAY_BUFFER, eyeBuf.byteLength, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 24, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 24, 12);
    gl.bindVertexArray(null);
  }
  const BODY_MAX_FLOATS = 40000;
  const flyBuf = new Float32Array(BODY_MAX_FLOATS);
  const flyVao = gl.createVertexArray();
  const flyVbo = gl.createBuffer();
  {
    gl.bindVertexArray(flyVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, flyVbo);
    gl.bufferData(gl.ARRAY_BUFFER, BODY_MAX_FLOATS * 4, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 36, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 36, 12);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 3, gl.FLOAT, false, 36, 24);
    gl.bindVertexArray(null);
  }
  /* 姿态：poseT 0=漂浮 1=落地；restT 0=站/走 0.4=坐 1=躺 */
  let poseT = 0,
    restT = 0;
  function fillReimu(t, dt, poseTarget) {
    poseT += (poseTarget - poseT) * (1 - Math.exp(-dt * 2)); // 起落 0.5s 过渡
    const airT = 1 - poseT;
    const restTarget = flyState === "sleeping" ? 1 : flyState === "resting" ? 0.4 : 0;
    restT += (restTarget - restT) * (1 - Math.exp(-dt * 2.5));
    const stepFreq = 6 + 2 * Math.min(1, flySpeed / 3); // 步频 6-8Hz 随速度缩放
    const bodyPitch = -0.22 * airT + 1.3 * restT; // 漂浮微前倾 / 睡觉后仰躺下
    const cx = Math.cos(bodyPitch),
      sx = Math.sin(bodyPitch);
    const cy = -Math.sin(flyRot),
      sy = -Math.cos(flyRot);
    const bounce = gaitT * Math.abs(Math.sin(t * Math.PI * 2 * stepFreq)) * 0.03; // 走路身体轻弹
    const ox = flyPos.x,
      oy = flyPos.y + bounce,
      oz = flyPos.z;
    const rot = (x, y, z, out) => {
      const y1 = y * cx - z * sx,
        z1 = y * sx + z * cx;
      out[0] = x * cy + z1 * sy + ox;
      out[1] = y1 + oy;
      out[2] = -x * sy + z1 * cy + oz;
    };
    const rotN = (x, y, z, out) => {
      const y1 = y * cx - z * sx,
        z1 = y * sx + z * cx;
      out[0] = x * cy + z1 * sy;
      out[1] = y1;
      out[2] = -x * sy + z1 * cy;
    };
    let o = 0;
    const w = [0, 0, 0];
    for (let i = 0; i < RIGID_SOUP.length; i += 9) {
      rot(RIGID_SOUP[i], RIGID_SOUP[i + 1], RIGID_SOUP[i + 2], w);
      flyBuf[o++] = w[0];
      flyBuf[o++] = w[1];
      flyBuf[o++] = w[2];
      rotN(RIGID_SOUP[i + 3], RIGID_SOUP[i + 4], RIGID_SOUP[i + 5], w);
      flyBuf[o++] = w[0];
      flyBuf[o++] = w[1];
      flyBuf[o++] = w[2];
      flyBuf[o++] = RIGID_SOUP[i + 6];
      flyBuf[o++] = RIGID_SOUP[i + 7];
      flyBuf[o++] = RIGID_SOUP[i + 8];
    }
    /* 圆台部件（袖/腿）：逐顶点变半径与分带色 */
    const writeCone = (p0, dir, len, r0, r1, colFn) => {
      const dl = Math.hypot(dir[0], dir[1], dir[2]) || 1e-4;
      const wx = dir[0] / dl,
        wy = dir[1] / dl,
        wz = dir[2] / dl;
      const ax = Math.abs(wy) < 0.9 ? 0 : 1,
        ay = Math.abs(wy) < 0.9 ? 1 : 0,
        az = 0;
      let ux = ay * wz - az * wy,
        uy = az * wx - ax * wz,
        uz = ax * wy - ay * wx;
      const ul = Math.hypot(ux, uy, uz) || 1;
      ux /= ul;
      uy /= ul;
      uz /= ul;
      const vx = wy * uz - wz * uy,
        vy = wz * ux - wx * uz,
        vz = wx * uy - wy * ux;
      for (let i = 0; i < TUBE.length; i += 6) {
        const u01 = TUBE[i + 1];
        const r = r0 + (r1 - r0) * u01;
        const lx = TUBE[i] * r,
          lz = TUBE[i + 2] * r;
        flyBuf[o++] = p0[0] + ux * lx + wx * u01 * len + vx * lz;
        flyBuf[o++] = p0[1] + uy * lx + wy * u01 * len + vy * lz;
        flyBuf[o++] = p0[2] + uz * lx + wz * u01 * len + vz * lz;
        flyBuf[o++] = ux * TUBE[i + 3] + vx * TUBE[i + 5];
        flyBuf[o++] = uy * TUBE[i + 3] + vy * TUBE[i + 5];
        flyBuf[o++] = uz * TUBE[i + 3] + vz * TUBE[i + 5];
        const cc = colFn(u01);
        flyBuf[o++] = cc[0];
        flyBuf[o++] = cc[1];
        flyBuf[o++] = cc[2];
      }
    };
    /* 白色 detached 袖：摆动幅度 ∝ 脑活动（接上神经元），漂浮时向后飘 */
    for (const s of [1, -1]) {
      rot(s * 0.2, 0.92, 0.02, w);
      const sway = Math.sin(t * 2.4 + s * 1.7) * (0.1 + 0.45 * activity) + airT * 0.3;
      const dL = [s * 0.26, -1, sway * 0.5 + airT * 0.3];
      const dl = Math.hypot(dL[0], dL[1], dL[2]);
      const dW = [0, 0, 0];
      rotN(dL[0] / dl, dL[1] / dl, dL[2] / dl, dW);
      writeCone([w[0], w[1], w[2]], dW, 0.32, 0.075, 0.105, (u) => (u > 0.8 ? C_RED : C_WHITE));
    }
    /* 小腿小鞋：散步交替摆步，坐/躺向前伸，漂浮垂落 */
    for (const s of [1, -1]) {
      rot(s * 0.095, 0.34, 0, w);
      const swing =
        gaitT * Math.sin(t * Math.PI * 2 * stepFreq + (s > 0 ? 0 : Math.PI)) * 0.55 +
        restT * 1.15 -
        airT * 0.28;
      const dW = [0, 0, 0];
      rotN(0, -Math.cos(swing), -Math.sin(swing), dW);
      writeCone([w[0], w[1], w[2]], dW, 0.27, 0.05, 0.042, (u) => (u > 0.78 ? C_SHOE : C_SKIN));
    }
    /* 蝴蝶结缎带尾：摆动幅度 ∝ 脑活动 */
    for (const s of [1, -1]) {
      const pv = [0, 0, 0];
      rot(s * 0.055, 1.35, 0.075, pv);
      const swayB = Math.sin(t * 3.1 + s * 2.3) * (0.15 + 0.7 * activity) + airT * 0.4;
      const dL = [s * 0.1, -1, 0.3 + swayB];
      const dl = Math.hypot(dL[0], dL[1], dL[2]);
      const dW = [0, 0, 0];
      rotN(dL[0] / dl, dL[1] / dl, dL[2] / dl, dW);
      const wv = [0, 0, 0];
      rotN(1, 0, 0, wv);
      const hw = 0.048;
      // 面片法线 = dir × width
      let nx = dW[1] * wv[2] - dW[2] * wv[1],
        ny = dW[2] * wv[0] - dW[0] * wv[2],
        nz = dW[0] * wv[1] - dW[1] * wv[0];
      const nl2 = Math.hypot(nx, ny, nz) || 1;
      nx /= nl2;
      ny /= nl2;
      nz /= nl2;
      const tip = [pv[0] + dW[0] * 0.34, pv[1] + dW[1] * 0.34, pv[2] + dW[2] * 0.34];
      const quadV = (px, py, pz, col) => {
        flyBuf[o++] = px;
        flyBuf[o++] = py;
        flyBuf[o++] = pz;
        flyBuf[o++] = nx;
        flyBuf[o++] = ny;
        flyBuf[o++] = nz;
        flyBuf[o++] = col[0];
        flyBuf[o++] = col[1];
        flyBuf[o++] = col[2];
      };
      const A = [pv[0] + wv[0] * hw, pv[1] + wv[1] * hw, pv[2] + wv[2] * hw],
        B = [pv[0] - wv[0] * hw, pv[1] - wv[1] * hw, pv[2] - wv[2] * hw],
        Cc = [tip[0] + wv[0] * hw * 0.7, tip[1] + wv[1] * hw * 0.7, tip[2] + wv[2] * hw * 0.7],
        D = [tip[0] - wv[0] * hw * 0.7, tip[1] - wv[1] * hw * 0.7, tip[2] - wv[2] * hw * 0.7];
      quadV(A[0], A[1], A[2], C_BOW);
      quadV(B[0], B[1], B[2], C_BOW);
      quadV(Cc[0], Cc[1], Cc[2], C_WHITE); // 尾端白边
      quadV(B[0], B[1], B[2], C_BOW);
      quadV(D[0], D[1], D[2], C_WHITE);
      quadV(Cc[0], Cc[1], Cc[2], C_WHITE);
    }
    /* 眼睛高光：亮度 ∝ 对应侧 vision（subtle 白点）；eating 双眼同闪 */
    let flash = 0;
    const ft = t - eatFlashT0;
    if (ft >= 0 && ft < 0.9) flash = (1 - ft / 0.9) * (0.65 + 0.35 * Math.sin(t * 26));
    const vL = Math.max(visionL, flash),
      vR = Math.max(visionR, flash);
    let eo = 0;
    const fillEye = (posArr, v) => {
      const k = Math.max(0, Math.min(1, 0.25 + 0.75 * v * (1 + 0.15 * Math.sin(t * 6))));
      for (let i = 0; i < posArr.length; i += 3) {
        rot(posArr[i], posArr[i + 1], posArr[i + 2], w);
        eyeBuf[eo++] = w[0];
        eyeBuf[eo++] = w[1];
        eyeBuf[eo++] = w[2];
        eyeBuf[eo++] = k;
        eyeBuf[eo++] = k * 0.98;
        eyeBuf[eo++] = k;
      }
    };
    fillEye(EYE_HI_L, vL);
    fillEye(EYE_HI_R, vR);
    eyeHiVerts = eo / 6;
    return o;
  }

  /* ================= 左上角「果蝇大脑」小窗（复活 f25008a6 点云渲染器，独立 GL 上下文） ================= */
  function createBrainView(cv) {
    // alpha:true + 非预乘 compositing：加法混合下 alpha 会随点亮累积（点中心≈1、间隙=0），
    // 用 premultipliedAlpha:false 让页面合成按非预乘处理，点色不被 alpha 二次压暗/截断
    const bgl = cv.getContext("webgl2", {
      antialias: true,
      alpha: true,
      premultipliedAlpha: false,
    });
    if (!bgl) return null;
    const SC_COLOR = {
      optic: "#1f77b4",
      central: "#ff7f0e",
      sensory: "#2ca02c",
      visual_projection: "#9467bd",
      visual_centrifugal: "#8c564b",
      ascending: "#e377c2",
      sensory_ascending: "#bcbd22",
      descending: "#d62728",
      motor: "#17becf",
      endocrine: "#7f7f7f",
    };
    const hexRgb = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
    const compile = (type, src) => {
      const s = bgl.createShader(type);
      bgl.shaderSource(s, src);
      bgl.compileShader(s);
      if (!bgl.getShaderParameter(s, bgl.COMPILE_STATUS)) throw new Error(bgl.getShaderInfoLog(s));
      return s;
    };
    const prog = bgl.createProgram();
    bgl.attachShader(
      prog,
      compile(
        bgl.VERTEX_SHADER,
        `#version 300 es
        in vec3 aPos; in vec3 aColor; in float aSpike;
        uniform mat4 uMVP; uniform float uNow; uniform float uDim; uniform float uSize;
        out vec3 vCol;
        void main() {
          vec4 p = uMVP * vec4(aPos, 1.0);
          gl_Position = p;
          float w = max(p.w, 0.05);
          float act = exp(-(uNow - aSpike) / 500.0);   /* 500ms 拖尾 */
          gl_PointSize = clamp(uSize / w * (1.0 + 1.5 * act), 1.0, 7.0);
          vec3 hot = mix(vec3(1.0, 0.4, 0.12), vec3(1.0, 1.0, 0.92), act);
          vCol = aColor * uDim + hot * act * 1.8;
        }`,
      ),
    );
    bgl.attachShader(
      prog,
      compile(
        bgl.FRAGMENT_SHADER,
        `#version 300 es
        precision mediump float;
        in vec3 vCol; out vec4 o;
        void main() {
          vec2 d = gl_PointCoord - 0.5;
          float r2 = dot(d, d);
          if (r2 > 0.25) discard;
          o = vec4(vCol, smoothstep(0.25, 0.05, r2));
        }`,
      ),
    );
    bgl.linkProgram(prog);
    const uMVP = bgl.getUniformLocation(prog, "uMVP"),
      uNow = bgl.getUniformLocation(prog, "uNow"),
      uDim = bgl.getUniformLocation(prog, "uDim"),
      uSize = bgl.getUniformLocation(prog, "uSize");
    let N = 0,
      vao = null,
      spikeTimes = null,
      spikeBuf = null,
      ready = false;
    // 显示模式（原版两档）：活动=uDim 0（静默点不可见，只看被激活神经元，默认）；解剖=uDim 1.1（全脑分类色）
    let modeDim = 0.0;
    let yaw = 0.8,
      pitch = 0.35,
      dist = 1.7;
    let lastDrag = 0,
      dragging = false,
      dragX = 0,
      dragY = 0;
    /* 轨道交互对齐原版网页：左键拖拽旋转、滚轮缩放、空闲 3 秒恢复自转 */
    on(cv, "pointerdown", (e) => {
      e.stopPropagation();
      e.preventDefault();
      dragging = true;
      dragX = e.clientX;
      dragY = e.clientY;
      cv.setPointerCapture(e.pointerId);
      lastDrag = performance.now();
    });
    on(cv, "pointermove", (e) => {
      if (!dragging) return;
      e.stopPropagation();
      const dx = e.clientX - dragX,
        dy = e.clientY - dragY;
      dragX = e.clientX;
      dragY = e.clientY;
      yaw += dx * 0.006;
      pitch = Math.max(-1.5, Math.min(1.5, pitch + dy * 0.006));
      lastDrag = performance.now();
    });
    on(cv, "pointerup", () => {
      dragging = false;
    });
    on(cv, "pointercancel", () => {
      dragging = false;
    });
    on(
      cv,
      "wheel",
      (e) => {
        e.preventDefault();
        e.stopPropagation();
        dist = Math.max(0.4, Math.min(5, dist * Math.exp(e.deltaY * 0.001)));
        lastDrag = performance.now();
      },
      { passive: false },
    );
    async function init() {
      const res = await invoke("fly_brain_positions");
      const data = base64ToF32(res.data_b64);
      const classes = res.classes;
      N = data.length / 4;
      const pos = new Float32Array(N * 3),
        col = new Float32Array(N * 3);
      spikeTimes = new Float32Array(N).fill(-1e9);
      const ctab = classes.map((c) => hexRgb(SC_COLOR[c] || "#888888"));
      for (let i = 0; i < N; i++) {
        pos[i * 3] = data[i * 4];
        pos[i * 3 + 1] = data[i * 4 + 1];
        pos[i * 3 + 2] = data[i * 4 + 2];
        const c = ctab[data[i * 4 + 3] | 0] || [0.5, 0.5, 0.5];
        col[i * 3] = c[0];
        col[i * 3 + 1] = c[1];
        col[i * 3 + 2] = c[2];
      }
      vao = bgl.createVertexArray();
      bgl.bindVertexArray(vao);
      const attr = (name, arr, size) => {
        const b = bgl.createBuffer();
        bgl.bindBuffer(bgl.ARRAY_BUFFER, b);
        bgl.bufferData(bgl.ARRAY_BUFFER, arr, bgl.STATIC_DRAW);
        const loc = bgl.getAttribLocation(prog, name);
        bgl.enableVertexAttribArray(loc);
        bgl.vertexAttribPointer(loc, size, bgl.FLOAT, false, 0, 0);
      };
      attr("aPos", pos, 3);
      attr("aColor", col, 3);
      spikeBuf = bgl.createBuffer();
      bgl.bindBuffer(bgl.ARRAY_BUFFER, spikeBuf);
      bgl.bufferData(bgl.ARRAY_BUFFER, spikeTimes, bgl.DYNAMIC_DRAW);
      const loc = bgl.getAttribLocation(prog, "aSpike");
      bgl.enableVertexAttribArray(loc);
      bgl.vertexAttribPointer(loc, 1, bgl.FLOAT, false, 0, 0);
      bgl.bindVertexArray(null);
      bgl.enable(bgl.BLEND);
      bgl.blendFunc(bgl.SRC_ALPHA, bgl.ONE);
      bgl.clearColor(0, 0, 0, 0); // 透明底，透出毛玻璃面板
      bgl.viewport(0, 0, cv.width, cv.height);
      ready = true;
    }
    function uploadSpikes(spikes, ages, now) {
      if (!ready || !spikes) return;
      for (let i = 0; i < spikes.length; i++) spikeTimes[spikes[i]] = now - (ages[i] || 0);
      bgl.bindBuffer(bgl.ARRAY_BUFFER, spikeBuf);
      bgl.bufferSubData(bgl.ARRAY_BUFFER, 0, spikeTimes);
    }
    function render(now, dt) {
      if (!ready || destroyed) return;
      if (now - lastDrag > 3000) yaw += dt * 0.06; // 空闲 3 秒缓慢自转（对齐原版）
      const asp = cv.width / cv.height;
      const m = matMul(trans(0, 0, -dist), matMul(rotX(pitch), rotY(yaw)));
      const mvp = matMul(persp(0.9, asp, 0.01, 10), m);
      bgl.clear(bgl.COLOR_BUFFER_BIT);
      bgl.useProgram(prog);
      bgl.uniformMatrix4fv(uMVP, false, mvp);
      bgl.uniform1f(uNow, now);
      bgl.uniform1f(uDim, modeDim);
      bgl.uniform1f(uSize, 3.0);
      bgl.bindVertexArray(vao);
      bgl.drawArrays(bgl.POINTS, 0, N);
      bgl.bindVertexArray(null);
    }
    function lose() {
      bgl.getExtension("WEBGL_lose_context")?.loseContext();
    }
    function toggleMode() {
      modeDim = modeDim === 0 ? 1.1 : 0.0;
      return modeDim === 0 ? "活动" : "解剖";
    }
    return {
      init,
      uploadSpikes,
      render,
      lose,
      toggleMode,
      get ready() {
        return ready;
      },
    };
  }
  let brainOn = false,
    brainInit = null;
  function setBrain(open) {
    brainOn = open;
    $("#brainPanel").hidden = !brainOn;
    $("#btnBrain").textContent = brainOn ? "大脑 开" : "大脑 关";
    $("#btnBrain").classList.toggle("active", brainOn);
    if (brainOn && !brain && !brainInit) {
      brain = createBrainView($("#brainCv"));
      if (!brain) {
        $("#brainHint").textContent = "⚠️ 此环境不支持 WebGL2";
        return;
      }
      $("#brainHint").textContent = "载入脑模型…";
      brainInit = brain
        .init()
        .then(() => {
          brainInit = null;
          if (!destroyed) $("#brainHint").textContent = brain?.ready ? "" : "载入失败";
        })
        .catch(() => {
          brainInit = null;
          brain = null;
          if (!destroyed) $("#brainHint").textContent = "载入失败，重新开关重试";
        });
    } else if (brainOn && brain?.ready) {
      $("#brainHint").textContent = "";
    }
  }

  /* ================= 相机（环绕 + 松弛跟随果蝇） ================= */
  let yaw = 0.7,
    pitch = 0.38,
    dist = 15;
  const camTarget = [0, heightAt(0, 0) + 1, 0];
  let lastInteract = 0;
  on(canvas, "contextmenu", (e) => e.preventDefault());
  let dragBtn = -1,
    dragX = 0,
    dragY = 0;
  on(canvas, "pointerdown", (e) => {
    dragBtn = e.button;
    dragX = e.clientX;
    dragY = e.clientY;
    canvas.setPointerCapture(e.pointerId);
    lastInteract = performance.now();
  });
  on(canvas, "pointermove", (e) => {
    if (dragBtn < 0) return;
    const dx = e.clientX - dragX,
      dy = e.clientY - dragY;
    dragX = e.clientX;
    dragY = e.clientY;
    yaw -= dx * 0.005;
    pitch = Math.max(0.06, Math.min(1.3, pitch + dy * 0.005));
    lastInteract = performance.now();
  });
  on(canvas, "pointerup", () => {
    dragBtn = -1;
  });
  on(canvas, "pointercancel", () => {
    dragBtn = -1;
  });
  on(
    canvas,
    "wheel",
    (e) => {
      e.preventDefault();
      dist = Math.max(5, Math.min(45, dist * Math.exp(e.deltaY * 0.001)));
      lastInteract = performance.now();
    },
    { passive: false },
  );

  /* ================= 仿真状态（200ms 轮询） ================= */
  const STATE_TEXT = {
    flying: "飞舞中",
    foraging: "觅食中",
    eating: "进食中",
    resting: "休息中",
    sleeping: "睡觉中",
    walking: "散步中",
  };
  const KNOWN_STATES = new Set(Object.keys(STATE_TEXT));
  let sim = null,
    firstState = false;
  let tod = 0.5;
  const flyPos = { x: 0, y: heightAt(0, 0) + 1.2, z: 0 },
    flyTarget = { x: 0, z: 0 };
  let flyHeading = 0,
    flyRot = 0,
    flyState = "flying",
    flyBob = Math.random() * 10;
  let visionL = 0,
    visionR = 0,
    visionLT = 0,
    visionRT = 0,
    eatFlashT0 = -10;
  let gaitT = 0, // tripod 步态混合权重（0=站立静止 1=行走）
    flySpeed = 0,
    flySpeedT = 0;
  let hungerV = 0,
    energyV = 1,
    activity = 0,
    activityTarget = 0,
    speedV = 1,
    plasticOn = false;
  let lastTick = -1,
    lastSeq = 0;
  const toastStack = $("#toastStack");

  function pushToast(text) {
    if (!text) return;
    const el = document.createElement("div");
    el.className = "toast";
    el.textContent = text;
    toastStack.appendChild(el);
    while (toastStack.children.length > 4) toastStack.firstChild.remove();
    on(el, "animationend", () => el.remove());
  }

  function syncHud(st) {
    const clockH = Math.floor(tod * 24) % 24,
      clockM = Math.floor(((tod * 24) % 1) * 60);
    $("#todClock").textContent =
      `${String(clockH).padStart(2, "0")}:${String(clockM).padStart(2, "0")}`;
    $("#todIcon").textContent =
      tod >= 0.22 && tod < 0.3
        ? "🌅"
        : tod >= 0.3 && tod < 0.7
          ? "☀️"
          : tod >= 0.7 && tod < 0.78
            ? "🌇"
            : "🌙";
    $("#flyState").textContent = STATE_TEXT[flyState] || "飞舞中";
    $("#barHunger").style.width = `${Math.round(Math.max(0, Math.min(100, hungerV)))}%`;
    $("#barEnergy").style.width = `${Math.round(Math.max(0, Math.min(100, energyV)))}%`;
    $("#btnSpeed").textContent = `流速 ${st.speed ?? speedV}x`;
    $("#btnPlastic").textContent = (st.plasticity ?? plasticOn) ? "学习 开" : "学习 关";
    // 「复眼」指示：L/R 小圆点随 vision 放电呼吸（旧快照无该字段时保持暗态）
    const vl = Math.max(0, Math.min(1, st.vision?.left ?? 0)),
      vr = Math.max(0, Math.min(1, st.vision?.right ?? 0));
    const eyeL = $("#eyeL"),
      eyeR = $("#eyeR");
    eyeL.style.opacity = String(0.22 + 0.78 * vl);
    eyeL.style.boxShadow = `0 0 8px 2px rgba(255,95,122,${0.65 * vl})`;
    eyeR.style.opacity = String(0.22 + 0.78 * vr);
    eyeR.style.boxShadow = `0 0 8px 2px rgba(255,95,122,${0.65 * vr})`;
    // 「活跃」近 600ms 发放过的去重神经元数（旧快照无此字段显示 —）
    $("#activeNeurons").textContent =
      typeof st.active_neurons === "number" ? st.active_neurons.toLocaleString("zh-CN") : "—";
  }

  async function poll() {
    if (destroyed) return;
    let st;
    try {
      st = await invoke("fly_brain_state");
    } catch {
      return; // 后端未就绪时跳过该帧
    }
    if (destroyed || !st) return;
    if (!firstState) {
      firstState = true;
      veil.classList.add("hide");
      flyPos.x = st.fly?.x ?? 0;
      flyPos.z = st.fly?.z ?? 0;
    }
    sim = st;
    tod = typeof st.time_of_day === "number" ? st.time_of_day : tod;
    if (st.fly) {
      flyTarget.x = st.fly.x ?? flyTarget.x;
      flyTarget.z = st.fly.z ?? flyTarget.z;
      if (typeof st.fly.heading === "number") flyHeading = st.fly.heading;
      const nextRaw = st.fly.state || "flying";
      const nextState = KNOWN_STATES.has(nextRaw) ? nextRaw : "flying"; // 不认识的 state 按 flying 处理
      if (nextState === "eating" && flyState !== "eating") eatFlashT0 = performance.now() / 1000; // 吃到瞬间双眼同闪
      flyState = nextState;
      if (typeof st.fly.speed === "number") flySpeedT = st.fly.speed;
      hungerV = st.fly.hunger ?? hungerV;
      energyV = st.fly.energy ?? energyV;
    }
    speedV = st.speed ?? speedV;
    plasticOn = !!st.plasticity;
    activityTarget = Math.max(0, Math.min(1, st.brain_activity ?? 0));
    if (st.vision) {
      // 视觉感觉群放电归一值（0..1），驱动复眼发光
      visionLT = Math.max(0, Math.min(1, st.vision.left ?? 0));
      visionRT = Math.max(0, Math.min(1, st.vision.right ?? 0));
    }
    if (brainOn && brain?.ready && st.spikes) {
      brain.uploadSpikes(st.spikes, st.spike_ages_ms || [], performance.now());
    }
    if (typeof st.tick === "number") {
      if (st.tick < lastTick) lastSeq = 0; // 重新开始后事件序号归零
      lastTick = st.tick;
    }
    const sig = (st.foods || []).map((f) => `${f.id}:${f.kind}`).join(",");
    if (sig !== foodsSig) {
      foodsSig = sig;
      rebuildFoods(st.foods);
    }
    for (const ev of st.events || []) {
      if (ev.seq > lastSeq) {
        pushToast(ev.text);
        lastSeq = ev.seq;
      }
    }
    syncHud(st);
  }

  /* ================= 控件 ================= */
  const control = (payload) =>
    invoke("fly_brain_control", { payload })
      .then((r) => {
        if (destroyed || !r) return;
        if (typeof r.speed === "number") {
          speedV = r.speed;
          $("#btnSpeed").textContent = `流速 ${speedV}x`;
        }
        if (typeof r.plasticity === "boolean") {
          plasticOn = r.plasticity;
          $("#btnPlastic").textContent = plasticOn ? "学习 开" : "学习 关";
        }
      })
      .catch(() => {});
  on($("#btnSpeed"), "click", () => {
    const next = speedV >= 2 ? 0.5 : speedV >= 1 ? 2 : 1;
    control({ speed: next });
  });
  on($("#btnPlastic"), "click", () => control({ plasticity: !plasticOn }));
  on($("#btnBrain"), "click", () => setBrain(!brainOn));
  on($("#brainMode"), "click", (e) => {
    e.stopPropagation(); // 别触发小窗 canvas 的拖拽
    if (brain) $("#brainMode").textContent = brain.toggleMode();
  });
  /* 双地图切换：神社（默认）/ 牧场，选择存 localStorage */
  function setMap(kind) {
    mapKind = kind;
    try {
      localStorage.setItem("flyBrainMap", kind);
    } catch (_) {}
    $("#btnMap").textContent = `地图 ${kind === "shrine" ? "神社" : "牧场"}`;
    decor = decorSets[kind];
  }
  on($("#btnMap"), "click", () => setMap(mapKind === "shrine" ? "pasture" : "shrine"));
  setMap(mapKind);
  on($("#btnRestart"), "click", () => control({ cmd: "restart" }));
  on($("#btnExit"), "click", () => {
    destroy();
    options.onExit();
  });

  /* ================= 天色板（青绿山水：白天石青白雾、黄昏暖金、夜晚水墨） ================= */
  const DAY_ZEN = [0.45, 0.68, 0.88],
    DAY_HOR = [0.9, 0.95, 0.94],
    DUSK_ZEN = [0.55, 0.45, 0.62],
    DUSK_HOR = [1.0, 0.72, 0.42],
    NIGHT_ZEN = [0.03, 0.06, 0.13],
    NIGHT_HOR = [0.1, 0.15, 0.24];
  const lerp = (a, b, t) => a + (b - a) * t;
  const lerp3 = (a, b, t) => [lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t)];
  const clamp01 = (v) => Math.max(0, Math.min(1, v));
  const smooth = (a, b, v) => {
    const t = clamp01((v - a) / (b - a));
    return t * t * (3 - 2 * t);
  };
  function palette(e) {
    const day = smooth(0.06, 0.32, e),
      night = smooth(0.03, 0.2, -e),
      dusk = clamp01(1 - Math.abs(e + 0.03) / 0.16);
    let zen = lerp3(DAY_ZEN, DUSK_ZEN, dusk * 0.6);
    zen = lerp3(zen, NIGHT_ZEN, night);
    let hor = lerp3(DAY_HOR, DUSK_HOR, dusk);
    hor = lerp3(hor, NIGHT_HOR, night);
    const sunVis = clamp01(day + dusk * 0.8) * smooth(-0.06, 0.01, e);
    const sunTint = lerp3([1, 0.96, 0.86], [1, 0.58, 0.38], dusk);
    const ambient = lerp3([0.17, 0.21, 0.32], [0.6, 0.64, 0.56], clamp01(day + dusk * 0.3));
    const sunColor = sunTint.map((v) => v * (day * 0.9 + dusk * 0.45));
    return { zen, hor, dusk, night, sunVis, sunTint, ambient, sunColor };
  }

  /* ================= 渲染主循环 ================= */
  const FOG_K = 0.000055;
  let lastT = 0;
  function resizeIfNeeded() {
    const dpr = Math.min(window.devicePixelRatio || 1, 1.75);
    const w = Math.max(1, Math.round(canvas.clientWidth * dpr)),
      h = Math.max(1, Math.round(canvas.clientHeight * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
      gl.viewport(0, 0, w, h);
    }
  }
  const angleLerp = (a, b, t) => {
    let d = (b - a) % (Math.PI * 2);
    if (d > Math.PI) d -= Math.PI * 2;
    if (d < -Math.PI) d += Math.PI * 2;
    return a + d * t;
  };

  function frame(now) {
    if (destroyed) return;
    frameId = requestAnimationFrame(frame);
    const dt = Math.min(0.05, lastT ? (now - lastT) / 1000 : 0.016);
    lastT = now;
    const t = now / 1000;
    resizeIfNeeded();

    /* 果蝇位置平滑（200ms 轮询间隙 rAF 补间） */
    const kMove = 1 - Math.exp(-dt * 6);
    const prevX = flyPos.x,
      prevZ = flyPos.z;
    flyPos.x += (flyTarget.x - flyPos.x) * kMove;
    flyPos.z += (flyTarget.z - flyPos.z) * kMove;
    const moved = Math.hypot(flyPos.x - prevX, flyPos.z - prevZ);
    if (moved / Math.max(dt, 1e-4) > 0.4) {
      // 运动方向优先（对后端 heading 轴向约定鲁棒）
      flyRot = angleLerp(flyRot, Math.atan2(flyPos.z - prevZ, flyPos.x - prevX), kMove);
    } else {
      flyRot = angleLerp(flyRot, flyHeading, 1 - Math.exp(-dt * 3));
    }
    const airborne = flyState === "flying" || flyState === "foraging";
    flyBob += dt * (airborne ? 7 : 0);
    const groundY = heightAt(flyPos.x, flyPos.z);
    // 灵梦：漂浮=离地 1.05 上下浮动；睡觉=躺下（贴地半高）；其余贴地站立
    const targetY = airborne
      ? groundY + 1.05 + Math.sin(flyBob) * 0.16
      : flyState === "sleeping"
        ? groundY + 0.24
        : groundY + 0.02;
    flyPos.y += (targetY - flyPos.y) * (1 - Math.exp(-dt * 3.5));
    activity += (activityTarget - activity) * (1 - Math.exp(-dt * 4));
    visionL += (visionLT - visionL) * (1 - Math.exp(-dt * 7));
    visionR += (visionRT - visionR) * (1 - Math.exp(-dt * 7));
    flySpeed += (flySpeedT - flySpeed) * (1 - Math.exp(-dt * 6));
    gaitT += ((flyState === "walking" ? 1 : 0) - gaitT) * (1 - Math.exp(-dt * 4));
    const flyFloats = fillReimu(t, dt, airborne ? 0 : 1);

    /* 相机 */
    if (now - lastInteract > 4000) yaw += dt * 0.045; // 空闲缓慢环绕
    const kCam = 1 - Math.exp(-dt * 1.6);
    camTarget[0] += (flyPos.x - camTarget[0]) * kCam;
    camTarget[2] += (flyPos.z - camTarget[2]) * kCam;
    camTarget[1] += (groundY + 1.1 - camTarget[1]) * kCam;
    const cp = Math.cos(pitch),
      sp = Math.sin(pitch);
    const eye = [
      camTarget[0] + Math.sin(yaw) * cp * dist,
      camTarget[1] + sp * dist,
      camTarget[2] + Math.cos(yaw) * cp * dist,
    ];
    let minEyeY = heightAt(eye[0], eye[2]) + 0.8;
    if (Math.hypot(eye[0] - POND.x, eye[2] - POND.z) < WATER_R + 1)
      minEyeY = Math.max(minEyeY, WATER_Y + 0.5);
    if (eye[1] < minEyeY) eye[1] = minEyeY;
    const view = lookAt(eye, camTarget);
    const proj = persp(0.95, canvas.width / canvas.height, 0.1, SKY_R * 2.4);
    const vp = matMul(proj, view);
    const viewRot = new Float32Array(view); // 天空/星星：去掉平移，穹顶以相机为中心
    viewRot[12] = viewRot[13] = viewRot[14] = 0;
    const vpSky = matMul(proj, viewRot);
    const right = [view[0], view[4], view[8]],
      up = [view[1], view[5], view[9]];

    /* 天色 */
    const e =
      sim && typeof sim.sun_elevation === "number"
        ? sim.sun_elevation
        : Math.sin((tod - 0.25) * Math.PI * 2);
    const pal = palette(e);
    const sunTh = (tod - 0.25) * Math.PI * 2;
    const sunDir = [Math.cos(sunTh), Math.sin(sunTh), 0.35];
    const sl = Math.hypot(...sunDir);
    sunDir[0] /= sl;
    sunDir[1] /= sl;
    sunDir[2] /= sl;
    const moonDir = [-sunDir[0], -sunDir[1], sunDir[2]];

    gl.clearColor(pal.hor[0], pal.hor[1], pal.hor[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.disable(gl.BLEND);
    gl.depthMask(false);
    gl.disable(gl.DEPTH_TEST);

    /* 天空 + 星星 */
    gl.useProgram(skyProg);
    gl.uniformMatrix4fv(skyU.uVP, false, vpSky);
    gl.uniform3fv(skyU.uZenith, pal.zen);
    gl.uniform3fv(skyU.uHorizon, pal.hor);
    gl.uniform3fv(skyU.uDuskCol, DUSK_HOR);
    gl.uniform3fv(skyU.uSunDir, sunDir);
    gl.uniform3fv(skyU.uSunTint, pal.sunTint);
    gl.uniform3fv(skyU.uMoonDir, moonDir);
    gl.uniform1f(skyU.uDuskAmt, pal.dusk * (1 - pal.night * 0.85));
    gl.uniform1f(skyU.uSunVis, pal.sunVis);
    gl.uniform1f(skyU.uNight, pal.night);
    gl.bindVertexArray(skyVao.vao);
    gl.drawElements(gl.TRIANGLES, skyVao.count, gl.UNSIGNED_SHORT, 0);

    gl.useProgram(starProg);
    gl.uniformMatrix4fv(starU.uVP, false, vpSky);
    gl.uniform1f(starU.uNight, pal.night);
    gl.uniform1f(starU.uTime, t);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.bindVertexArray(starVao);
    gl.drawArrays(gl.POINTS, 0, STAR_N);
    gl.disable(gl.BLEND);

    gl.enable(gl.DEPTH_TEST);
    gl.depthMask(true);

    /* 光照系 uniforms（远山 / 地形 / 果蝇身体共用 litProg） */
    gl.useProgram(litProg);
    gl.uniformMatrix4fv(litU.uVP, false, vp);
    gl.uniform3fv(litU.uSunDir, sunDir);
    gl.uniform3fv(litU.uSunColor, pal.sunColor);
    gl.uniform3fv(litU.uAmbient, pal.ambient);
    gl.uniform3fv(litU.uFogColor, pal.hor);
    gl.uniform1f(litU.uFogK, FOG_K);
    gl.bindVertexArray(mountainVao.vao);
    gl.drawElements(gl.TRIANGLES, mountainVao.count, gl.UNSIGNED_SHORT, 0);
    gl.bindVertexArray(terrainVao.vao);
    gl.drawElements(gl.TRIANGLES, terrainVao.count, gl.UNSIGNED_SHORT, 0);
    /* 博丽神社（本殿/鸟居/参道/石灯笼，静态单次 draw，仅神社地图） */
    if (mapKind === "shrine") {
      gl.bindVertexArray(shrineGeo.vao);
      gl.drawArrays(gl.TRIANGLES, 0, shrineGeo.count);
    }
    /* 灵梦身体（刚体汤 + 袖/腿/缎带，CPU 逐帧变换后整体上传，一次 draw） */
    gl.bindBuffer(gl.ARRAY_BUFFER, flyVbo);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, flyBuf, 0, flyFloats);
    gl.bindVertexArray(flyVao);
    gl.drawArrays(gl.TRIANGLES, 0, flyFloats / 9);
    /* 眼睛高光：emissive（随 vision 调制的白色小亮点，不走 N·L），一次小 draw */
    gl.useProgram(eyeProg);
    gl.uniformMatrix4fv(eyeU.uVP, false, vp);
    gl.uniform3fv(eyeU.uFogColor, pal.hor);
    gl.uniform1f(eyeU.uFogK, FOG_K);
    gl.bindBuffer(gl.ARRAY_BUFFER, eyeVbo);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, eyeBuf);
    gl.bindVertexArray(eyeVao);
    gl.drawArrays(gl.TRIANGLES, 0, eyeHiVerts);

    /* 草叶 */
    gl.useProgram(grassProg);
    gl.uniformMatrix4fv(grassU.uVP, false, vp);
    gl.uniform1f(grassU.uTime, t);
    gl.uniform3fv(grassU.uGrassLow, [0.2, 0.44, 0.24]);
    gl.uniform3fv(grassU.uGrassHigh, [0.5, 0.74, 0.36]);
    gl.uniform3fv(grassU.uSunColor, pal.sunColor);
    gl.uniform3fv(grassU.uAmbient, pal.ambient);
    gl.uniform3fv(grassU.uFogColor, pal.hor);
    gl.uniform1f(grassU.uFogK, FOG_K);
    gl.uniform1f(grassU.uSunUp, sunDir[1]);
    gl.bindVertexArray(mapKind === "shrine" ? grassVaoShrine : grassVaoPasture);
    gl.drawArraysInstanced(gl.TRIANGLES, 0, 3, GRASS_N);

    /* 透明通道：水面 → 广告牌精灵 → 膜翅 → 萤火虫 */
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.depthMask(false);

    gl.useProgram(waterProg);
    gl.uniformMatrix4fv(waterU.uVP, false, vp);
    gl.uniform1f(waterU.uTime, t);
    gl.uniform3fv(waterU.uDeep, [0.14, 0.3, 0.33]);
    gl.uniform3fv(waterU.uSky, pal.hor);
    gl.uniform3fv(waterU.uSunTint, pal.sunTint);
    gl.uniform3fv(waterU.uEye, eye);
    gl.uniform3fv(waterU.uFogColor, pal.hor);
    gl.uniform1f(waterU.uSunVis, pal.sunVis);
    gl.uniform1f(waterU.uFogK, FOG_K);
    gl.bindVertexArray(waterVao.vao);
    gl.drawElements(gl.TRIANGLES, waterVao.count, gl.UNSIGNED_SHORT, 0);

    const sprites = [];
    for (const d of decor) sprites.push({ ...d, alpha: 1, rot: 0 });
    for (const m of mists) {
      m.ang += dt * m.speed;
      sprites.push({
        x: Math.cos(m.ang) * m.r,
        y: m.y,
        z: Math.sin(m.ang) * m.r,
        w: m.w,
        h: m.h,
        uv: UV.mist,
        phase: m.phase,
        sway: 0,
        alpha: 0.26 + 0.1 * Math.sin(t * 0.3 + m.phase),
        rot: 0,
      });
    }
    for (const f of foodSprites) sprites.push({ ...f, alpha: 1, rot: 0 });
    /* 石灯笼暖光晕：仅神社地图、仅夜晚点亮，轻微闪烁 */
    if (mapKind === "shrine")
      for (const g of lanternGlows)
        sprites.push({
          x: g.x,
          y: g.y,
          z: g.z,
          w: 1.15,
          h: 1.15,
          uv: UV.glow,
          phase: g.phase,
          sway: 0,
          alpha: pal.night * (0.5 + 0.18 * Math.sin(t * 2.6 + g.phase)),
          rot: 0,
        });
    const fwd = [-view[2], -view[6], -view[10]];
    for (const s of sprites)
      s.depth = (s.x - eye[0]) * fwd[0] + (s.y - eye[1]) * fwd[1] + (s.z - eye[2]) * fwd[2];
    sprites.sort((a, b) => b.depth - a.depth);
    const n = Math.min(sprites.length, MAX_SPRITES);
    for (let i = 0; i < n; i++) {
      const s = sprites[i],
        o = i * 13;
      spriteData[o] = s.x;
      spriteData[o + 1] = s.y;
      spriteData[o + 2] = s.z;
      spriteData[o + 3] = s.w;
      spriteData[o + 4] = s.h;
      spriteData[o + 5] = s.uv[0];
      spriteData[o + 6] = s.uv[1];
      spriteData[o + 7] = s.uv[2];
      spriteData[o + 8] = s.uv[3];
      spriteData[o + 9] = s.phase;
      spriteData[o + 10] = s.alpha;
      spriteData[o + 11] = s.rot;
      spriteData[o + 12] = s.sway;
    }
    gl.useProgram(spriteProg);
    gl.uniformMatrix4fv(spriteU.uVP, false, vp);
    gl.uniform3fv(spriteU.uRight, right);
    gl.uniform3fv(spriteU.uUp, up);
    gl.uniform1f(spriteU.uTime, t);
    gl.uniform3fv(spriteU.uFogColor, pal.hor);
    gl.uniform1f(spriteU.uFogK, FOG_K);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, atlasTex);
    gl.uniform1i(spriteU.uTex, 0);
    gl.bindBuffer(gl.ARRAY_BUFFER, spriteInstBuf);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, spriteData, 0, n * 13);
    gl.bindVertexArray(spriteVao);
    gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, n);

    gl.useProgram(fireflyProg);
    gl.uniformMatrix4fv(fireflyU.uVP, false, vp);
    gl.uniform1f(fireflyU.uTime, t);
    gl.uniform1f(fireflyU.uNight, pal.night);
    gl.uniform1f(fireflyU.uActivity, activity);
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.bindVertexArray(fireflyVao);
    gl.drawArrays(gl.POINTS, 0, FF_N);
    gl.disable(gl.BLEND);
    gl.depthMask(true);
    gl.bindVertexArray(null);

    /* 大脑小窗（独立 GL 上下文，共享主 rAF） */
    if (brainOn && brain) brain.render(now, dt);

    /* 脑活动呼吸点 */
    const dot = $("#brainDot");
    dot.style.transform = `scale(${0.65 + 0.55 * activity})`;
    dot.style.opacity = String(0.45 + 0.55 * activity);
  }

  frameId = requestAnimationFrame(frame);
  pollTimer = setInterval(poll, 200);
  poll();
  if (options.signal.aborted) {
    destroy();
  } else {
    on(options.signal, "abort", destroy);
  }
  return { destroy };
}

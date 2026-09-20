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
  const grassVao = (() => {
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
  })();

  /* ================= 程序化贴图集（荷花/桃子/竹/松/桃树/石头/雾带/荷叶） ================= */
  const ATLAS = 512;
  const atlasCanvas = document.createElement("canvas");
  atlasCanvas.width = ATLAS;
  atlasCanvas.height = ATLAS;
  const REG = {
    lotus: [0, 0, 128, 128],
    peach: [128, 0, 128, 128],
    bamboo: [256, 0, 128, 128],
    pine: [384, 0, 128, 128],
    peachTree: [0, 128, 128, 128],
    stone: [128, 128, 128, 128],
    mist: [0, 256, 256, 64],
    lotusLeaf: [256, 256, 64, 64],
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
    /* 荷花（蜜源 kind 0）：层叠粉瓣 + 莲蓬 */
    c.save();
    c.translate(0, 0);
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
    /* 桃子（蜜源 kind 1）：粉橙成串 */
    c.save();
    c.translate(128, 0);
    const peach = (x, y, r) => {
      const g = c.createRadialGradient(x - r * 0.3, y - r * 0.4, r * 0.15, x, y, r);
      g.addColorStop(0, "#ffd9b8");
      g.addColorStop(0.6, "#ffb08a");
      g.addColorStop(1, "#f07f68");
      c.fillStyle = g;
      c.beginPath();
      c.arc(x, y, r, 0, Math.PI * 2);
      c.fill();
      c.strokeStyle = "rgba(214,90,80,0.65)";
      c.lineWidth = 2;
      c.beginPath();
      c.moveTo(x, y - r * 0.9);
      c.quadraticCurveTo(x + r * 0.18, y, x, y + r * 0.9);
      c.stroke();
      c.fillStyle = "rgba(255,255,255,0.7)";
      c.beginPath();
      c.ellipse(x - r * 0.32, y - r * 0.4, r * 0.2, r * 0.12, -0.6, 0, Math.PI * 2);
      c.fill();
    };
    c.fillStyle = "#5da85f";
    c.beginPath();
    c.ellipse(60, 42, 18, 7, -0.4, 0, Math.PI * 2);
    c.fill();
    c.beginPath();
    c.ellipse(78, 50, 14, 6, 0.5, 0, Math.PI * 2);
    c.fill();
    peach(54, 76, 17);
    peach(80, 84, 15);
    peach(62, 100, 13);
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
    /* 桃树：褐干 + 粉色花团 */
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
    blossom(46, 62, 22, "rgba(255,178,204,A)");
    blossom(82, 58, 24, "rgba(255,194,214,A)");
    blossom(64, 44, 24, "rgba(255,214,228,A)");
    blossom(56, 76, 18, "rgba(255,158,196,A)");
    blossom(78, 80, 16, "rgba(255,178,204,A)");
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

  /* 装饰：竹丛 / 松树 / 桃树 / 石头 + 塘中荷叶荷花（静态） */
  const UV = {};
  for (const k of Object.keys(REG)) UV[k] = uvRect(...REG[k]);
  const decor = [];
  {
    const clearOfPond = (x, z, margin) => Math.hypot(x - POND.x, z - POND.z) > POND.r + margin;
    const scatter = (count, rMin, rMax) => {
      const out = [];
      let guard = 0;
      while (out.length < count && guard++ < count * 30) {
        const a = Math.random() * Math.PI * 2,
          r = rMin + Math.random() * (rMax - rMin);
        const x = Math.cos(a) * r,
          z = Math.sin(a) * r;
        if (!clearOfPond(x, z, 3)) continue;
        out.push([x, z]);
      }
      return out;
    };
    for (const [i, [x, z]] of scatter(4, 12, 40).entries()) {
      const h = 4.2 + Math.random();
      decor.push({
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
    for (const [i, [x, z]] of scatter(3, 14, 42).entries()) {
      const h = 4.0 + Math.random() * 0.8;
      decor.push({
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
    for (const [i, [x, z]] of scatter(3, 10, 38).entries()) {
      const h = 3.6 + Math.random() * 0.6;
      decor.push({
        x,
        y: heightAt(x, z) + h * 0.46,
        z,
        w: 3.6,
        h,
        uv: UV.peachTree,
        phase: i * 3.1,
        sway: 0.05,
      });
    }
    for (const [i, [x, z]] of scatter(3, 8, 40).entries()) {
      const s = 1.0 + Math.random() * 0.9;
      decor.push({
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
    // 塘面荷叶与一朵荷花
    for (let i = 0; i < 5; i++) {
      const a = hash2(i, 31) * Math.PI * 2,
        r = 1.5 + hash2(i, 37) * (WATER_R - 3);
      const x = POND.x + Math.cos(a) * r,
        z = POND.z + Math.sin(a) * r;
      decor.push({
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
    decor.push({
      x: POND.x + 2.2,
      y: WATER_Y + 0.42,
      z: POND.z - 1.4,
      w: 0.85,
      h: 0.95,
      uv: UV.lotus,
      phase: 4.4,
      sway: 0.03,
    });
  }
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

  /* 蜜源（后端 foods 驱动，吃掉消失/重生）：荷花与桃子 */
  let foodSprites = [],
    foodsSig = "";
  function rebuildFoods(foods) {
    foodSprites = (foods || []).map((f) => {
      const isFruit = f.kind === 1;
      const w = isFruit ? 1.55 : 1.3,
        h = isFruit ? 1.55 : 1.45;
      return {
        x: f.x,
        y: heightAt(f.x, f.z) + h * 0.44,
        z: f.z,
        w,
        h,
        uv: isFruit ? UV.peach : UV.lotus,
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

  /* ================= 3D 果蝇（程序化建模：头胸腹 + 复眼 + 膜翅 + 六条腿） ================= */
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
  /* 单位细圆柱（半径 1，y∈[0,1]，侧面），腿节共用 */
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
      // (x,y,z, nx,0,nz)
      const a = [c0, 0, s0, c0, 0, s0],
        b = [c0, 1, s0, c0, 0, s0],
        d = [c1, 0, s1, c1, 0, s1],
        e = [c1, 1, s1, c1, 0, s1];
      out.push(...a, ...b, ...d, ...d, ...b, ...e);
    }
    return out;
  })();
  const BODY_DARK = [0.3, 0.23, 0.16];
  const RIGID_SOUP = (() => {
    const abdomenBand = (p) => {
      const z = p[2] - 0.03;
      return Math.floor(z / 0.1) % 2 === 0 ? [0.38, 0.3, 0.21] : [0.24, 0.18, 0.13];
    };
    return new Float32Array([
      ...sphereSoup(0, 0, 0, 0.2, 0.17, 0.22, 10, 7, () => BODY_DARK), // 胸
      ...sphereSoup(0, -0.03, 0.33, 0.21, 0.16, 0.3, 10, 7, abdomenBand), // 腹（环纹）
      ...sphereSoup(0, 0.03, -0.27, 0.12, 0.11, 0.11, 9, 6, () => [0.27, 0.2, 0.14]), // 头
    ]);
  })();
  /* 复眼：独立几何（只存位置），颜色逐帧随 vision 放电调制，emissive 不走光照 */
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
  const EYE_L_POS = spherePos(0.08, 0.07, -0.31, 0.055, 7, 5),
    EYE_R_POS = spherePos(-0.08, 0.07, -0.31, 0.055, 7, 5);
  const EYE_VERTS = (EYE_L_POS.length + EYE_R_POS.length) / 3;
  const EYE_BASE = [0.38, 0.08, 0.07], // 平时暗红
    EYE_HOT = [1.0, 0.42, 0.56]; // 看到食物一侧的粉红亮光
  const eyeBuf = new Float32Array(EYE_VERTS * 6);
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
  function eyeColorAt(v, t, phase) {
    const pulse = 1 + 0.2 * v * Math.sin(t * 8 + phase); // 轻微脉动，幅度随视觉放电
    const k = Math.max(0, Math.min(1, v * pulse));
    return [
      EYE_BASE[0] + (EYE_HOT[0] - EYE_BASE[0]) * k,
      EYE_BASE[1] + (EYE_HOT[1] - EYE_BASE[1]) * k,
      EYE_BASE[2] + (EYE_HOT[2] - EYE_BASE[2]) * k,
    ];
  }
  /* 六条腿：胸节两侧各 3 条（前/中/后），站立与蜷飞两套姿态 */
  const LEGS = [];
  for (const s of [1, -1])
    for (let i = 0; i < 3; i++) {
      LEGS.push({
        s,
        i,
        phase: (s > 0 ? i : i + 3) * 1.31,
        a: [s * 0.11, -0.06, -0.13 + i * 0.13],
        standK: [s * 0.3, -0.16, -0.13 + i * 0.13 + (i - 1) * 0.02],
        standF: [s * 0.38, -0.3, -0.17 + i * 0.15 + (i === 0 ? -0.05 : i === 2 ? 0.07 : 0)],
        curlK: [s * 0.16, -0.14, -0.08 + i * 0.1],
        curlF: [s * 0.18, -0.26, -0.02 + i * 0.08],
      });
    }
  const LEG_COLORS = { femur: [0.22, 0.17, 0.12], tibia: [0.16, 0.12, 0.09] };
  /* 膜翅：root 相对的 12 个顶点（两侧各一个四边形） */
  const WING_ROOT = [0.09, 0.12, 0.03];
  const WING_VERTS = [];
  for (const s of [1, -1]) {
    const q = [
      [0, 0, -0.08],
      [s * 0.55, 0.03, -0.02],
      [s * 0.45, 0.03, 0.3],
      [0, 0, -0.08],
      [s * 0.45, 0.03, 0.3],
      [0, 0, 0.12],
    ];
    for (const v of q) WING_VERTS.push({ s, v });
  }
  const FLY_MAX_FLOATS = 40000;
  const flyBuf = new Float32Array(FLY_MAX_FLOATS);
  const flyVao = gl.createVertexArray();
  const flyVbo = gl.createBuffer();
  {
    gl.bindVertexArray(flyVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, flyVbo);
    gl.bufferData(gl.ARRAY_BUFFER, FLY_MAX_FLOATS * 4, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 36, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 36, 12);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 3, gl.FLOAT, false, 36, 24);
    gl.bindVertexArray(null);
  }
  const wingProg = makeProg(
    `#version 300 es
    layout(location=0) in vec3 aPos;
    uniform mat4 uVP;
    out float vFog;
    void main() {
      vec4 cp = uVP * vec4(aPos, 1.0);
      vFog = cp.w;
      gl_Position = cp;
    }`,
    `#version 300 es
    precision mediump float;
    in float vFog;
    uniform vec4 uColor;
    uniform vec3 uFogColor;
    uniform float uLight, uFogK;
    out vec4 o;
    void main() {
      vec3 col = uColor.rgb * uLight;
      float f = 1.0 - exp(-vFog * vFog * uFogK);
      col = mix(col, uFogColor, f);
      o = vec4(col * uColor.a, uColor.a);
    }`,
  );
  const wingU = uniforms(wingProg, ["uVP", "uColor", "uLight", "uFogColor", "uFogK"]);
  const wingVao = gl.createVertexArray();
  const wingVbo = gl.createBuffer();
  const wingBuf = new Float32Array(WING_VERTS.length * 3);
  {
    gl.bindVertexArray(wingVao);
    gl.bindBuffer(gl.ARRAY_BUFFER, wingVbo);
    gl.bufferData(gl.ARRAY_BUFFER, wingBuf.byteLength, gl.DYNAMIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);
  }
  /* 果蝇姿态：poseT 0=飞行 1=落地 */
  let poseT = 0;
  function fillFly(t, dt, poseTarget) {
    poseT += (poseTarget - poseT) * (1 - Math.exp(-dt * 2)); // 起飞/落地过渡 ~0.5s
    const airT = 1 - poseT;
    const lean = airT * 0.32; // 飞行前倾
    const cx = Math.cos(lean),
      sx = Math.sin(lean);
    const cy = -Math.sin(flyRot),
      sy = -Math.cos(flyRot);
    const ox = flyPos.x,
      oy = flyPos.y,
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
    /* 六条腿：抖动幅度 ∝ 脑活动——「接上神经元」 */
    const jitterAmp = 0.006 + 0.05 * activity;
    const pa = [0, 0, 0],
      pk = [0, 0, 0],
      pf = [0, 0, 0];
    const writeTube = (p0, p1, radius, col) => {
      const dx = p1[0] - p0[0],
        dy = p1[1] - p0[1],
        dz = p1[2] - p0[2];
      const len = Math.hypot(dx, dy, dz) || 1e-4;
      const wx = dx / len,
        wy = dy / len,
        wz = dz / len;
      // 正交基
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
        const lx = TUBE[i] * radius,
          ly = TUBE[i + 1] * len,
          lz = TUBE[i + 2] * radius;
        flyBuf[o++] = p0[0] + ux * lx + wx * ly + vx * lz;
        flyBuf[o++] = p0[1] + uy * lx + wy * ly + vy * lz;
        flyBuf[o++] = p0[2] + uz * lx + wz * ly + vz * lz;
        const nx = TUBE[i + 3],
          nz = TUBE[i + 5];
        flyBuf[o++] = ux * nx + vx * nz;
        flyBuf[o++] = uy * nx + vy * nz;
        flyBuf[o++] = uz * nx + vz * nz;
        flyBuf[o++] = col[0];
        flyBuf[o++] = col[1];
        flyBuf[o++] = col[2];
      }
    };
    /* tripod 步态：L1/R2/L3 与 R1/L2/L3 两组交替，步频 6-8Hz 随速度缩放 */
    const stepFreq = 6 + 2 * Math.min(1, flySpeed / 3);
    for (const leg of LEGS) {
      const j1 = Math.sin(t * 13 + leg.phase) * jitterAmp,
        j2 = Math.sin(t * 17 + leg.phase * 1.3) * jitterAmp,
        j3 = Math.cos(t * 15 + leg.phase) * jitterAmp;
      const dangle = airT * Math.sin(t * 2.6 + leg.phase) * 0.035; // 飞行悬垂轻摆
      const lerpP = (a, b) => a + (b - a) * poseT;
      const kL = [
        lerpP(leg.curlK[0], leg.standK[0]) + j1 * 0.5,
        lerpP(leg.curlK[1], leg.standK[1]),
        lerpP(leg.curlK[2], leg.standK[2]) + j3 * 0.5,
      ];
      const fL = [
        lerpP(leg.curlF[0], leg.standF[0]) + j1,
        lerpP(leg.curlF[1], leg.standF[1]) + j2,
        lerpP(leg.curlF[2], leg.standF[2]) + j3 + dangle,
      ];
      if (gaitT > 0.001) {
        // 摆动相抬腿前移、支撑相蹬地后移（局部 -Z 为前方）
        const group = (leg.i + (leg.s > 0 ? 0 : 1)) % 2;
        const ph = (t * stepFreq + group * 0.5 + leg.phase * 0.03) % 1;
        let gz = 0,
          gy = 0;
        if (ph < 0.5) {
          const u = ph / 0.5;
          gz = 0.24 * (0.5 - u);
          gy = 0.05 * Math.sin(Math.PI * u);
        } else {
          const u = (ph - 0.5) / 0.5;
          gz = 0.24 * (u - 0.5);
        }
        kL[1] += gy * 0.45 * gaitT;
        kL[2] += gz * 0.5 * gaitT;
        fL[1] += gy * gaitT;
        fL[2] += gz * gaitT;
      }
      rot(leg.a[0], leg.a[1], leg.a[2], pa);
      rot(kL[0], kL[1], kL[2], pk);
      rot(fL[0], fL[1], fL[2], pf);
      writeTube(pa, pk, 0.017, LEG_COLORS.femur);
      writeTube(pk, pf, 0.013, LEG_COLORS.tibia);
    }
    /* 膜翅：绕胸部连接点扇动，落地收拢后掠 */
    const flap = Math.sin(t * Math.PI * 2 * 11) * 0.9 * airT; // 飞行 ~11Hz 扇动
    const fold = poseT * 1.05;
    let wo = 0;
    for (const { s, v } of WING_VERTS) {
      // 收拢后掠（绕 root 的 rotY）
      const sweep = s * fold;
      const cs = Math.cos(sweep),
        ss = Math.sin(sweep);
      const x1 = v[0] * cs + v[2] * ss,
        z1 = -v[0] * ss + v[2] * cs,
        y1 = v[1];
      // 扇动（绕 root 的 rotZ）
      const ang = s * (0.25 + flap - poseT * 0.2);
      const ca = Math.cos(ang),
        sa = Math.sin(ang);
      const x2 = x1 * ca - y1 * sa,
        y2 = x1 * sa + y1 * ca;
      rot(WING_ROOT[0] * s + x2, WING_ROOT[1] + y2, WING_ROOT[2] + z1, w);
      wingBuf[wo++] = w[0];
      wingBuf[wo++] = w[1];
      wingBuf[wo++] = w[2];
    }
    /* 复眼发光：亮度/脉动 ∝ 对应侧视觉神经放电；eating 时双眼同闪一下 */
    let flash = 0;
    const ft = t - eatFlashT0;
    if (ft >= 0 && ft < 0.9) flash = (1 - ft / 0.9) * (0.65 + 0.35 * Math.sin(t * 26));
    const vL = Math.max(visionL, flash),
      vR = Math.max(visionR, flash);
    let eo = 0;
    const fillEye = (posArr, col) => {
      for (let i = 0; i < posArr.length; i += 3) {
        rot(posArr[i], posArr[i + 1], posArr[i + 2], w);
        eyeBuf[eo++] = w[0];
        eyeBuf[eo++] = w[1];
        eyeBuf[eo++] = w[2];
        eyeBuf[eo++] = col[0];
        eyeBuf[eo++] = col[1];
        eyeBuf[eo++] = col[2];
      }
    };
    fillEye(EYE_L_POS, eyeColorAt(vL, t, 0));
    fillEye(EYE_R_POS, eyeColorAt(vR, t, 2.1));
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
      bgl.uniform1f(uDim, 0.22);
      bgl.uniform1f(uSize, 3.0);
      bgl.bindVertexArray(vao);
      bgl.drawArrays(bgl.POINTS, 0, N);
      bgl.bindVertexArray(null);
    }
    function lose() {
      bgl.getExtension("WEBGL_lose_context")?.loseContext();
    }
    return {
      init,
      uploadSpikes,
      render,
      lose,
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
    const targetY = airborne ? groundY + 1.15 + Math.sin(flyBob) * 0.14 : groundY + 0.3; // 落地 y = 草面 + 腿长
    flyPos.y += (targetY - flyPos.y) * (1 - Math.exp(-dt * 3.5));
    activity += (activityTarget - activity) * (1 - Math.exp(-dt * 4));
    visionL += (visionLT - visionL) * (1 - Math.exp(-dt * 7));
    visionR += (visionRT - visionR) * (1 - Math.exp(-dt * 7));
    flySpeed += (flySpeedT - flySpeed) * (1 - Math.exp(-dt * 6));
    gaitT += ((flyState === "walking" ? 1 : 0) - gaitT) * (1 - Math.exp(-dt * 4));
    const flyFloats = fillFly(t, dt, airborne ? 0 : 1);

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
    /* 果蝇身体与六条腿（CPU 逐帧变换后整体上传，一次 draw） */
    gl.bindBuffer(gl.ARRAY_BUFFER, flyVbo);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, flyBuf, 0, flyFloats);
    gl.bindVertexArray(flyVao);
    gl.drawArrays(gl.TRIANGLES, 0, flyFloats / 9);
    /* 复眼：emissive 自发光（随视觉神经放电调制的顶点色，不走 N·L），一次小 draw */
    gl.useProgram(eyeProg);
    gl.uniformMatrix4fv(eyeU.uVP, false, vp);
    gl.uniform3fv(eyeU.uFogColor, pal.hor);
    gl.uniform1f(eyeU.uFogK, FOG_K);
    gl.bindBuffer(gl.ARRAY_BUFFER, eyeVbo);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, eyeBuf);
    gl.bindVertexArray(eyeVao);
    gl.drawArrays(gl.TRIANGLES, 0, EYE_VERTS);

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
    gl.bindVertexArray(grassVao);
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

    gl.useProgram(wingProg);
    gl.uniformMatrix4fv(wingU.uVP, false, vp);
    gl.uniform4fv(wingU.uColor, [0.92, 0.96, 1.0, 0.38]);
    gl.uniform1f(
      wingU.uLight,
      0.35 + 0.75 * (pal.ambient[0] + pal.ambient[1]) * 0.5 + pal.sunVis * 0.3,
    );
    gl.uniform3fv(wingU.uFogColor, pal.hor);
    gl.uniform1f(wingU.uFogK, FOG_K);
    gl.bindBuffer(gl.ARRAY_BUFFER, wingVbo);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, wingBuf);
    gl.bindVertexArray(wingVao);
    gl.drawArrays(gl.TRIANGLES, 0, WING_VERTS.length);

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

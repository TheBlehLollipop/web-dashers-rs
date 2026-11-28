// Browser front end for the Rust port: the game (simulation, triggers, sprite state) runs in wasm, this file only draws what it hands over.
import { AudioFx } from "./audio_fx.js";
import { createEndFx } from "./endfx.js";
import { createGL } from "./glr.js";
import { showLoading } from "./loading.js";
const params = new URLSearchParams(location.search);
const LEVEL = params.get("level") || "1";
const AUTO = params.get("auto");          // "N": play N frames without a keyboard (headless screenshots), jumping every ~JUMP frames
const JUMP = Number(params.get("jump") || 0);
const WASM = params.get("wasm") || "wd_core.wasm";
const SHEETS = "../ref/assets/sheets/";
const canvas = document.getElementById("c");
const ctx = canvas.getContext("2d"); // transparent: the level layer is the WebGL canvas behind it
const gcanvas = document.getElementById("g");
let GL = null;
try { if (new URLSearchParams(location.search).get("gl") !== "0") GL = createGL(gcanvas); } catch (e) { console.warn("WebGL unavailable, drawing everything with canvas 2D", e); }
const hud = document.getElementById("hud");
const W = 1138, H = 640;
// Phaser's Scale.FIT: the canvas is stretched to the largest whole-pixel size with the game's aspect ratio that fits the window, and centred.
const fitStyle = () => {
  const iw = innerWidth || W, ih = innerHeight || H, aspect = W / H;
  let w, h;
  if (iw / ih > aspect) { h = ih; w = Math.round(h * aspect); } else { w = iw; h = Math.round(w / aspect); }
  const left = Math.floor((iw - w) / 2), top = Math.floor((ih - h) / 2);
  for (const c of [document.getElementById("g"), canvas]) if (c) { c.style.width = w + "px"; c.style.height = h + "px"; c.style.left = left + "px"; c.style.top = top + "px"; }
};
fitStyle(); addEventListener("resize", fitStyle);

const fail = (e) => { document.body.insertAdjacentHTML("beforeend", `<pre style="position:fixed;left:8px;bottom:8px;color:#f66;font:12px monospace">${String(e && e.stack || e)}</pre>`); console.error(e); window.__error = String(e); };
window.addEventListener("error", (e) => fail(e.error || e.message));

let loaded = 0, onLoaded = () => {}; // files finished, for the loading bar
const loadImage = (url) => new Promise((res, rej) => { const i = new Image(); i.onload = () => { loaded++; onLoaded(); res(i); }; i.onerror = () => rej(new Error("image " + url)); i.src = url; });
const loadJson = async (url) => { const j = await (await fetch(url)).json(); loaded++; onLoaded(); return j; };
const loadText = async (url) => { const r = await fetch(url); if (!r.ok) throw new Error(url + " " + r.status); const t = await r.text(); loaded++; onLoaded(); return t; };

async function main() {
  let loading = null;
  if (!params.get("noload")) { try { loading = await showLoading(ctx, SHEETS); const LOAD_ESTIMATE = 96; onLoaded = () => loading.progress(loaded / LOAD_ESTIMATE); } catch (e) { console.warn("loading screen", e); } }
  if (params.get("noclip")) { try { const c = JSON.parse(localStorage.getItem("gd_settings") || "{}"); c.noclip = true; if (params.get("noclip") === "all") Object.assign(c, { noclipAccuracy: true, showFPS: true, showCPS: true }); localStorage.setItem("gd_settings", JSON.stringify(c)); } catch (e) {} } // test hook for benchmarks
  // ---- wasm ----
  const { instance } = await WebAssembly.instantiateStreaming(fetch(WASM, { cache: "no-cache" }), { env: { js_sin: Math.sin, js_cos: Math.cos, js_tan: Math.tan, js_acos: Math.acos, js_atan: Math.atan, js_atan2: Math.atan2, js_pow: Math.pow, js_random: Math.random } });
  const w = instance.exports;
  const enc = new TextEncoder(), dec = new TextDecoder();
  const put = (s) => { const b = enc.encode(s); const p = w.wd_alloc(Math.max(1, b.length)); new Uint8Array(w.memory.buffer, p, b.length).set(b); return [p, b.length]; };
  const text = (n) => dec.decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), n));

  const afx = new AudioFx();
  window.__WD = { w, put, text };

  // ---- atlases ----
  const sheets = ["GJ_GameSheet", "GJ_GameSheet02", "GJ_GameSheet03", "GJ_GameSheet04", "GJ_GameSheetGlow", "GJ_GameSheetIcons", "GJ_WebSheet", "GJ_EditorSheet01", "GJ_GameSheetEditor", "PixelSheet_01", "GJ_ParticleSheet", "GJ_LaunchSheet", "FireSheet_01-hd", "CCControlColourPickerSpriteSheet-uhd"];
  const atlas = {};
  await Promise.all(sheets.map(async (name) => {
    const [json, img] = await Promise.all([loadText(SHEETS + name + ".json"), loadImage(SHEETS + name + ".png")]);
    const [np, nl] = put(name), [jp, jl] = put(json);
    w.wd_atlas_add(np, nl, jp, jl);
    const j = JSON.parse(json);
    let frames = j.frames;
    if (!frames && j.textures) frames = j.textures.flatMap((t) => t.frames);
    if (Array.isArray(frames)) frames = Object.fromEntries(frames.map((f) => [f.filename, f]));
    atlas[name] = { frames, img };
  }));

  // ---- BeastAnim descriptions, mirror portals ----
  await Promise.all([1, 2, 3, 4, 5].map(async (k) => {
    const name = `GJBeast0${k}`;
    try { const [np, nl] = put(name), [jp, jl] = put(await loadText(SHEETS + name + "_AnimDesc.json")); w.wd_beast_desc_add(np, nl, jp, jl); } catch (e) {}
  }));
  // `localStorage` writes the game logic asked for (saved coins), as `[[key, value], ...]`
  const flushStorage = () => { try { for (const [k, v] of JSON.parse(text(w.wd_coins_outbox()))) localStorage.setItem(k, v); } catch (e) {} };

  // ---- level (can be switched from the level select) ----
  // the sprite images of the level in the game (`wd_render_frames` lists them by index); the editor builds a new table whenever it rebuilds its view
  let frameNames = [], edFramesVer = -1;
  function loadFrameTable() {
    const frameTable = JSON.parse(text(w.wd_render_frames()));
    frameNames = frameTable;
    frames = frameTable.map(([a, f]) => { const at = atlas[a]; const d = at && at.frames[f]; return d ? { img: at.img, d } : null; });
  }
  let LEVEL_ID = LEVEL, info = {}, bgImg = null, groundImg = null, ground2Img = null, frames = [];
  const standalone = {}; window.__standalone = standalone;
  const loadLevel = async (id) => loadLevelText(id, await loadText(`../ref/assets/levels/${id}.txt`));
  window.__WD.loadLevelText = (...a) => loadLevelText(...a); // test hook
  const loadLevelText = async (id, levelText) => {
    LEVEL_ID = id;
    { let ldm = 0; try { ldm = JSON.parse(localStorage.getItem("gd_settings") || "{}").enableLDM ? 1 : 0; } catch (e) {} w.wd_set_ldm(ldm); } // Low Detail Mode counts from the moment the level is built
    w.wd_set_official(/^\d{5,}$/.test(id) ? 0 : 1);
    w.wd_visual_seed((Math.random() * 4294967295) >>> 0);
    {
      const official = !/^\d{5,}$/.test(id), key = official ? "level_" + id : id;
      const ls = (k, d) => { try { return localStorage.getItem(k) ?? d; } catch (e) { return d; } };
      const ids = (k) => { try { const a = JSON.parse(ls(k, "[]")); return Array.isArray(a) ? a.map(String).join(",") : ""; } catch (e) { return ""; } };
      const [kp, kl] = put(key), [sp, sl] = put(ids("gd_secretCoins_" + key)), [up, ul] = put(ids("gd_userCoins_" + key));
      w.wd_coins_config(official ? 1 : 0, key.startsWith("local_") ? 1 : 0, 0, kp, kl, sp, sl, up, ul, parseInt(ls("gd_totalsecretcoins", "0"), 10) || 0, parseInt(ls("gd_totalusercoins", "0"), 10) || 0);
    }
    window.__loadBench = (n) => { const t0 = performance.now(); for (let k = 0; k < n; k++) { const [p, l] = put(levelText); w.wd_game_load_level(p, l); w.wd_game_after_load(); w.wd_game_restart_level(-1); } return (performance.now() - t0) / n; }; // test hook
    const T = [performance.now()];
    { const [p, l] = put(levelText); w.wd_game_load_level(p, l); } T.push(performance.now());
    w.wd_mirror_enable(1);
    w.wd_game_after_load(); T.push(performance.now());
    w.wd_game_restart_level(-1); T.push(performance.now()); window.__loadT = T.slice(1).map((v, i) => +(v - T[i]).toFixed(1)); // test hook: ms of load / after_load / restart
    info = JSON.parse(text(w.wd_render_info()));
    bgImg = await loadImage(`../ref/assets/game-bg/game_bg_${info.background || "01"}_001-hd.png`).catch(() => null);
    const groundIdx = String((parseInt(info.ground || "00", 10) || 0) + 1).padStart(2, "0");
    groundImg = await loadImage(`../ref/assets/game-ground/groundSquare_${groundIdx}_001.png`).catch(() => null);
    ground2Img = await loadImage(`../ref/assets/game-ground/groundSquare_${groundIdx}_2_001.png`).catch(() => null);
    standalone["groundSquare_main"] = groundImg; standalone["groundSquare_second"] = ground2Img;
    w.wd_render_setup(groundImg ? groundImg.width : 1012, ground2Img ? 1 : 0, bgImg ? bgImg.height : 0);
    if (params.get("mode")) w.wd_game_set_start(Number(params.get("mode")), Number(params.get("speed") || 0), params.get("mini") ? 1 : 0, params.get("flip") ? 1 : 0, params.get("mirror") ? 1 : 0, params.get("dual") ? 1 : 0); // test hook
    w.wd_game_restart_level(-1);
    loadFrameTable();
  };
  const levels = await (async () => { try { const src = await loadText("../ref/assets/scripts/game/allLevels.js"); const win = {}; new Function("window", src)(win); return win.allLevels || []; } catch (e) { return []; } })();
  let loadingLevel = false;
  // an online level (web/online.js): the level text was downloaded, start it like a level picked in the level select
  // a black cover over everything (the scene's `fadeOut` rectangle / camera fade); `fadeTo` resolves when it got there
  let fadeA = 0, fade = null;
  const fadeTo = (to, ms) => new Promise((res) => { if (ms <= 0) { fadeA = to; fade = null; res(); } else fade = { from: fadeA, to, t0: performance.now(), ms, res }; });
  const stepFade = (now) => { if (!fade) return; const k = Math.min(1, (now - fade.t0) / fade.ms); fadeA = fade.from + (fade.to - fade.from) * k; if (k >= 1) { const r = fade.res; fade = null; r(); } };
  window.__onlinePress = () => { afx.stopMusic(); afx.playEffect("playSound_01", { volume: 1 }); };
  window.__playOnline = async (info) => {
    loadingLevel = true; window.__onlineName = info.name; window.__onlineActive = true;
    try {
      await fadeTo(1, 150); w.wd_on_close(); if (w.wd_em_close) w.wd_em_close(); window.__createdPlayId = info.created || null; // a created level: quitting goes back to its level view // `_closePlayMenu(false, restart)`: the screen goes black in 150 ms, then the scene restarts into the level
      window.__songData = info.songData || null; songBuf = null;
      await ensureAudio(); if (ac && !songBuf && window.__songData) { try { songBuf = await ac.decodeAudioData(window.__songData.slice(0)); } catch (e) { songBuf = null; } }
      await loadLevelText(info.id, info.text); w.wd_menu_mode(0);
      loadingLevel = false; startPlaying(); fadeA = 0; fade = null; // the restarted scene has no cover
    } finally { loadingLevel = false; fade = null; fadeA = 0; }
  };
  const loadSong = (id) => {
    const entry = levels.find((l) => l[2] === "level_" + id);
    window.__songData = null; songBuf = null;
    if (AUTO || !entry) return Promise.resolve();
    const file = entry[4] || String(entry[1]).replaceAll(" ", "");
    return fetch(`../ref/assets/music/${file}.mp3`).then((r) => r.arrayBuffer()).then(async (ab) => { window.__songData = ab; if (ac) songBuf = await ac.decodeAudioData(ab.slice(0)); }).catch(() => {});
  };
  let ac = null, analyser = null, gain = null, source = null, songBuf = null;
  await loadLevel(LEVEL);
  loadSong(LEVEL);

  // ---- UI (menus) ----
  w.wd_ui_new(W);
  // the pixels of the colour picker artwork (the editor's colour picker finds colours in them, as `_openEditorColorPickerPopup` does with a canvas)
  { const sheet = atlas["CCControlColourPickerSpriteSheet-uhd"];
    const grab = (frame, size) => { const f = sheet.frames[frame].frame, c = document.createElement("canvas"); c.width = c.height = size; const g = c.getContext("2d", { willReadFrequently: true }); g.drawImage(sheet.img, f.x, f.y, f.w, f.h, 0, 0, size, size); return g.getImageData(0, 0, size, size).data; };
    [["huePickerBackground.png", 252], ["colourPickerBackground.png", 185], ["colourPickerShadow.png", 185], ["colourPickerOverlay.png", 185]].forEach(([fr, sz], k) => {
      const px = grab(fr, sz), p = w.wd_alloc(px.length); new Uint8Array(w.memory.buffer, p, px.length).set(px); w.wd_ed_cp_tex(k, p, px.length);
    }); }
  for (const [name, file] of [["GJ_MenuBeta", "GJ_MenuBeta"], ["square04_001", "square04_001"], ["GJ_button01", "GJ_button_01"], ["sliderBar", "sliderBar"], ["sliderBar2", "sliderBar2"], ["slidergroove2", "slidergroove2"], ["GJ_square67", "GJ_square67"], ["GJ_button05", "GJ_button_05"], ["GJ_button02", "GJ_button_02"], ["square01_001", "square01_001"], ["GJ_square01", "GJ_square01"]].map(([n, f]) => [n, `../ref/assets/sprites/${f}.png`]).concat(window.WD_ASSETS || [])) { const img = await loadImage(file).catch(() => null); if (img) { standalone[name] = img; const [p, l] = put("/" + name); w.wd_ui_size(p, l, img.width, img.height); } }
  const fonts = {};
  for (const name of ["bigFont", "goldFont"]) {
    const [fnt, img] = await Promise.all([loadText(`../ref/assets/fonts/${name}.fnt`), loadImage(`../ref/assets/fonts/${name}.png`)]);
    const chars = {}, kern = {};
    let size = 0;
    for (const line of fnt.split("\n")) {
      const kv = Object.fromEntries([...line.matchAll(/(\w+)=("[^"]*"|\S+)/g)].map((m) => [m[1], m[2].replace(/"/g, "")]));
      if (line.startsWith("info ")) size = Number(kv.size);
      else if (line.startsWith("char ")) chars[kv.id] = { x: +kv.x, y: +kv.y, w: +kv.width, h: +kv.height, xo: +kv.xoffset, yo: +kv.yoffset, adv: +kv.xadvance };
      else if (line.startsWith("kerning ")) kern[kv.first + "," + kv.second] = +kv.amount;
    }
    fonts[name] = { img, chars, kern, size };
    const [np, nl] = put(name), [fp, fl] = put(fnt); w.wd_ui_font(np, nl, fp, fl);
  }
  const hidden = ["level_22", "level_67"];
  window.__allLevels = levels;
  const visibleLevels = levels.filter((l) => !hidden.includes(l[2]));
  const pushLevels = (page) => { const [p, l] = put(JSON.stringify(visibleLevels.map((l) => [l[0], l[1], l[2], l[4] || null]))); w.wd_ui_levels(p, l, Math.max(0, page)); };
  pushLevels(visibleLevels.findIndex((l) => l[2] === "level_" + LEVEL));
  const refGround = await loadImage("../ref/assets/game-ground/groundSquare_01_001.png").catch(() => null);
  const pushProgress = () => {
    let total = 0;
    for (const l of visibleLevels) {
      const id = l[2];
      let slots = []; try { slots = JSON.parse(localStorage.getItem(`gd_secretCoins_${id}_slots`) || "[]") || []; } catch (e) {}
      const mask = slots.reduce((m, sl) => (sl >= 0 && sl < 3 ? m | (1 << sl) : m), 0);
      total += slots.length;
      const [p, n] = put(id);
      w.wd_ui_progress(p, n, parseFloat(localStorage.getItem("bestPercent_" + id) || "0"), parseFloat(localStorage.getItem("practiceBestPercent_" + id) || "0"), mask);
    }
    w.wd_ui_ground(groundImg ? groundImg.width : 1012, refGround ? refGround.height : 270, ground2Img ? 1 : 0, total);
  };
  const bump = (k) => { try { localStorage.setItem(k, String((parseInt(localStorage.getItem(k) || "0", 10) || 0) + 1)); } catch (e) {} };
  const bumpLevel = (kind) => bump("level" + kind + "_level_" + LEVEL_ID);
  pushProgress();
  const START_PLAYING = !!AUTO || params.get("play") === "1";
  if (!START_PLAYING) { w.wd_menu_mode(1); w.wd_ui_main_menu(); }
  else { w.wd_ui_start_game(); w.wd_ui_hud(); bump("gd_totalAttempts"); bumpLevel("Attempts"); w.wd_ui_attempt(1, w.wd_game_scene(0)); }
  let menuMode = !START_PLAYING;
  let paused = false, attempts = 1, practiceOn = false, won = false, winClock = 0, winStarted = false, playTime = 0, jumps = 0, wasHeld = false;
  const MESSAGES = ["Awesome!", "Good\nJob!", "Well\nDone!", "Impressive!", "Amazing!", "Incredible!", "Skillful!", "Brilliant!", "Not\nbad!", "Warp\nSpeed!", "Challenge\nBreaker!", "Reflex\nMaster!", "I am\nspeechless...", "You are...\nThe One!", "How is this\npossible!?", "You beat\nme..."];
  if (params.get("ui") === "select" && menuMode) { w.wd_ui_open_level_select(); const pg = Number(params.get("page") || -1); if (pg >= 0) for (let k = 0; k < pg; k++) w.wd_ui_switch(1); }
  if (params.get("ui") === "settings" && menuMode) { w.wd_ui_set_options(0.6, 1, 1); w.wd_ui_open_settings(); }

  // ---- drawing helpers ----
  let tintCache = new WeakMap(), tintPixels = 0;
  const TS = { n: 0, px: 0, h: {} }; window.__tint = TS; // test hook: tints painted on the CPU
  let bgTile = null, cpuCv = null, cpuCtx = null;
  const tinted = (fr, tint) => {
    // keyed by image object, then by frame rectangle, then by tint: nothing is built per call (the callers make a fresh `fr` object every time)
    let byFrame = tintCache.get(fr.img); if (!byFrame) tintCache.set(fr.img, byFrame = new Map());
    const f0 = fr.d.frame, fk = ((f0.x * 8192 + f0.y) * 8192 + f0.w) * 8192 + f0.h;
    let per = byFrame.get(fk); if (!per) byFrame.set(fk, per = new Map());
    let c = per.get(tint);
    if (c) return c;
    const { x, y, w: fw, h: fh } = fr.d.frame;
    const paint = (cv) => { TS.n++; TS.px += fw * fh; { const k = fw + 'x' + fh + (cv.tint === undefined ? '' : ' scratch') + ' ' + ((fr.img && fr.img.src) || '').split('/').pop() + '@' + fr.d.frame.x + ',' + fr.d.frame.y; TS.h[k] = (TS.h[k] || 0) + 1; } // a Phaser tint multiplies the colour and keeps the alpha: do the same per pixel (a canvas "multiply" would tint the transparent edges, e.g. the soft black shadow of the fonts)
      if (!cpuCv) { cpuCv = document.createElement("canvas"); cpuCtx = cpuCv.getContext("2d", { willReadFrequently: true }); }
      if (cpuCv.width < fw || cpuCv.height < fh) { cpuCv.width = Math.max(cpuCv.width, fw); cpuCv.height = Math.max(cpuCv.height, fh); }
      cpuCtx.clearRect(0, 0, fw, fh);
      cpuCtx.drawImage(fr.img, x, y, fw, fh, 0, 0, fw, fh);
      const im = cpuCtx.getImageData(0, 0, fw, fh), px = im.data;
      const tr = ((tint >> 16) & 255) / 255, tg = ((tint >> 8) & 255) / 255, tb = (tint & 255) / 255;
      for (let k = 0; k < px.length; k += 4) { px[k] *= tr; px[k + 1] *= tg; px[k + 2] *= tb; }
      const g = cv.getContext("2d");
      g.globalCompositeOperation = "source-over"; g.clearRect(0, 0, fw, fh);
      g.putImageData(im, 0, 0);
    };
    if (per.size >= (fw * fh <= 1024 ? 2048 : 12)) { // (small frames, e.g. particles, may keep many colours: they are cheap to store) // a colour that fades (or the menu's rainbow): repaint one canvas instead of making a new one every frame
      let sc = per.scratch;
      if (!sc) { sc = per.scratch = document.createElement("canvas"); sc.width = fw; sc.height = fh; sc.tint = -2; }
      if (sc.tint !== tint) { paint(sc); sc.tint = tint; }
      return sc;
    }
    c = document.createElement("canvas"); c.width = fw; c.height = fh;
    paint(c);
    tintPixels += fw * fh;
    if (tintPixels > 48e6) { // ~190 MB of coloured copies at most; start over
      tintCache = new WeakMap(); tintPixels = fw * fh;
      tintCache.set(fr.img, byFrame = new Map()); byFrame.set(fk, per = new Map());
    }
    per.set(tint, c);
    return c;
  };
  let snapOn = false; // (edge snapping was for the hi-res canvas; the 1138x640 canvas draws exactly like Phaser)
  const snapPx = (v) => Math.round(v * pxScale) / pxScale;
  // Phaser's ColorMatrix grayscale(1) is saturate(-1): every channel becomes the average of r, g and b; then the tint multiplies
  const grayCache = new WeakMap();
  // the ground tiles tint with a canvas "multiply" (only their soft edges differ by a shade from a per-pixel tint) and it runs on the GPU: the ground colour changes
  // every frame during a colour trigger, and repainting a 256x256 tile pixel by pixel on the CPU each time caused the hitches
  const opaqueCache = new WeakMap();
  const tintedOpaque = (img, tint) => {
    tint &= 0xffffff; if (tint === 0xffffff) return img;
    let e = opaqueCache.get(img);
    if (!e) { const cv = document.createElement("canvas"); cv.width = img.width; cv.height = img.height; e = { cv, g: cv.getContext("2d"), tint: -1 }; opaqueCache.set(img, e); }
    if (e.tint !== tint) {
      const g = e.g; g.globalCompositeOperation = "source-over"; g.clearRect(0, 0, e.cv.width, e.cv.height); g.drawImage(img, 0, 0);
      g.globalCompositeOperation = "multiply"; g.fillStyle = hex(tint); g.fillRect(0, 0, e.cv.width, e.cv.height);
      g.globalCompositeOperation = "destination-in"; g.drawImage(img, 0, 0); g.globalCompositeOperation = "source-over"; // keeps the picture's own transparency
      e.tint = tint;
    }
    return e.cv;
  };
  const grayTinted = (fr, tint) => {
    let by = grayCache.get(fr.img); if (!by) grayCache.set(fr.img, by = new Map());
    const f = fr.d.frame, key = ((f.x * 8192 + f.y) * 8192 + f.w) * 8192 + f.h + ":" + tint;
    let c = by.get(key); if (c) return c;
    c = document.createElement("canvas"); c.width = f.w; c.height = f.h;
    const g = c.getContext("2d", { willReadFrequently: true });
    g.drawImage(fr.img, f.x, f.y, f.w, f.h, 0, 0, f.w, f.h);
    const im = g.getImageData(0, 0, f.w, f.h), px = im.data;
    const tr = tint >= 0 ? ((tint >> 16) & 255) / 255 : 1, tg = tint >= 0 ? ((tint >> 8) & 255) / 255 : 1, tb = tint >= 0 ? (tint & 255) / 255 : 1;
    for (let i = 0; i < px.length; i += 4) { const a = (px[i] + px[i + 1] + px[i + 2]) / 3; px[i] = a * tr; px[i + 1] = a * tg; px[i + 2] = a * tb; }
    g.putImageData(im, 0, 0);
    by.set(key, c);
    return c;
  };
  const hex = (n) => "#" + (n & 0xffffff).toString(16).padStart(6, "0");
  const drawFrame = (fr, x, y, rot, sx, sy, alpha, tint, blend, flips) => {
    const d = fr.d, f = d.frame;
    const fw = f.w, fh = f.h;
    const src = d.sourceSize || { w: fw, h: fh }, ss = d.spriteSourceSize || { x: 0, y: 0 };
    if (snapOn && !rot && !flips && sx > 0 && sy > 0 && !d.rotated) {
      // upright sprites: put all four edges on whole device pixels, so two sprites that share an edge share the same pixels (no hairline gaps)
      const ox = -src.w / 2 + ss.x, oy = -src.h / 2 + ss.y;
      const L = snapPx(x + ox * sx), R = snapPx(x + (ox + fw) * sx), T = snapPx(y + oy * sy), B = snapPx(y + (oy + fh) * sy);
      if (R > L && B > T) {
        ctx.globalAlpha = Math.max(0, Math.min(1, alpha));
        ctx.globalCompositeOperation = blend === 1 ? "lighter" : "source-over";
        if (tint >= 0 && tint !== 0xffffff) ctx.drawImage(tinted(fr, tint), 0, 0, fw, fh, L, T, R - L, B - T);
        else ctx.drawImage(fr.img, f.x, f.y, fw, fh, L, T, R - L, B - T);
        ctx.globalAlpha = 1; ctx.globalCompositeOperation = "source-over";
      }
      return;
    }
    ctx.save();
    ctx.translate(x, y);
    if (rot) ctx.rotate(rot);
    ctx.scale((flips & 1 ? -1 : 1) * sx, (flips & 2 ? -1 : 1) * sy);
    ctx.globalAlpha = Math.max(0, Math.min(1, alpha));
    ctx.globalCompositeOperation = blend === 1 ? "lighter" : "source-over";
    const ox = -src.w / 2 + ss.x, oy = -src.h / 2 + ss.y;
    const ins = (fw < fr.img.width || fh < fr.img.height) ? 0.5 : 0; // Phaser frames here sample half a texel inside (core/main.js `insetFrameUVs`)
    if (tint >= 0 && tint !== 0xffffff) { const c = tinted(fr, tint); ctx.drawImage(c, ins, ins, fw - 2 * ins, fh - 2 * ins, ox, oy, fw, fh); }
    else if (d.rotated) { ctx.rotate(-Math.PI / 2); ctx.drawImage(fr.img, f.x, f.y, fh, fw, -oy - fh, ox, fh, fw); }
    else ctx.drawImage(fr.img, f.x + ins, f.y + ins, fw - 2 * ins, fh - 2 * ins, ox, oy, fw, fh);
    ctx.restore();
  };

  // ---- player visuals (web/player_vis.js) ----
  const pv = PlayerVis.create({ ctx, atlas, drawFrame, tinted, loadImage, loadText, loadJson, W, H, w, onIconAtlas: (base, frames) => { for (const [n, d] of Object.entries(frames)) { const [p, l] = put("icon_" + base + "/" + n); w.wd_ui_size(p, l, d.sourceSize.w, d.sourceSize.h); } } });
  window.__pv = pv;
  // the two extra icon sheets come with the garage and the mini-icon option
  for (const name of ["player_ball_00", "player_dart_00"]) { try { const [json, img] = await Promise.all([loadText(SHEETS + name + ".json"), loadImage(SHEETS + name + ".png")]); const [np, nl] = put(name), [jp, jl] = put(json); w.wd_atlas_add(np, nl, jp, jl); const j = JSON.parse(json); let fr = j.frames || (j.textures && j.textures.flatMap((t) => t.frames)); if (Array.isArray(fr)) fr = Object.fromEntries(fr.map((f) => [f.filename, f])); atlas[name] = { frames: fr, img }; } catch (e) {} }
  await pv.init();
  if (window.__garageInit) await window.__garageInit({ w, put });
  const endfx = createEndFx({ ctx, atlas, drawFrame, W });
  const iconColor = (k, d) => { try { return parseInt(localStorage.getItem(k) || d, 16); } catch (e) { return parseInt(d, 16); } };
  let pvRestart = true, pvHidden = false;
  const pvStep = (dt) => {
    const camX = w.wd_game_scene(0), camY = w.wd_game_scene(1);
    const n = w.wd_pv_events();
    pv.players[0].hidden = pv.players[1].hidden = pvHidden;
    pv.frame(dt, { ox: -camX, oy: camY, camX, camY, menu: menuMode || !!w.wd_ui_menu_active(), paused, won: won || !!window.__pvForceWon, restart: pvRestart, mirrorP: w.wd_mirror_progress(), endX: w.wd_game_scene(5), endY: w.wd_game_scene(4), screenW: W, events: n ? text(n).split("|") : [] });
    pvRestart = false;
  };
  // text objects: a frame named `@text` / `<size>|<text>` is drawn with the bitmap font (origin at the centre)
  const textFrames = new Map();
  const textFrame = (idx) => {
    const fn = frameNames[idx];
    if (!fn || fn[0] !== "@text") return null;
    let fr = textFrames.get(idx);
    if (fr !== undefined) return fr;
    const fnt = fonts.bigFont, bar = fn[1].indexOf("|"), size = Number(fn[1].slice(0, bar)) || 36, str = fn[1].slice(bar + 1);
    fr = null;
    if (fnt) {
      const k = size / fnt.size, lines = str.split("\n"), lh = (fnt.lineHeight || fnt.size) * k;
      const widths = lines.map((ln) => { let cur = 0, prev = null; for (const ch of ln) { const g = fnt.chars[ch.codePointAt(0)]; if (!g) continue; cur += ((prev !== null ? fnt.kern[prev + "," + ch.codePointAt(0)] || 0 : 0) + g.adv) * k; prev = ch.codePointAt(0); } return cur; });
      const cw = Math.max(1, Math.ceil(Math.max(0, ...widths)) + 2), ch = Math.max(1, Math.ceil(lh * lines.length) + 2);
      const c = document.createElement("canvas"); c.width = cw; c.height = ch;
      const g2 = c.getContext("2d");
      lines.forEach((ln, li) => { let cur = (cw - widths[li]) / 2, prev = null; for (const chr of ln) { const cp = chr.codePointAt(0), g = fnt.chars[cp]; if (!g) continue; cur += (prev !== null ? fnt.kern[prev + "," + cp] || 0 : 0) * k; g2.drawImage(fnt.img, g.x, g.y, g.w, g.h, cur + g.xo * k, li * lh + g.yo * k, g.w * k, g.h * k); cur += g.adv * k; prev = cp; } });
      c.src = "text:" + idx;
      fr = { img: c, d: { frame: { x: 0, y: 0, w: cw, h: ch }, sourceSize: { w: cw, h: ch }, spriteSourceSize: { x: 0, y: 0 } } };
    }
    textFrames.set(idx, fr);
    return fr;
  };
  const fontOf = [null, fonts.bigFont, fonts.goldFont];
  const drawBitmapText = (fnt, str, size, x, y, scale, ox, oy, color) => {
    const k = size / fnt.size;
    // lines are laid out one under the other and centred on the widest one (`setCenterAlign`)
    const lines = String(str).split("\n").map((line) => {
      let cur = 0, prev = null;
      const glyphs = [];
      for (const ch of line) {
        const id = ch.codePointAt(0), g = fnt.chars[id];
        if (prev !== null) cur += (fnt.kern[prev + "," + id] || 0);
        if (g) { glyphs.push([g, cur]); cur += g.adv; }
        prev = id;
      }
      return { glyphs, width: cur * k };
    });
    const width = Math.max(...lines.map((l) => l.width));
    const lineH = 67 * k;
    ctx.save();
    ctx.translate(x, y); ctx.scale(scale, scale);
    ctx.translate(-width * ox, -lineH * lines.length * oy);
    lines.forEach((ln, li) => {
      const off = (width - ln.width) / 2, top = li * lineH;
      for (const [g, gx] of ln.glyphs) {
        if (!g.w) continue;
        const dx = off + (gx + g.xo) * k, dy = top + g.yo * k;
        if (color !== 0xffffff) { const c = tinted({ img: fnt.img, d: { frame: { x: g.x, y: g.y, w: g.w, h: g.h } } }, color); ctx.drawImage(c, dx, dy, g.w * k, g.h * k); }
        else ctx.drawImage(fnt.img, g.x, g.y, g.w, g.h, dx, dy, g.w * k, g.h * k);
      }
    });
    ctx.restore();
  };
  let camXUi = 0, camYUi = 0, shakeUi = 0;
  let uiTable = [], uiTableLen = -1;
  const drawUi = () => {
    const n = w.wd_ui_build(0);
    if (!n) return;
    const ub = new Float64Array(w.memory.buffer, w.wd_ui_ptr(), n * 16);
    const texts = JSON.parse(text(w.wd_ui_texts()));
    const fl = w.wd_ui_frames_len(); if (fl !== uiTableLen) { uiTable = JSON.parse(text(w.wd_ui_frames())); uiTableLen = fl; }
    const table = uiTable;
    for (let k = 0; k < n; k++) {
      const o = k * 16, kind = ub[o];
      ctx.save();
      if (kind === 6) { ctx.beginPath(); ctx.rect(ub[o + 1], ub[o + 2], ub[o + 12], ub[o + 13]); ctx.clip(); continue; }
      if (kind === 7) { ctx.restore(); ctx.restore(); continue; }
      ctx.globalAlpha = Math.max(0, Math.min(1, ub[o + 6]));
      if (kind !== 0 && (ub[o + 8] & 4)) ctx.globalCompositeOperation = "lighter";
      if (kind === 0) {
        const [a, f] = table[ub[o + 13]];
        let img, d;
        if (a === "") { img = standalone[f]; d = img && { frame: { x: 0, y: 0, w: img.width, h: img.height }, sourceSize: { w: img.width, h: img.height }, spriteSourceSize: { x: 0, y: 0 } }; }
        else { const at = atlas[a]; img = at && at.img; d = at && at.frames[f]; }
        if (img && d) {
          const fl = ub[o + 8], sw2 = d.sourceSize.w, sh2 = d.sourceSize.h;
          ctx.translate(ub[o + 1] + ((fl & 8) ? -camXUi : 0), ub[o + 2] + ((fl & 8) ? camYUi : 0)); if (ub[o + 5]) ctx.rotate(ub[o + 5]);
          ctx.scale(ub[o + 3], ub[o + 4]);
          if (fl & 4) ctx.globalCompositeOperation = "lighter";
          const fr = { img, d };
          // flips mirror the frame in place (the origin is applied first)
          if (fl & 1) { ctx.translate(sw2 * (1 - 2 * ub[o + 9]), 0); ctx.scale(-1, 1); }
          if (fl & 2) { ctx.translate(0, sh2 * (1 - 2 * ub[o + 10])); ctx.scale(1, -1); }
          const dx = -sw2 * ub[o + 9] + d.spriteSourceSize.x, dy = -sh2 * ub[o + 10] + d.spriteSourceSize.y;
          const tint = ub[o + 7], crop = ub[o + 12];
          const cw = crop > 0 ? Math.min(crop, d.frame.w) : d.frame.w;
          if (crop > d.frame.w && a === "") { for (let tx2 = 0; tx2 < crop; tx2 += d.frame.w) { const pw = Math.min(d.frame.w, crop - tx2); ctx.drawImage(img, 0, 0, pw, d.frame.h, dx + tx2, dy, pw, d.frame.h); } }
          else {
            const ins = a !== "" && cw === d.frame.w && (d.frame.w < img.width || d.frame.h < img.height) ? 0.5 : 0; // the half-texel UV inset of the original's atlas frames
            const ch = d.frame.h;
            if (fl & 16) { const tc = grayTinted(fr, tint); ctx.drawImage(tc, ins, ins, cw - 2 * ins, ch - 2 * ins, dx, dy, cw, ch); }
            else if (tint >= 0 && tint !== 0xffffff) { const tc = tinted(fr, tint); ctx.drawImage(tc, ins, ins, cw - 2 * ins, ch - 2 * ins, dx, dy, cw, ch); }
            else ctx.drawImage(img, d.frame.x + ins, d.frame.y + ins, cw - 2 * ins, ch - 2 * ins, dx, dy, cw, ch);
          }
        }
      } else if (kind === 1) {
        const t = texts[ub[o + 11]]; const size = ub[o + 12];
        const fnt = fontOf[ub[o + 15]];
        if (fnt) drawBitmapText(fnt, t, size, ub[o + 1] + ((ub[o + 8] & 8) ? -camXUi : 0), ub[o + 2] + ((ub[o + 8] & 8) ? camYUi : 0), ub[o + 3], ub[o + 9], ub[o + 10], ub[o + 14]);
        else {
          ctx.translate(ub[o + 1], ub[o + 2]); ctx.scale(ub[o + 3], ub[o + 4]);
          ctx.font = `${size}px Arial`; ctx.fillStyle = hex(ub[o + 14]);
          ctx.textAlign = ub[o + 9] === 1 ? "right" : ub[o + 9] === 0.5 ? "center" : "left"; ctx.textBaseline = ub[o + 10] === 1 ? "bottom" : ub[o + 10] === 0.5 ? "middle" : "top";
          ctx.fillText(t, 0, 0);
        }
      } else if (kind === 2) { ctx.fillStyle = hex(ub[o + 14]); ctx.fillRect(ub[o + 1], ub[o + 2], ub[o + 12], ub[o + 13]); }
      else if (kind === 3) { ctx.fillStyle = hex(ub[o + 14]); ctx.beginPath(); ctx.roundRect(ub[o + 1], ub[o + 2], ub[o + 12], ub[o + 13], ub[o + 15]); ctx.fill(); }
      else if (kind === 5) { // nine-slice: a standalone image or a frame of an atlas; trimmed frames keep their transparent margin (Phaser slices the untrimmed size)
        const key = texts[ub[o + 11]], slash = key.indexOf("/");
        let img = null, fx = 0, fy = 0, fw = 0, fh = 0, uw = 0, uh = 0, tx0 = 0, ty0 = 0;
        if (slash > 0) {
          const at = atlas[key.slice(0, slash)], d = at && at.frames[key.slice(slash + 1)];
          if (d) { img = at.img; fx = d.frame.x; fy = d.frame.y; fw = d.frame.w; fh = d.frame.h; uw = d.sourceSize ? d.sourceSize.w : fw; uh = d.sourceSize ? d.sourceSize.h : fh; tx0 = d.spriteSourceSize ? d.spriteSourceSize.x : 0; ty0 = d.spriteSourceSize ? d.spriteSourceSize.y : 0; }
        } else { img = standalone[key]; if (img) { fw = uw = img.width; fh = uh = img.height; } }
        if (img) {
          const x0 = ub[o + 1], y0 = ub[o + 2], W2 = ub[o + 12], H2 = ub[o + 13], b = ub[o + 15], tint = ub[o + 14];
          const grayN = !!(ub[o + 8] & 16), prep = grayN || (tint >= 0 && tint !== 0xffffff); // greyscale nodes and tinted ones are drawn from a prepared copy of the frame
          const fr0 = { img, d: { frame: { x: fx, y: fy, w: fw, h: fh } } };
          const src = grayN ? grayTinted(fr0, tint) : prep ? tinted(fr0, tint) : img;
          const sx0 = prep ? 0 : fx, sy0 = prep ? 0 : fy;
          const us = [0, b, uw - b, uw], vs = [0, b, uh - b, uh];
          const nineIns = slash > 0 && (fw < img.width || fh < img.height);
          const xd = [x0, x0 + b, x0 + W2 - b, x0 + W2], yd = [y0, y0 + b, y0 + H2 - b, y0 + H2]; // exact, like Phaser; the pieces overlap a little at their joins so no seam shows
          for (let i = 0; i < 3; i++) for (let j = 0; j < 3; j++) {
            let iu0 = Math.max(us[i], tx0), iu1 = Math.min(us[i + 1], tx0 + fw), iv0 = Math.max(vs[j], ty0), iv1 = Math.min(vs[j + 1], ty0 + fh);
            if (iu1 <= iu0 || iv1 <= iv0) continue;
            const full = nineIns; // atlas frames sample half a texel inside their outer edge
            const dl = full && i === 0 && iu0 === tx0 ? 0.5 : 0, dr = full && i === 2 && iu1 === tx0 + fw ? 0.5 : 0, dt = full && j === 0 && iv0 === ty0 ? 0.5 : 0, db = full && j === 2 && iv1 === ty0 + fh ? 0.5 : 0;
            const kx = (xd[i + 1] - xd[i]) / (us[i + 1] - us[i]), ky = (yd[j + 1] - yd[j]) / (vs[j + 1] - vs[j]);
            const ex = i < 2 && iu1 === us[i + 1] ? 0.4 : 0, ey = j < 2 && iv1 === vs[j + 1] ? 0.4 : 0;
            ctx.drawImage(src, sx0 + (iu0 - tx0) + dl, sy0 + (iv0 - ty0) + dt, iu1 - iu0 - dl - dr, iv1 - iv0 - dt - db, xd[i] + (iu0 - us[i]) * kx, yd[j] + (iv0 - vs[j]) * ky, (iu1 - iu0) * kx + ex, (iv1 - iv0) * ky + ey);
          }
        }
      }
      else if (kind === 4) { ctx.fillStyle = hex(ub[o + 14]); ctx.beginPath(); ctx.arc(ub[o + 1], ub[o + 2], ub[o + 12], 0, Math.PI * 2); ctx.fill(); }
      ctx.restore();
    }
  };

  // ---- audio: the level's song, and the analyser the music meter is fed from ----
  const MUSIC_VOL = 0.8; // `_effectiveVolume()` with the default user volume
  const peakBuf = new Float32Array(2048);
  const ensureAudio = async () => {
    if (ac || AUTO) return;
    ac = new AudioContext();
    gain = ac.createGain(); gain.gain.value = afx.userMusic() * 0.8;
    analyser = ac.createAnalyser(); analyser.fftSize = 2048;
    gain.connect(analyser); analyser.connect(ac.destination);
    w.wd_audio_live(1);
    afx.attach(ac, gain, analyser);
    if (menuMode) afx.enterMenu();
    if (window.__songData && !songBuf) songBuf = await ac.decodeAudioData(window.__songData.slice(0));
  };
  // `startMusic(StartPosOffset)`: the level's song offset (`kA13`) and the offset of the start position the attempt begins at
  const musicNow = () => (info.musicOffset || 0) + w.wd_game_music_sync_offset();
  const startMusic = (offset) => {
    if (!ac || !songBuf || menuMode) return;
    try { source && source.stop(); } catch (e) {}
    source = ac.createBufferSource(); source.buffer = songBuf; source.connect(gain);
    source.start(0, Math.max(0, Math.min(songBuf.duration - 0.1, offset)));
  };
  const stopSong = () => { try { source && source.stop(); } catch (e) {} source = null; };
  const pollSong = async () => { if (ac && !songBuf && window.__songData && !menuMode && !sliding && !paused && !wasDead) { songBuf = await ac.decodeAudioData(window.__songData.slice(0)); startMusic(musicNow()); } };

  // ---- input ----
  let held = false;
  // ---- keybinds: gd_keybinds = { action: [KeyboardEvent.code, ...] } with the defaults of `_loadKeybinds`
  const KB_DEF = { jump: ["Space", "KeyG", "KeyL", "KeyW"], jump2: ["ArrowUp", "KeyH"], pause: [], restart: ["KeyR"], noclip: ["KeyN"], showHitboxes: ["KeyH"], practice: ["KeyP"], startPosPrev: ["KeyQ"], startPosNext: ["KeyE"], saveCheckpoint: ["KeyZ"], deleteCheckpoint: ["KeyX"], editorTab1: ["Digit1"], editorTab2: ["Digit2"], editorTab3: ["Digit3"], editorMoveUp: ["KeyW"], editorMoveLeft: ["KeyA"], editorMoveDown: ["KeyS"], editorMoveRight: ["KeyD"], toggleMusic: ["KeyM"], screenshot: ["F2"], debugMode: ["F1"], zoomIn: ["Equal"], zoomOut: ["Minus"], toggleUI: ["KeyU"] };
  const kbCodes = (id) => { try { const m = JSON.parse(localStorage.getItem("gd_keybinds") || "{}") || {}; if (Array.isArray(m[id])) return m[id]; } catch (e) {} return KB_DEF[id] || []; };
  const kbIs = (id, code) => kbCodes(id).includes(code);
  const isJumpKey = (code) => kbIs("jump", code) || kbIs("jump2", code);
  const down = (e) => { if (e.type === "keydown" && !isJumpKey(e.code)) return; if (!menuMode && !paused && !sliding && !held) { bump("gd_totalJumps"); bumpLevel("Jumps"); } held = true; e.preventDefault(); if (!ac) ensureAudio().then(() => startMusic(musicNow())); };
  const up = (e) => { if (e.type === "keyup" && !isJumpKey(e.code)) return; held = false; };
  addEventListener("keydown", down); addEventListener("keyup", up);
  const toGame = (e) => { const r = canvas.getBoundingClientRect(); const sc = Math.min(r.width / W, r.height / H); const ox = r.left + (r.width - W * sc) / 2, oy = r.top + (r.height - H * sc) / 2; return [(e.clientX - ox) / sc, (e.clientY - oy) / sc]; };
  canvas.addEventListener("wheel", (e) => { const [x, y] = toGame(e); w.wd_ui_wheel(x, y, e.deltaY); e.preventDefault(); }, { passive: false });
  canvas.addEventListener("pointerdown", (e) => { if (loadingLevel) return; const [x, y] = toGame(e); w.wd_ui_pointer(0, x, y); if (!w.wd_ui_menu_active()) down(e); });
  addEventListener("pointermove", (e) => { const [x, y] = toGame(e); w.wd_ui_pointer(1, x, y); });
  addEventListener("pointerup", (e) => { if (loadingLevel) return; const [x, y] = toGame(e); w.wd_ui_pointer(2, x, y); up(e); });
  let sliding = false, wasDead = false;
  const startPlaying = () => { pvRestart = true; menuMode = false; paused = false; won = false; playTime = 0; jumps = 0; w.wd_ui_close_end(); attempts = 1; practiceOn = false; afx.stopMusic(); w.wd_ui_start_game(); applyOptions(); w.wd_game_begin_slide(); sliding = true; w.wd_ui_hud(); w.wd_ui_hud_pause(0); bump("gd_totalAttempts"); bumpLevel("Attempts"); w.wd_ui_attempt(1, w.wd_game_scene(0)); if (!ac) ensureAudio(); };
  // the slide-in ends: the song starts and the pause button fades in
  const pollSlide = () => { if (!sliding) return; const ss = w.wd_game_slide_state(); if (ss === 2) { sliding = false; w.wd_ui_hud_pause(1); if (ac) startMusic(musicNow()); else ensureAudio().then(() => startMusic(musicNow())); } };
  const saveBest = () => {
    const pct = Math.floor(w.wd_game_percent(0));
    const key = (practiceOn ? "practiceBestPercent_" : "bestPercent_") + "level_" + LEVEL_ID;
    try { if (pct > parseFloat(localStorage.getItem(key) || "0")) localStorage.setItem(key, String(pct)); } catch (e) {}
  };
  // coins on the level-complete screen: how many the level has, which were picked up this run, which were saved earlier
  const pushEndCoins = (o) => {
    const main = o ? o.main : levels.some((l) => l[2] === "level_" + LEVEL_ID);
    const count = o ? o.count : w.wd_game_coin_count(main ? 1 : 0);
    let saved = 0;
    try { const a = JSON.parse(localStorage.getItem(`gd_${main ? "secret" : "user"}Coins_level_${LEVEL_ID}_slots`) || "[]"); for (const sl of a) if (sl >= 0 && sl < 3) saved |= 1 << sl; } catch (e) {}
    w.wd_ui_end_coins(main ? 1 : 0, count, o ? o.run : 0, o ? o.saved : saved);
  };
  const restarted = () => { pvRestart = true; saveBest(); bump("gd_totalAttempts"); bumpLevel("Attempts"); attempts++; w.wd_ui_attempt(attempts, w.wd_game_scene(0)); if (practiceOn && !readSettings().practiceMusicSync) { if (!source) startMusic(info.musicOffset || 0); } else startMusic(musicNow()); }; // practice without music sync keeps the song playing
  // ---- level editor host glue ----
  const edDown = new Set();
  addEventListener("keydown", (e) => { if (window.WD_EDITOR_ON && e.key.startsWith("Arrow")) { edDown.add(e.key); e.preventDefault(); } });
  addEventListener("keyup", (e) => edDown.delete(e.key));
  const edJump = new Set();
  addEventListener("keydown", (e) => { if (window.WD_EDITOR_ON && kbIs("jump", e.code)) { edJump.add(e.code); e.preventDefault(); } });
  addEventListener("keyup", (e) => edJump.delete(e.code));
  const edKeys = () => (edDown.has("ArrowLeft") ? 1 : 0) | (edDown.has("ArrowRight") ? 2 : 0) | (edDown.has("ArrowUp") ? 4 : 0) | (edDown.has("ArrowDown") ? 8 : 0) | (edJump.size ? 16 : 0);
  // playtest: the music follows the start / pause / stop of the playtest (`_startEditorPlaytest` .. `_stopEditorPlaytest`)
  let edPtEvent = 0, edSongKey = null;
  // the song of the level being edited (the one picked in the level settings, else the saved one), and where the playtest starts in it:
  // the level's own song offset (`kA13`) plus the offset of the start position it begins at
  const edSongId = () => { const pid = w.wd_ed_pending_song(); if (!Number.isNaN(pid)) return pid; const rec = (window.WD_ED ? window.WD_ED.read() : []).find((x) => x.createdId === window.__edId); return rec && Number.isFinite(rec.songId) ? rec.songId : -1; };
  const edMusicStart = () => { let lvl = 0; try { lvl = JSON.parse(text(w.wd_render_info())).musicOffset || 0; } catch (e) {} return lvl + w.wd_ed_pt(3); };
  const edLoadSong = async () => {
    try {
      await ensureAudio();
      const id = edSongId(); let data = null;
      if (id > 0 && window.__songById) data = await window.__songById(id);
      else { const e = (window.__allLevels || [])[Math.max(0, -id - 1)]; if (e) { const r = await fetch(`../ref/assets/music/${e[4] || String(e[1]).replaceAll(" ", "")}.mp3`); if (r.ok) data = await r.arrayBuffer(); } }
      if (data && ac) { songBuf = await ac.decodeAudioData(data.slice(0)); edSongKey = id; }
    } catch (e) { console.warn("editor song", e); }
  };
  const edPtStep = () => {
    if (w.wd_ed_pt(1)) {
      w.wd_trig_time(performance.now());
      if (analyser) { w.wd_audio_live(1); analyser.getFloatTimeDomainData(peakBuf); let pk = 0; for (let k = 0; k < peakBuf.length; k++) { const v = Math.abs(peakBuf[k]); if (v > pk) pk = v; } w.wd_set_audio_peak(pk, afx.userMusic() * 0.8); } else w.wd_set_audio_meter(0.1);
    }
    const ev = w.wd_ed_pt(0);
    if (ev !== edPtEvent) {
      edPtEvent = ev;
      if (params.get("sim")) { /* headless picture: no sound */ }
      else if (w.wd_ed_pt(1) && !w.wd_ed_pt(2)) { if (edSongKey === edSongId() && songBuf) { if (ac) ac.resume(); startMusic(edMusicStart()); } else edLoadSong().then(() => { if (w.wd_ed_pt(1) && !w.wd_ed_pt(2)) startMusic(edMusicStart()); }); }
      else if (w.wd_ed_pt(1)) { if (ac) ac.suspend(); }
      else { stopSong(); if (ac) ac.resume(); }
    }
  };
  // the green / cyan path of the playtest and the red crosses of its deaths, drawn in the world
  const drawEdMarks = (ox, oy) => {
    if (!window.WD_EDITOR_ON) return;
    const n = w.wd_ed_marks(); if (!n) return;
    const a = new Float64Array(w.memory.buffer, w.wd_ed_marks_ptr(), n * 6);
    ctx.save(); ctx.lineCap = "butt";
    for (let i = 0; i < n; i++) {
      const o = i * 6;
      if (a[o] === 0) { ctx.strokeStyle = "#" + (a[o + 5] | 0).toString(16).padStart(6, "0"); ctx.globalAlpha = 0.95; ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(a[o + 1] + ox, a[o + 2] + oy); ctx.lineTo(a[o + 3] + ox, a[o + 4] + oy); ctx.stroke(); }
      else { const x = a[o + 1] + ox, y = a[o + 2] + oy, sz = 28; ctx.strokeStyle = "#ff0000"; ctx.globalAlpha = 1; ctx.lineWidth = 13; ctx.beginPath(); ctx.moveTo(x - sz, y - sz); ctx.lineTo(x + sz, y + sz); ctx.moveTo(x - sz, y + sz); ctx.lineTo(x + sz, y - sz); ctx.stroke(); }
    }
    ctx.restore();
  };
  window.__startEditor = async (levelString, createdId) => {
    await loadLevelText("editor", levelString);
    w.wd_ui_start_game(); w.wd_menu_mode(0); menuMode = false; won = false;
    const [p, n] = put(levelString), [ip, il] = put(createdId || "");
    w.wd_ed_open(p, n, ip, il);
    try { // the official song names and the song of this level for the level settings
      const rec = (window.WD_ED ? window.WD_ED.read() : []).find((x) => x.createdId === createdId);
      const [sp, sl] = put(JSON.stringify((window.__allLevels || []).map((l) => l[1])));
      w.wd_ed_set_songs(sp, sl, rec && Number.isFinite(rec.songId) ? rec.songId : -1);
    } catch (e) {}
    try { w.wd_ed_set_glow(JSON.parse(localStorage.getItem("gd_settings") || "{}").showEditorGlow ? 1 : 0); } catch (e) {} // `window.showEditorGlow` (off by default)
    window.WD_EDITOR_ON = true;
  };
  window.__stopEditor = () => { window.WD_EDITOR_ON = false; w.wd_ed_close(); };
  const doPause = () => {
    if (paused || menuMode || sliding || won) return; // `_pauseGame`: not in the menu, during the slide-in or after the level is won
    paused = true; if (ac) ac.suspend();
    { let mm = "true"; try { mm = localStorage.getItem("menuMusicEnabled") || "true"; } catch (e) {} w.wd_ui_set_options(afx.userMusic(), afx.userSfx(), mm === "true" ? 1 : 0); } // the volume sliders start at the saved values
    const entry = levels.find((l) => l[2] === "level_" + LEVEL_ID);
    const [p, n] = put(window.__onlineName && /^\d{5,}$/.test(LEVEL_ID) ? window.__onlineName : (entry ? entry[1] : "Level"));
    w.wd_ui_pause(1, p, n, parseFloat(localStorage.getItem("bestPercent_level_" + LEVEL_ID) || "0"), parseFloat(localStorage.getItem("practiceBestPercent_level_" + LEVEL_ID) || "0"));
  };
  // the pause menu quits at once; the level-complete menu fades out for 400 ms and back in (`fadeInFromBlack`)
  const quitToMenu = () => {
    const fromEnd = won;
    afx.playEffect("quitSound_01");
    const back = () => {
      sliding = false; doResume(); won = false; w.wd_ui_close_end(); if (source) { try { source.stop(); } catch (e) {} } menuMode = true; w.wd_menu_mode(1); w.wd_ui_main_menu_again(); afx.enterMenu();
      // back where the level was picked: the play menu of an online level, the level select for the main levels (`_onlineReturnToPlayMenu`, `_mainLevelReturnToSelect`)
      if (window.__createdPlayId) { const id = window.__createdPlayId; window.__createdPlayId = null; if (window.WD_ED) { window.WD_ED.push({ w, put, text }); const [p, n] = put(id); w.wd_em_open_new(p, n); } }
      else if (window.__onlineActive) { const i = w.wd_on_reopen(); if (i >= 0) { const h = window.WD_EVENTS[`Pop("OnView", ${i})`]; if (h) h("", { w, put, text, params }); } }
      else { const idx = visibleLevels.findIndex((l) => l[2] === "level_" + LEVEL_ID); if (idx >= 0) { pushLevels(idx); pushProgress(); w.wd_ui_open_level_select(); } }
    };
    if (fromEnd) { afx.stopMusic(); fadeTo(1, 400).then(() => { back(); return fadeTo(0, 400); }); } else back();
  };
  const doResume = () => { if (!paused) return; paused = false; w.wd_ui_pause(0, 0, 0, 0, 0); if (ac) ac.resume(); };
  const OPT_DEFAULTS = { noclip: 0, showPercentage: 1, percentDecimals: 0, showHitboxes: 0, progressBar: 0, hitboxTrail: 0, showFPS: 0, hidePauseBtn: 0, hidePracticeUI: 0, solidWaveTrail: 0, noclipAccuracy: 0, hitboxesOnDeath: 0, createObjectIds: 0, showObjectIds: 0, showCPS: 0, speedHack: 1, respawnDelay: 1, macroBot: 0, practiceMusicSync: 0, enableLDM: 0, showGlow: 1, enablePortalGuide: 1, enableOrbGuide: 0, enableMiniIcon: 0, cullDistance: 3 };
  let settingsRaw = null, settingsVal = {};
  const readSettings = () => { // parsed again only when the stored text changed
    let raw = null; try { raw = localStorage.getItem("gd_settings"); } catch (e) {}
    if (raw !== settingsRaw || raw === null) { settingsRaw = raw; try { settingsVal = JSON.parse(raw || "{}") || {}; } catch (e) { settingsVal = {}; } }
    return settingsVal;
  };
  // gd_settings -> gameplay (noclip, speed hack, respawn delay, smart start pos) and HUD (percentage, bar, pause button, practice UI)
  const applyOptions = () => {
    const saved = { ...OPT_DEFAULTS, ...readSettings() };
    for (const k of Object.keys(OPT_DEFAULTS)) { const [p, n] = put(k); w.wd_ui_opt_set(p, n, Number(saved[k] === true ? 1 : saved[k] === false ? 0 : saved[k])); }
    const num = (v, d) => (Number.isFinite(Number(v)) && v !== null && v !== "" ? Number(v) : d);
    w.wd_game_set_options(saved.noclip ? 1 : 0, saved.noclipAccuracy ? 1 : 0, num(saved.speedHack, 1), num(saved.respawnDelay, 1), saved.smartStartPos ? 1 : 0);
    w.wd_set_ldm(saved.enableLDM ? 1 : 0);
    w.wd_game_set_hitbox_opts(saved.showHitboxes ? 1 : 0, saved.hitboxTrail ? 1 : 0, saved.hitboxesOnDeath ? 1 : 0);
    w.wd_ui_apply_hud();
    return saved;
  };
  if (START_PLAYING) applyOptions();
  const openOptions = () => {
    const saved = { ...OPT_DEFAULTS, ...readSettings() };
    for (const k of Object.keys(OPT_DEFAULTS)) { const [p, n] = put(k); w.wd_ui_opt_set(p, n, Number(saved[k] === true ? 1 : saved[k] === false ? 0 : saved[k])); }
    w.wd_ui_open_options();
  };
  const saveOptions = () => {
    const o = JSON.parse(text(w.wd_ui_opts_json())), cur = readSettings();
    for (const k of Object.keys(o)) cur[k] = typeof OPT_DEFAULTS[k] === "number" && ["speedHack", "respawnDelay", "cullDistance"].includes(k) ? o[k] : !!o[k];
    try { localStorage.setItem("gd_settings", JSON.stringify(cur)); } catch (e) {}
    applyOptions();
  };
  addEventListener("pointerdown", () => { if (!ac && !AUTO) ensureAudio(); });
  addEventListener("keydown", (e) => { const c = e.key === "Backspace" ? 8 : e.key === "Enter" ? 13 : e.key.length === 1 ? e.key.charCodeAt(0) : 0; if (c) w.wd_ui_key(c); });
  const handleUi = () => {
    for (;;) {
      const n = w.wd_ui_event(); if (!n) break;
      const ev = text(n);
      if (ev === "OpenLevelSelect") { pushProgress(); w.wd_ui_open_level_select(); }
      else if (ev === "CloseLevelSelect") w.wd_ui_close_level_select();
      else if (ev === "PlayLevel") {
        const l = visibleLevels[w.wd_ui_page()];
        if (l && !loadingLevel) { // game-scene.js: "Downloading Level Assets...", input off until the level and song are loaded, then sound, close the select, restart into the level
          const id = l[2].replace("level_", ""); loadingLevel = true; window.__onlineActive = false;
          Promise.all([loadText(`../ref/assets/levels/${id}.txt`), loadSong(id)]).then(async ([txt]) => { await ensureAudio(); if (ac && !songBuf && window.__songData) songBuf = await ac.decodeAudioData(window.__songData.slice(0)); afx.playEffect("playSound_01", { volume: 1 }); w.wd_ui_close_level_select(); afx.stopMusic(); await loadLevelText(id, txt); w.wd_menu_mode(0); loadingLevel = false; startPlaying(); }).catch((e) => { loadingLevel = false; fail(e); });
        }
      }
      else if (ev === "Settings") { const g = (k, d) => { try { const v = localStorage.getItem(k); return v === null ? d : v; } catch (e) { return d; } }; w.wd_ui_set_options(parseFloat(g("userMusicVol", 1)), parseFloat(g("userSfxVol", 1)), g("menuMusicEnabled", "true") === "true" ? 1 : 0); w.wd_ui_open_settings(); }
      else if (ev === "Volume") { const m = w.wd_ui_option(0), sv = w.wd_ui_option(1); if (gain) gain.gain.value = m * 0.8; try { localStorage.setItem("userMusicVol", m); localStorage.setItem("userSfxVol", sv); } catch (e) {} }
      else if (ev === "ToggleMenuMusic") { try { localStorage.setItem("menuMusicEnabled", w.wd_ui_option(2) ? "true" : "false"); } catch (e) {} if (w.wd_ui_option(2)) { if (!afx.isPlaying()) afx.startMenuMusic(); } else afx.stopMusic(); }
      else if (window.WD_EVENTS[ev]) window.WD_EVENTS[ev](ev, { w, put, text, params });
      else if (ev === 'Popup("Options")') openOptions();
      else if (ev === "OptChanged") saveOptions();
      else if (ev.startsWith("Popup(")) { /* other settings popups: not ported yet */ }
      else if (ev === "LevelStats") {
        const l = visibleLevels[w.wd_ui_page()]; if (l) {
          const key = l[2], g = (k) => { try { return localStorage.getItem(k); } catch (e) { return null; } };
          const [p, n] = put(l[1] || "Unnamed");
          w.wd_pb_level_stats(p, n, parseInt(g("levelAttempts_" + key) || "0", 10) || 0, parseInt(g("levelJumps_" + key) || "0", 10) || 0, Math.floor(parseFloat(g("bestPercent_" + key) || "0")), Math.floor(parseFloat(g("practiceBestPercent_" + key) || "0")));
        }
      }
      else if (ev.startsWith("Sfx(")) afx.playEffect(ev.slice(5, -2));
      else if (ev === "Garage") { /* handled by garage.js */ }
      else if (ev === "Pause") doPause();
      else if (ev === "PauseResume") doResume();
      else if (ev === "PauseReplay") { doResume(); w.wd_ui_close_end(); won = false; w.wd_game_restart_current(); restarted(); }
      else if (ev === "PauseMenu") quitToMenu();
      else if (ev === "PausePractice") { practiceOn = !practiceOn; w.wd_game_practice(0); w.wd_ui_practice(practiceOn ? 1 : 0); doResume(); doPause(); }
      else if (ev === "Checkpoint") { if (practiceOn) w.wd_game_practice(1); }
      else if (ev === "ClearCheckpoint") { if (practiceOn) w.wd_game_practice(2); }
      else if (ev.startsWith("url:")) window.open(ev.slice(4), "_blank");
    }
  };
  const toggleSetting = (k) => { const cur = readSettings(); cur[k] = !cur[k]; try { localStorage.setItem("gd_settings", JSON.stringify(cur)); } catch (e) {} applyOptions(); };
  let shotNext = false;
  // one handler for every keybind (`window.kbOnPress` of the original); menus get the key first
  addEventListener("keydown", (e) => {
    const t = document.activeElement; if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
    if ((e.ctrlKey || e.metaKey) && !/^(Control|Meta)/.test(e.code)) return;
    if (w.wd_gar_is_open() && e.code === "Escape") { w.wd_gar_close(); e.preventDefault(); return; }
    if (menuMode || paused || won || window.WD_EDITOR_ON || w.wd_ui_menu_active()) { // menu shortcuts
      // a jump key does what the game-scene `update` does in a menu: open the level select, start the shown level, resume after a pause (75 ms later)
      const jumpInMenu = !window.WD_EDITOR_ON && !won && isJumpKey(e.code);
      if (jumpInMenu && e.repeat) { e.preventDefault(); return; }
      const [cp, cl] = put(jumpInMenu ? "Enter" : e.code);
      if (jumpInMenu && paused) { if (!w.wd_ui_menu_key_probe(cp, cl)) { /* a popup is open */ } else { setTimeout(() => { const [p2, l2] = put("Enter"); w.wd_ui_menu_key(p2, l2); }, 75); e.preventDefault(); return; } }
      else if (w.wd_ui_menu_key(cp, cl)) { e.preventDefault(); return; }
    }
    const k = (id) => kbIs(id, e.code);
    if (window.WD_EDITOR_ON) { // the editor: tabs, moving the selection, zoom
      const act = [["editorTab1", 0], ["editorTab2", 1], ["editorTab3", 2], ["editorMoveUp", 3], ["editorMoveLeft", 4], ["editorMoveDown", 5], ["editorMoveRight", 6], ["zoomIn", 7], ["zoomOut", 8]];
      for (const [id, n] of act) if (k(id) && (!e.repeat || n >= 3)) { w.wd_ed_key_action(n, e.shiftKey ? 0 : 1); if (n >= 3 && n <= 6 && !kbIs("jump", e.code)) e.preventDefault(); }
      if (k("toggleMusic") && !e.repeat) { if (ac) { if (ac.state === "running") ac.suspend(); else ac.resume(); } }
      if (k("screenshot") && !e.repeat) { shotNext = true; e.preventDefault(); }
      return;
    }
    if (k("toggleMusic") && !e.repeat) { if (ac) { if (ac.state === "running") ac.suspend(); else ac.resume(); } }
    if (k("screenshot") && !e.repeat) { shotNext = true; e.preventDefault(); }
    if (k("debugMode") && !e.repeat) { toggleSetting("showFPS"); e.preventDefault(); }
    if (k("toggleUI") && !e.repeat && !menuMode) window.__hideUI = !window.__hideUI;
    if (menuMode || e.repeat) return;
    const st = { ...OPT_DEFAULTS, ...readSettings() };
    if (k("pause") && !sliding && !won) { if (paused) doResume(); else doPause(); }
    if (!paused && !sliding && !won) {
      if (k("restart")) { w.wd_game_restart_current(); restarted(); }
      if (k("practice")) { practiceOn = !practiceOn; w.wd_game_practice(0); w.wd_ui_practice(practiceOn ? 1 : 0); startMusic(musicNow()); }
      if (practiceOn && k("saveCheckpoint")) w.wd_game_practice(1);
      if (practiceOn && k("deleteCheckpoint")) w.wd_game_practice(2);
    }
    if (paused) return;
    if (st.startPosSwitcher && (k("startPosPrev") || k("startPosNext")) && w.wd_game_start_pos_count() > 0) { w.wd_game_change_start_pos(k("startPosNext") ? 1 : -1); restarted(); } // `changeStartPos` does nothing in a level without start positions
    if (k("noclip")) toggleSetting("noclip");
    if (k("showHitboxes")) toggleSetting("showHitboxes");
  });
  addEventListener("keydown", (e) => { if (e.code === "Escape" && !menuMode && !window.WD_EDITOR_ON) { if (paused) quitToMenu(); else if (!sliding && !won) doPause(); else if (won) quitToMenu(); } });

  // ---- frame ----
  let last = performance.now(), frame = 0, fps = 0, acc = 0;
  // Like the original (Phaser `resolution: 1`, FIT): the canvas is 1138x640 and the browser stretches it to the window.
  let pxScale = 1;
  let glShown = true, shk = [0, 0];
  const fitCanvas = () => {
    shk = endfx.shake();
    ctx.setTransform(1, 0, 0, 1, shk[0], shk[1]);
    const sq = params.get("sq"); if (sq) { ctx.imageSmoothingEnabled = sq !== "off"; if (sq !== "off") ctx.imageSmoothingQuality = sq; } // test hook
  };
  // `drawHitboxes` of player.js: the outlines come from Rust as [kind, colour, alpha, width, ...] records
  const drawHitboxes = () => {
    if (menuMode || w.wd_ui_menu_active()) return;
    const n = w.wd_hitbox_build(); if (!n) return;
    const a = new Float64Array(w.memory.buffer, w.wd_hitbox_ptr(), n);
    ctx.save(); ctx.globalCompositeOperation = "source-over"; ctx.lineJoin = "miter";
    for (let i = 0; i < n;) {
      ctx.strokeStyle = hex(a[i + 1]); ctx.globalAlpha = a[i + 2]; ctx.lineWidth = a[i + 3];
      ctx.beginPath();
      if (a[i] === 0) { const c = a[i + 4]; ctx.moveTo(a[i + 5], a[i + 6]); for (let k = 1; k < c; k++) ctx.lineTo(a[i + 5 + k * 2], a[i + 6 + k * 2]); ctx.closePath(); i += 5 + c * 2; }
      else { ctx.arc(a[i + 4], a[i + 5], a[i + 6], 0, Math.PI * 2); i += 7; }
      ctx.stroke();
    }
    ctx.restore();
  };
  const render = () => {
    fitCanvas();
    const n = w.wd_render_build();
    const buf = new Float64Array(w.memory.buffer, w.wd_render_ptr(), n);
    const camX = buf[0], camY = buf[1], count = buf[2] | 0, tiles = buf[18] | 0;
    const shakeX = buf[31] || 0, shakeY = buf[32] || 0;
    pvHidden = !!buf[27];
    const useGL = !!GL && !(buf[33] > 0); // the editor view still draws with canvas 2D
    if (glShown !== useGL) { glShown = useGL; gcanvas.style.visibility = useGL ? "visible" : "hidden"; }
    if (useGL) { ctx.save(); ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.clearRect(0, 0, W, H); ctx.restore(); }
    // background
    if (useGL) {
      GL.begin(shk[0], shk[1]);
      if (bgImg) { // the image over its own mirror, tiled (a TileSprite); the tint is a vertex colour
        const tw = bgImg.width, ih = bgImg.height, th = ih * 2, tint = buf[22] & 0xffffff;
        const tx = ((buf[20] % tw) + tw) % tw, ty = ((buf[21] % th) + th) % th;
        for (let xx = -tx; xx < W; xx += tw) for (let yy = -ty; yy < H; yy += th) { GL.image(bgImg, xx, yy, tw, ih, tint, false); GL.image(bgImg, xx, yy + ih, tw, ih, tint, true); }
      } else GL.quad([0, 0, W, 0, W, H, 0, H], buf[22], 1, false);
      endfx.drawBack(GL);
      const base0 = 40 + tiles, ox0 = -camX + shakeX, oy0 = camY + shakeY;
      GL.view(buf[38] || 1, buf[39] || 0);
      for (let k = 0; k < count; k++) {
        const o = base0 + k * 12;
        if (buf[o + 10] === 1) break;
        const fr = frames[buf[o]]; if (!fr) continue;
        GL.frame(fr, buf[o + 1] + ox0, buf[o + 2] + oy0, buf[o + 3], buf[o + 4], buf[o + 5], buf[o + 6], buf[o + 7], buf[o + 8], buf[o + 9]);
      }
      endfx.drawWall(ox0, oy0, w.wd_game_scene(5), w.wd_game_scene(4), iconColor("iconMainColor", "04FF00"), !menuMode && !w.wd_ui_menu_active() && !window.WD_EDITOR_ON, GL.frame);
      GL.view(1, 0);
      GL.end();
    } else {
    // background
    ctx.globalAlpha = 1; ctx.globalCompositeOperation = "source-over";
    if (bgImg) {
      if (!bgTile || bgTile.src !== bgImg) { // the image above its own mirror (`_createMirroredBackgroundTexture`), made once per level
        const c = document.createElement("canvas"); c.width = bgImg.width; c.height = bgImg.height * 2;
        const g = c.getContext("2d"); g.drawImage(bgImg, 0, 0); g.translate(0, c.height); g.scale(1, -1); g.drawImage(bgImg, 0, 0);
        bgTile = c; bgTile.src = bgImg;
      }
      const tw = bgTile.width, th = bgTile.height;
      const tx = ((buf[20] % tw) + tw) % tw, ty = ((buf[21] % th) + th) % th; // a TileSprite shows texture pixel (x + tilePositionX, y + tilePositionY) at its (x, y)
      const e1 = 1 / pxScale;
      for (let xx = -tx; xx < W; xx += tw) for (let yy = -ty; yy < H; yy += th) ctx.drawImage(bgTile, xx, yy, tw + e1, th + e1);
      const tint = buf[22] & 0xffffff;
      if (tint !== 0xffffff) { ctx.globalCompositeOperation = "multiply"; ctx.fillStyle = hex(tint); ctx.fillRect(0, 0, W, H); ctx.globalCompositeOperation = "source-over"; }
    } else { ctx.fillStyle = hex(buf[22]); ctx.fillRect(0, 0, W, H); }
    endfx.drawBack();
    }
    // level objects (normal layer first, then the player, then the top layer)
    const base = 40 + tiles;
    if (buf[33] > 0) { // level editor: static level seen through the editor camera
      const z = buf[33]; ctx.save(); ctx.scale(z, z);
      const ex = -buf[34] / z, ey = -buf[35] / z;
      for (let k = 0; k < count; k++) {
        const o = base + k * 12; const fr = frames[buf[o]] || textFrame(buf[o]); if (!fr) continue;
        const sx = (buf[o + 1] + ex) * z, sy = (buf[o + 2] + ey) * z; if (sx < -400 || sx > W + 400 || sy < -400 || sy > H + 400) continue;
        drawFrame(fr, buf[o + 1] + ex, buf[o + 2] + ey, buf[o + 3], buf[o + 4], buf[o + 5], buf[o + 6], buf[o + 7], buf[o + 8], buf[o + 9]);
      }
      drawEdMarks(ex, ey);
      ctx.restore();
      // grid (`_updateEditorGrid`)
      const gs = 60, camX = buf[34] / z, camY2 = buf[35] / z;
      ctx.strokeStyle = "rgba(0,0,0,0.4)"; ctx.lineWidth = 1; ctx.beginPath();
      for (let x = -camX % gs; x < W / z + gs; x += gs) { ctx.moveTo(x * z + 0.5, 0); ctx.lineTo(x * z + 0.5, H); }
      for (let y = (-camY2 - 20) % gs; y < H / z + gs; y += gs) { ctx.moveTo(0, y * z + 0.5); ctx.lineTo(W, y * z + 0.5); }
      ctx.stroke();
      ctx.strokeStyle = "#fff"; ctx.beginPath();
      const sl = -buf[34]; if (sl >= -50 && sl <= W + 50) { ctx.moveTo(sl + 0.5, 0); ctx.lineTo(sl + 0.5, H); }
      const gl = (-20 + 60 * 8) * z - buf[35]; if (gl >= -50 && gl <= H + 50) { ctx.moveTo(0, gl + 0.5); ctx.lineTo(W, gl + 0.5); }
      ctx.stroke();
      return buf;
    }
    const ox = -camX + shakeX, oy = camY + shakeY;
    const vz = buf[38] || 1, va = buf[39] || 0, vOn = vz !== 1 || va !== 0;
    if (vOn) { ctx.save(); ctx.translate(W / 2, H / 2); ctx.rotate(va * Math.PI / 180); ctx.scale(vz, vz); ctx.translate(-W / 2, -H / 2); }
    let i = 0;
    for (; i < count; i++) {
      const o = base + i * 12;
      if (buf[o + 10] === 1) break;
      if (useGL) continue;
      const fr = frames[buf[o]]; if (!fr) continue;
      drawFrame(fr, buf[o + 1] + ox, buf[o + 2] + oy, buf[o + 3], buf[o + 4], buf[o + 5], buf[o + 6], buf[o + 7], buf[o + 8], buf[o + 9]);
    }
    if (!useGL) endfx.drawWall(ox, oy, w.wd_game_scene(5), w.wd_game_scene(4), iconColor("iconMainColor", "04FF00"), !menuMode && !w.wd_ui_menu_active() && !window.WD_EDITOR_ON);
    pv.drawWorld(ox, oy);
    endfx.drawPortalParticles(ox, oy);
    for (; i < count; i++) {
      const o = base + i * 12;
      const fr = frames[buf[o]] || textFrame(buf[o]); if (!fr) continue;
      drawFrame(fr, buf[o + 1] + ox, buf[o + 2] + oy, buf[o + 3], buf[o + 4], buf[o + 5], buf[o + 6], buf[o + 7], buf[o + 8], buf[o + 9]);
    }
    pv.drawTop(ox, oy);
    drawEdMarks(ox, oy);
    if (vOn) ctx.restore();
    // ground
    ctx.globalAlpha = 1; ctx.globalCompositeOperation = "source-over";
    if (groundImg) {
      const gy = buf[15], tint = buf[23];
      const tile = tintedOpaque(groundImg, tint);
      const tile2 = ground2Img && buf[25] ? tintedOpaque(ground2Img, buf[24]) : null;
      for (let t = 0; t < tiles; t++) {
        const tx = buf[40 + t];
        const gw = tile.width + 1 / pxScale; // a device pixel of overlap hides the seam between tiles
        ctx.drawImage(tile, 0, 0, tile.width, tile.height, tx, gy, gw, tile.height);
        if (tile2) ctx.drawImage(tile2, 0, 0, tile2.width, tile2.height, tx, gy, gw, tile2.height);
        if (buf[17]) { ctx.save(); ctx.translate(tx, buf[16]); ctx.scale(1, -1); ctx.drawImage(tile, 0, 0); if (tile2) ctx.drawImage(tile2, 0, 0); ctx.restore(); }
      }
    }
    // floor line and the dark edge shadows (`_buildGround`)
    const web = atlas["GJ_WebSheet"];
    const line = web.frames["floorLine_01_001.png"], shadow = web.frames["groundSquareShadow_001.png"];
    const lineAt = (y, flipY) => {
      if (!line) return;
      const sc = (W + 400) / line.sourceSize.w;
      ctx.save(); ctx.translate(buf[19], y); ctx.scale(sc, flipY ? -1 : 1); ctx.globalCompositeOperation = "lighter";
      ctx.drawImage(web.img, line.frame.x, line.frame.y, line.frame.w, line.frame.h, -line.sourceSize.w / 2 + line.spriteSourceSize.x, line.spriteSourceSize.y, line.frame.w, line.frame.h);
      ctx.restore();
    };
    const shadowAt = (y, flipY) => {
      if (!shadow) return;
      const sw = shadow.sourceSize.w * 0.7;
      for (const right of [false, true]) {
        ctx.save(); ctx.globalAlpha = 100 / 255; ctx.translate(right ? W + 1 : -1, y); ctx.scale(right ? -0.7 : 0.7, flipY ? -1 : 1);
        ctx.drawImage(web.img, shadow.frame.x, shadow.frame.y, shadow.frame.w, shadow.frame.h, right ? -shadow.sourceSize.w + (shadow.sourceSize.w - shadow.spriteSourceSize.x - shadow.frame.w) : shadow.spriteSourceSize.x, shadow.spriteSourceSize.y, shadow.frame.w, shadow.frame.h);
        ctx.restore();
      }
    };
    lineAt(buf[15], false); shadowAt(buf[15], false);
    if (buf[17]) { lineAt(buf[16], true); shadowAt(buf[16], true); }
    drawHitboxes();
    pv.drawScreen();
    return buf;
  };

  // no audio yet: a steady beat stands in for the music's level so the audio-reactive sprites move
  const beat = (ms) => 0.45 + 0.35 * Math.abs(Math.sin(ms / 1000 * Math.PI * 2));
  const step = (dtMs) => {
    w.wd_trig_time(performance.now());
    if (analyser) { w.wd_audio_live(1); analyser.getFloatTimeDomainData(peakBuf); let pk = 0; for (let k = 0; k < peakBuf.length; k++) { const v = Math.abs(peakBuf[k]); if (v > pk) pk = v; } w.wd_set_audio_peak(pk, afx.userMusic() * 0.8); } // audio.js `_effectiveVolume()`
    else w.wd_set_audio_meter(0.1); // no sound yet: the meter rests at 0.1, objects do not pulse
    const r = w.wd_game_update(dtMs, held ? 1 : 0);
    if ((r >> 30) & 1) restarted(); // died and restarted
    pollSong();
    pollSlide();
  };

  if (loading) { if (params.get("loadhold")) await new Promise((r) => setTimeout(r, Number(params.get("loadhold")))); await loading.done(); } // loadhold: test hook, keeps the loading screen up
  if (AUTO) {
    // headless: fixed 60 fps, scripted jumps, draw the last frame
    const frames = Number(AUTO);
    for (let f = 0; f < frames; f++) {
      held = JUMP > 0 && (f % JUMP) < 8;
      w.wd_trig_time(f * 16.6667);
      w.wd_set_audio_meter(beat(f * 16.6667));
      w.wd_game_update(1000 / 60, held ? 1 : 0);
      if (params.get("pvend") && f === Number(params.get("pvend"))) { window.__pvForceWon = true; w.wd_game_set_end(w.wd_game_scene(0) + 900); } // test hook
      pvStep(1 / 60);
    }
    if (params.get("bench")) { // ?auto=N&bench=M: milliseconds per frame of the sim, the sprite build, the canvas draw and the UI draw
      const M = Number(params.get("bench")), T = {}; const tm = (k, fn) => { const t0 = performance.now(); for (let i = 0; i < M; i++) fn(); T[k] = +((performance.now() - t0) / M).toFixed(3); };
      tm("update", () => w.wd_game_update(1000 / 60, 0)); tm("build", () => w.wd_render_build()); tm("render", () => render()); tm("ui", () => drawUi());
      window.__bench = "BENCH " + JSON.stringify(T) + " sprites=" + new Float64Array(w.memory.buffer, w.wd_render_ptr(), 3)[2]; console.log(window.__bench);
    }
    if (params.get("endfx")) { // test hook: ?endfx=MS draws the level-complete effects MS milliseconds in
      endfx.begin({ px: 700, py: 380, main: 0x04ff00, second: 0x00fbff, ldm: false, onText: () => { w.wd_ui_banner(0); }, onEnd: () => {} });
      for (let ms = 0; ms < Number(params.get("endfx")); ms += 1000 / 60) { endfx.update(1000 / 60); w.wd_ui_step(1000 / 60); }
    }
    const buf = render();
    if (params.get("endfx")) { endfx.drawFront(); drawUi(); }
    hud.textContent = `frame ${frames}  x=${buf[3].toFixed(0)}  sprites=${buf[2]} ${window.__bench || ""}`;
    window.__done = true;
    return;
  }

  if (params.get("ui") === "options" && menuMode) { openOptions(); const pg = Number(params.get("page") || 0); if (pg) { w.wd_ui_pointer(0, 0, 0); } }
  if (params.get("ui") === "garage" && menuMode) { WD_EVENTS["Garage"]("Garage", { w, put, text, params }).then(() => { if (params.get("gartab")) w.wd_gar_tab(Number(params.get("gartab"))); }); }
  const SIM = Number(params.get("sim") || 0);
  const DRAG = params.get("drag") ? params.get("drag").split(",").map(Number) : null;
  if (SIM) {
    const showErr = (m) => { hud.textContent = "ERR " + m; window.__done = true; }; // test hook: a failure shows up in the picture
    addEventListener("error", (e) => showErr(e.message)); addEventListener("unhandledrejection", (e) => showErr(String(e.reason && e.reason.stack || e.reason)));
    // headless script: frames at 60 fps, clicks (x,y pairs) every 10 frames, then one picture
    const cl = (params.get("clicks") || "").split(",").filter(Boolean).map(Number);
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    let ci = 0;
    if (params.get("ui") === "end") { const [p, n] = put(params.get("msg") || "Awesome!"); pushEndCoins({ main: params.get("main") !== "0", count: Number(params.get("coins") || 0), run: Number(params.get("run") || 0), saved: Number(params.get("saved") || 0) }); w.wd_ui_banner(0); w.wd_ui_end_layer(37, 120, 185, 0, p, n); }
    for (let f = 0; f < SIM; f++) {
      if (cl.length && f % 10 === 5 && ci < cl.length) { const x = cl[ci++], y = cl[ci++]; w.wd_ui_pointer(0, x, y); w.wd_ui_pointer(2, x, y); }
      if (DRAG) { // drag=startX,y,endX,frames,startFrame[,hold]: press, move in even steps, release (then wait `hold` more frames)
        const [dx0, dy0, dx1, dn, df, hold] = DRAG;
        if (f === df) w.wd_ui_pointer(0, dx0, dy0);
        if (f > df && f <= df + dn) w.wd_ui_pointer(1, dx0 + (dx1 - dx0) * (f - df) / dn, dy0);
        if (f === df + dn + (hold || 0)) w.wd_ui_pointer(2, dx1, dy0);
      }
      held = JUMP > 0 && (f % JUMP) < 8;
      if (params.get("endnear") && f === 1 && !menuMode) w.wd_game_set_end(w.wd_game_scene(0) + 1100); // test hook: the end portal just ahead
      w.wd_ui_step(1000 / 60); if (!window.WD_EDITOR_ON) { w.wd_trig_time(f * 16.6667); w.wd_set_audio_meter(beat(f * 16.6667)); } // the editor view does not step the triggers
      if (window.WD_EDITOR_ON && params.get("edpt") && f >= Number(params.get("edpt")) && !window.__edptDone) { window.__edptDone = true; w.wd_ed_playtest(0); } // test hook: start the playtest at this frame
      if (window.WD_EDITOR_ON && params.get("edop") && f >= Number(params.get("edop")) && !window.__edopDone) { window.__edopDone = true; w.wd_ed_playtest(Number(params.get("edopn") || 4)); } // test hook: open a popup (4 level settings, 5 start options, 6 background picker, 7 speed picker)
      if (window.WD_EDITOR_ON && params.get("edpm") && f >= Number(params.get("edpm")) && !window.__edpmDone) { window.__edpmDone = true; w.wd_ed_playtest(3); } // test hook: open the pause menu
      if (window.WD_EDITOR_ON) { edPtStep(); w.wd_ed_frame(1000 / 60, edKeys() | (held ? 16 : 0)); { const v = w.wd_ed_frames_ver(); if (v !== edFramesVer) { edFramesVer = v; loadFrameTable(); } } }
      if (window.WD_EDITOR_ON && w.wd_ed_pt(1)) pvStep(1 / 60);
      const rr = window.WD_EDITOR_ON ? 0 : w.wd_game_update(1000 / 60, held ? 1 : 0);
      pollSlide();
      handleUi();
      if (!menuMode && !paused) w.wd_ui_hud_update(Math.max(0, Math.min(100, w.wd_game_scene(0) * 0)));
      if (params.get("pause") && f === Number(params.get("pause"))) doPause();
      if (f % 10 === 9) await sleep(200);
    }
    await sleep(500);
    const b = render(); camXUi = b[0]; camYUi = b[1]; drawUi();
    hud.textContent = menuMode ? "" : `frame ${SIM} x=${b[3].toFixed(0)}` + (window.WD_EDITOR_ON ? ` pt event=${w.wd_ed_pt(0)} active=${w.wd_ed_pt(1)} paused=${w.wd_ed_pt(2)} marks=${w.wd_ed_marks()}` : "");
    window.__done = true;
    return;
  }
  const clicks = (params.get("clicks") || "").split(",").filter(Boolean).map(Number);
  let clickIdx = 0, tick = 0; const cpsTimes = []; let cpsHeld = false;
  let hudText = "";
  const PROF = !!params.get("prof"); let tp = 0; const P = window.__prof = {};
  const lp = PROF ? (k) => { const n = performance.now(); P[k] = (P[k] ?? 0) * 0.97 + (n - tp) * 0.03; tp = n; } : () => {};
  // the indicators of game-scene.js: "Noclip", accuracy, deaths, CPS and "Botted" in the big font at 40% alpha, and the FPS text in Arial
  let fpsAcc = 0, fpsFrames = 0, fpsStr = "FPS: 0";
  const drawHud = () => {
    const S = readSettings();
    if (S.showFPS) { ctx.save(); ctx.font = "17px Arial"; ctx.fillStyle = "#fff"; ctx.textBaseline = "top"; ctx.fillText(fpsStr, 4, 2); ctx.restore(); }
    const f = fontOf[1];
    if (!f || menuMode || w.wd_ui_menu_active()) return;
    ctx.save(); ctx.globalAlpha = 0.4;
    const t = (str, y, color) => drawBitmapText(f, str, 20, 10, y, 1, 0, 0, color);
    const nc = !!S.noclip, acc = nc && !!S.noclipAccuracy;
    if (nc) t("Noclip", 10, 0xffffff);
    if (acc) { t(w.wd_noclip_stat(0).toFixed(2) + "%", 30, 0xffffff); t(w.wd_noclip_stat(1) + " Deaths", 50, 0xffffff); }
    const cpsY = 10 + (nc ? 20 : 0) + (acc ? 40 : 0);
    if (S.showCPS) t(cpsTimes.length + " CPS", cpsY, held && !won && !wasDead ? 0x00ff00 : 0xffffff);
    if (w.wd_macro_state() === 2 && !paused && !won) t("Botted", cpsY + (S.showCPS ? 20 : 0), 0xff0000);
    ctx.restore();
  };
  // Phaser's `smoothStep` (TimeStep.smoothDelta): the frame time is the mean of the last 10, so a 200 Hz display does not make motion jitter with its timer
  const deltaHist = new Array(10).fill(1000 / 60); let deltaIdx = 0;
  const smoothDelta = (t) => {
    if (t > 200) t = Math.min(deltaHist[deltaIdx], 200);
    deltaHist[deltaIdx] = t; deltaIdx = (deltaIdx + 1) % 10;
    let sum = 0; for (let i = 0; i < 10; i++) sum += deltaHist[i];
    return sum / 10;
  };
  const loop = (now) => {
    if (PROF) { tp = performance.now(); P.frame = (P.frame ?? 16) * 0.97 + (now - last) * 0.03; }
    if (clicks.length && ++tick % 6 === 0 && clickIdx < clicks.length) { const x = clicks[clickIdx++], y = clicks[clickIdx++]; w.wd_ui_pointer(0, x, y); w.wd_ui_pointer(2, x, y); }
    const dt = Math.min(100, smoothDelta(Math.max(0, now - last))); last = now;
    w.wd_ui_step(dt);
    if (window.WD_EDITOR_ON) {
      edPtStep(); w.wd_ed_frame(dt, edKeys());
      { const v = w.wd_ed_frames_ver(); if (v !== edFramesVer) { edFramesVer = v; loadFrameTable(); } }
      if (window.__edArtDirty) { // the background or ground picked in the level settings: the images follow
        window.__edArtDirty = false;
        const inf = JSON.parse(text(w.wd_render_info()));
        const gi = String((parseInt(inf.ground || "00", 10) || 0) + 1).padStart(2, "0");
        Promise.all([loadImage(`../ref/assets/game-bg/game_bg_${inf.background || "01"}_001-hd.png`).catch(() => null), loadImage(`../ref/assets/game-ground/groundSquare_${gi}_001.png`).catch(() => null), loadImage(`../ref/assets/game-ground/groundSquare_${gi}_2_001.png`).catch(() => null)]).then(([b, g1, g2]) => {
          bgImg = b; groundImg = g1; ground2Img = g2; standalone["groundSquare_main"] = g1; standalone["groundSquare_second"] = g2;
          w.wd_render_setup(g1 ? g1.width : 1012, g2 ? 1 : 0, b ? b.height : 0);
        });
      }
    } else if (!paused) step(dt);
    lp("step");
    pvStep(paused ? 0 : dt / 1000);
    lp("pvStep");
    if (!won && endfx.active) endfx.reset();
    if (!paused) endfx.update(dt);
    endfx.portalUpdate(paused ? 0 : dt, w.wd_game_scene(5), w.wd_game_scene(4), iconColor("iconMainColor", "04FF00"), !menuMode && !window.WD_EDITOR_ON && !readSettings().enableLDM);
    if (PROF) { P.meter = w.wd_audio_dbg(0); P.pulse = w.wd_audio_dbg(1); P.nAS = w.wd_audio_dbg(2); P.sc = w.wd_audio_dbg(3); P.ap = w.wd_audio_dbg(8); P.nAnim = w.wd_audio_dbg(9); P.nSaw = w.wd_audio_dbg(10); P.nSpr = w.wd_audio_dbg(11); P.ac = ac ? ac.state : "none"; P.song = !!songBuf; }
    lp("fx");
    const buf = render(); camXUi = buf[0]; camYUi = buf[1];
    lp("render");
    if (!menuMode && buf[6] && !wasDead) { afx.playEffect("explode_11", { volume: 0.65 * afx.userSfx() }); if (!(practiceOn && !readSettings().practiceMusicSync)) stopSong(); }
    wasDead = !!buf[6];
    if (!menuMode) w.wd_ui_hud_update(Math.max(0, Math.min(100, buf[30] * 100)));
    if (!menuMode && !paused) {
      if (!won) { playTime += dt / 1000; if (held && !wasHeld) jumps++; wasHeld = held; }
      if (!won && w.wd_game_percent(2)) { won = true; winClock = 0; winStarted = false; } // the player starts flying into the end wall
      // player.js `playEndAnimation` takes 1000 ms; only when it ends (the player has hit the wall) does `_levelComplete` run
      if (won && !winStarted) {
        winClock += dt;
        if (winClock >= 1000) {
          winStarted = true;
          if (!practiceOn) {
            w.wd_coins_commit(); flushStorage();
            try {
              const lid = "level_" + LEVEL_ID, set = JSON.parse(localStorage.getItem("gd_completedSet") || "[]");
              if (!set.includes(lid)) { set.push(lid); localStorage.setItem("gd_completedSet", JSON.stringify(set)); localStorage.setItem("gd_completedLevels", String(set.length)); }
            } catch (e) {}
          }
          afx.playRaw("endStart_02", 0.8 * afx.userSfx());
          try { localStorage.setItem((practiceOn ? "practiceBestPercent_" : "bestPercent_") + "level_" + LEVEL_ID, "100"); } catch (e) {}
          endfx.begin({
            px: w.wd_game_scene(5) - w.wd_game_scene(0), py: 460 - w.wd_game_scene(4) + w.wd_game_scene(1),
            main: iconColor("iconMainColor", "04FF00"), second: iconColor("iconSecondaryColor", "00FBFF"), ldm: !!readSettings().enableLDM,
            onText: () => w.wd_ui_banner(practiceOn ? 1 : 0),
            onEnd: () => { pushEndCoins(); const [p, n] = put(MESSAGES[Math.floor(Math.random() * MESSAGES.length)]); w.wd_ui_end_layer(attempts, jumps, playTime, practiceOn ? 1 : 0, p, n); },
          });
        }
      }
    }
    lp("misc");
    endfx.drawFront();
    drawHud();
    if (!window.__hideUI || menuMode || window.WD_EDITOR_ON) drawUi(); // toggleUI hides the HUD in a level
    if (shotNext) { shotNext = false; const c = document.createElement("canvas"); c.width = W; c.height = H; const g = c.getContext("2d"); try { g.drawImage(gcanvas, 0, 0, W, H); } catch (e) {} g.drawImage(canvas, 0, 0, W, H); c.toBlob((b) => { const a = document.createElement("a"); a.href = URL.createObjectURL(b); a.download = `webdashers_${Date.now()}.png`; a.click(); setTimeout(() => URL.revokeObjectURL(a.href), 2000); }); }
    lp("ui");
    if (PROF && params.get("prof") === "2") { ctx.getImageData(0, 0, 1, 1); lp("flush"); } // forces the queued drawing to finish, so its cost shows up
    if (loadingLevel && !window.__onlineActive && fontOf[2]) drawBitmapText(fontOf[2], "Downloading Level Assets...", 20, W / 2, H / 2, 1, 0.5, 0.5, 0xffffff);
    stepFade(now);
    if (fadeA > 0) { ctx.save(); ctx.globalAlpha = Math.min(1, fadeA); ctx.globalCompositeOperation = "source-over"; ctx.fillStyle = "#000"; ctx.fillRect(0, 0, W, H); ctx.restore(); }
    handleUi();
    lp("hud0");
    fpsAcc += dt; fpsFrames++; // game-scene.js: "FPS: n", refreshed every 250 ms
    if (fpsAcc >= 250) { fpsStr = "FPS: " + Math.round(fpsFrames * 1000 / fpsAcc); fpsAcc = 0; fpsFrames = 0; }
    if (held && !cpsHeld && !won && !wasDead) cpsTimes.push(now); cpsHeld = held; while (cpsTimes.length && now - cpsTimes[0] > 1000) cpsTimes.shift();
    lp("hud");
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}
main().catch(fail);

// Differential test of the visual half of `_spawnObject`: the original (run with a recording stand-in for Phaser) versus visual.rs,
// object by object, on the shipped levels. usage: node tools/compare-sprites.mjs [levelIdx|-] [maxMismatchesShown]
import fs from "node:fs";
import vm from "node:vm";
import { makeRecorder } from "./sprite-ref.mjs";

const root = new URL("..", import.meta.url).pathname;
const { H, ctx, scene, created } = await makeRecorder();
const w = H.w;
const files = fs.readdirSync(root + "ref/assets/levels").filter((f) => f.endsWith(".txt")).sort();
const ONLY = process.argv[2] !== undefined && process.argv[2] !== "-" ? Number(process.argv[2]) : null;
const SHOW = Number(process.argv[3] || 8);
ctx.window.enableLDM = false;
const stream = (seed0) => { let s = seed0 >>> 0; return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296); };

// atlases into the Rust side
const enc = new TextEncoder();
const put = (str) => { const b = enc.encode(str); const p = w.wd_alloc(Math.max(1, b.length)); new Uint8Array(w.memory.buffer, p, b.length).set(b); return [p, b.length]; };
for (const f of fs.readdirSync(root + "ref/assets/sheets").filter((n) => n.endsWith(".json") && !/AnimDesc/.test(n))) {
  const [np, nl] = put(f.replace(/\.json$/, ""));
  const [jp, jl] = put(fs.readFileSync(root + "ref/assets/sheets/" + f, "utf8"));
  w.wd_atlas_add(np, nl, jp, jl);
}

const SKIP = new Set(["_animScene", "_eeInvisibleBlockEntry"]);
const norm = (v) => (typeof v === "number" ? (Object.is(v, -0) ? 0 : Number.isFinite(v) ? v : null) : v);
function snap(c) {
  const o = { atlas: c.atlas, frame: c.frameName, x: norm(c.x), y: norm(c.y), rotation: norm(c.rotation), scaleX: norm(c.scaleX), scaleY: norm(c.scaleY), alpha: norm(c.alpha), visible: c.visible, flipX: c.flipX, flipY: c.flipY, tint: c.tintv === null || c.tintv === undefined ? null : norm(c.tintv), blend: c.blend };
  for (const k of Object.keys(c)) {
    if (!k.startsWith("_") || SKIP.has(k)) continue;
    const v = c[k];
    if (k === "_eeHsv") o[k] = [v.h, v.s, v.v, +v.sAdd, +v.vAdd].map(norm);
    else if (k === "_gdWarp") o[k] = [v.rotationX, v.rotationY, v.scaleX, v.scaleY].map(norm);
    else if (typeof v === "number") o[k] = norm(v);
    else if (typeof v === "boolean" || typeof v === "string") o[k] = v;
    else if (Array.isArray(v) && v.every((x) => typeof x === "number" || typeof x === "string")) o[k] = v.map(norm);
  }
  return o;
}
const rjson = () => JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), lastLen)));
let lastLen = 0;

let objsTotal = 0, objsBad = 0, spritesTotal = 0;
const badById = {};
for (let li = 0; li < files.length; li++) {
  if (ONLY !== null && li !== ONLY) continue;
  const str = fs.readFileSync(root + "ref/assets/levels/" + files[li], "utf8");
  const lvl = ctx.parseLevel(str);
  const L = new ctx.LevelObject(scene, { _v: 0 });
  const normalized = ctx.LevelObject.prototype._normalizeTeleportPortals.call(L, lvl.objects);
  created.length = 0;
  ctx.__rnd = stream(12345); vm.runInContext("Math.random = __rnd", ctx);
  L._nextObjectId = 0;
  const js = [];
  for (const lo of normalized) {
    const def = ctx.getObjectFromId(lo.id);
    if (def && def.type === ctx.triggerType && ![1813, 2069, 3645].includes(Number(lo.id))) { L._nextObjectId++; js.push([]); continue; }
    const b = created.length;
    L._spawnObject(lo);
    js.push(created.slice(b).filter((c) => c.kind === "image" && !c._eeEditorTrigger).map(snap));
  }
  const [sp, sl] = put(str);
  w.wd_visual_seed(12345);
  lastLen = w.wd_visual_level(sp, sl);
  const rs = rjson();
  let bad = 0;
  normalized.forEach((lo, i) => {
    objsTotal++;
    const a = js[i], [r0, r1] = rs.ranges[i], b = rs.sprites.slice(r0, r1);
    spritesTotal += a.length;
    let msg = null;
    if (a.length !== b.length) msg = `count js=${a.length} rust=${b.length} (${a.map((s) => s.frame).join(",")} | ${b.map((s) => s.frame).join(",")})`;
    else for (let k = 0; k < a.length && !msg; k++) {
      const keys = new Set([...Object.keys(a[k]), ...Object.keys(b[k])]);
      for (const key of keys) if (JSON.stringify(a[k][key]) !== JSON.stringify(b[k][key])) { msg = `sprite ${k} (${a[k].frame}) ${key}: js=${JSON.stringify(a[k][key])} rust=${JSON.stringify(b[k][key])}`; break; }
    }
    if (msg) { bad++; objsBad++; badById[lo.id] = (badById[lo.id] || 0) + 1; if (process.env.PERID && badById[lo.id] === 1) console.log(`  id ${lo.id}: ${msg}`); if (bad <= SHOW) console.log(`${files[li]} object #${i} id ${lo.id}: ${msg}`); if (process.env.FULL && bad === 1) { const pick = (arr) => arr.map((q) => process.env.KEYS ? Object.fromEntries([["frame", q.frame], ...process.env.KEYS.split(",").map((k) => [k, q[k]])]) : q); console.log("JS:  ", JSON.stringify(pick(a))); console.log("RUST:", JSON.stringify(pick(b))); } }
  });
  if (process.env.TINT) {
    // colour channels: the same random colour manager history on both sides, then the tint every sprite would get
    const cmJs = new ctx.ColorManager();
    ctx.window.mainColor = 0x04ff00; ctx.window.secondaryColor = 0x00fbff;
    w.wd_cm_new(0x04ff00, 0x00fbff);
    const cp = w.wd_alloc(16 * 8);
    const cmop = (code, args) => { const m = new Float64Array(w.memory.buffer, cp, 16); m.fill(0); args.forEach((v, i) => (m[i] = v)); w.wd_cm_op(code, cp); };
    let sd = 99 + li; const r2 = () => ((sd = (sd * 1664525 + 1013904223) >>> 0) / 4294967296);
    const chans = [1, 2, 3, 4, 5, 1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007, 1009, 1012, 8, 20, 85, 100];
    for (let k = 0; k < 40; k++) {
      const ch = chans[Math.floor(r2() * chans.length)], c = { r: Math.floor(r2() * 256), g: Math.floor(r2() * 256), b: Math.floor(r2() * 256) };
      cmJs.triggerColor(ch, c, 0, null, undefined); cmop(4, [ch, c.r, c.g, c.b, 0, 0, 0, 0, 0, 0, 0, 0, NaN]);
    }
    const images = created.filter((c) => c.kind === "image" && !c._eeEditorTrigger);
    for (const spr of images) if (spr._eeColorChannel !== undefined && spr.tintv !== undefined) { /* keep spawn tint */ }
    const spawnTints = images.map((c) => (c.tintv === null || c.tintv === undefined ? null : c.tintv));
    // `created` also holds the sprites of earlier levels: take this level's tail
    const mine = images.slice(images.length - rs.sprites.length);
    mine.forEach((spr) => { if (spr._eeColorChannel !== undefined) L._applyChannelTint(spr, String(spr._eeColorChannel), cmJs.getHex(spr._eeColorChannel), cmJs); });
    lastLen = w.wd_visual_tints(sp, sl);
    const rt = rjson();
    let tb = 0;
    mine.forEach((spr, i) => {
      const jsT = spr.tintv === undefined ? null : spr.tintv;
      const rustT = rt[i] !== null ? rt[i] : rs.sprites[i].tint;
      if (jsT !== rustT && tb++ < 5) console.log(`  TINT ${files[li]} sprite ${i} (${rs.sprites[i].frame}) ch=${spr._eeColorChannel}: js=${jsT} rust=${rustT}`);
    });
    if (tb) { objsBad += tb; }
    console.log(`  tints: ${mine.length} sprites, ${tb} differ`);
  }
  console.log(`${files[li]}: ${normalized.length} objects, ${bad} differ`);
}
console.log(`${objsTotal} objects, ${spritesTotal} sprites: ${objsBad ? objsBad + " objects differ" : "all identical"}`);
console.log("most affected ids:", Object.entries(badById).sort((a, b) => b[1] - a[1]).slice(0, 15).map(([k, v]) => `${k}:${v}`).join(" "));
process.exit(objsBad ? 1 : 0);

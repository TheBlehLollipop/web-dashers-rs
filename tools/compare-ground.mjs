// Differential test of the ground/ceiling layout and fly-floor animation: level.js methods (verbatim) vs ground.rs.
// usage: node tools/compare-ground.mjs [seed] [sequences] [stepsPerSequence]
import fs from "node:fs";
import vm from "node:vm";
import { load } from "./physics-harness.mjs";

const root = new URL("..", import.meta.url).pathname;
const H = await load();
const { ctx, w } = H;
const src = fs.readFileSync(root + "ref/assets/scripts/core/level.js", "utf8");
const m0 = src.indexOf("  updateGroundTiles(cameraY = 0) {"), m1 = src.indexOf("  _applyVisualProps(scene, sprite");
vm.runInContext(`var S = 1, E = 0;
this.GroundJS = class GroundJS {\n${src.slice(m0, m1)}\n  getViewPad() { return 0; }\n};`, ctx);
let seed = Number(process.argv[2] || 1);
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const SEQ = Number(process.argv[3] || 300), STEPS = Number(process.argv[4] || 200);
const ptr = w.wd_alloc(8 * 8);
const args = (a) => { const m = new Float64Array(w.memory.buffer, ptr, 8); m.fill(0); a.forEach((v, i) => (m[i] = v)); };
const mk = () => ({ x: 0, y: 0, vis: true, setVisible(v) { this.vis = v; return this; }, setScale() { return this; } });
const same = (a, b) => (a === b) || (Number.isNaN(a) && Number.isNaN(b)) || (a === null && b === null);
const read = () => JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), last)));
let bad = 0, checks = 0, last = 0;
for (let s = 0; s < SEQ && bad < 5; s++) {
  const sw = Math.round(1024 + rnd() * 470), tw = rnd() < 0.5 ? 1012 : Math.round(300 + rnd() * 900);
  const has2 = rnd() < 0.5;
  const g = new ctx.GroundJS();
  const centre = sw / 2 - 150;
  const count = Math.ceil(sw / tw) + 2;
  ctx.screenWidth = sw;
  g._tileW = tw; g._scene = { _cameraY: 0, textures: { exists: () => has2, getFrame: () => null } };
  g._cameraXRef = { value: 0 };
  g._groundTiles = []; g._ceilingTiles = []; g._ground2Tiles = []; g._ceiling2Tiles = [];
  for (let i = 0; i < count; i++) {
    const wx = -centre + i * tw;
    for (const arr of [g._groundTiles, g._ceilingTiles, ...(has2 ? [g._ground2Tiles, g._ceiling2Tiles] : [])]) { const t = mk(); t._worldX = wx; arr.push(t); }
  }
  g._maxGroundWorldX = -centre + (count - 1) * tw;
  g._groundShift = (((-centre % tw) + tw) % tw);
  g._groundLine = mk(); g._ceilingLine = mk(); g._groundShadowL = mk(); g._groundShadowR = mk(); g._ceilingShadowL = mk(); g._ceilingShadowR = mk();
  g.resetGroundState(); g._flyFloorY = null;
  w.wd_ground_new(sw, tw);
  let cam = rnd() * 500;
  const log = [];
  for (let k = 0; k < STEPS && bad < 5; k++) {
    const r = rnd();
    let d;
    if (r < 0.08) { const en = rnd() < 0.7, cy = rnd() * 1200 - 100, h = rnd() < 0.5 ? 600 : 120 + rnd() * 600, vo = rnd() < 0.3, ch = rnd() < 0.5 ? h : (rnd() < 0.2 ? NaN : 200 + rnd() * 500);
      g.setFlyMode(en, cy, h, vo, ch); args([+en, cy, h, +vo, ch, 0]); w.wd_ground_op(0, ptr); d = `fly ${en} ${cy} ${h} ${vo} ${ch}`; }
    else if (r < 0.12) { const a = (rnd() - 0.5) * 4000; g.shiftGroundTiles(a); args([a]); w.wd_ground_op(2, ptr); d = "shift " + a; }
    else if (r < 0.15) { const cx = rnd() * 5000; g.resetGroundTiles(cx); args([cx]); w.wd_ground_op(3, ptr); d = "reset " + cx; }
    else if (r < 0.17) { g.resetGroundState(); w.wd_ground_op(4, ptr); d = "resetState"; }
    else { const dt = rnd() < 0.2 ? rnd() * 0.7 : 1 / 60; g.stepGroundAnimation(dt); args([dt]); w.wd_ground_op(1, ptr); d = "step " + dt; }
    log.push(d);
    cam += rnd() * 30;
    const cy = rnd() < 0.5 ? 0 : rnd() * 300, sx = rnd() < 0.3 ? (rnd() - 0.5) * 400 : 0, sy = rnd() < 0.3 ? (rnd() - 0.5) * 400 : 0;
    g._cameraXRef.value = cam; g.shakeOffsetX = sx; g.shakeOffsetY = sy;
    g.updateGroundTiles(cy);
    args([cam, cy, sx, sy, +has2]);
    last = w.wd_ground_update(ptr);
    const o = read();
    checks++;
    const jsTile = g._groundTiles.map((t) => t.x), jsW = g._groundTiles.map((t) => t._worldX);
    const cmp = [
      ["tileX", jsTile, o.tileX], ["worldX", jsW, o.worldX],
    ];
    let msg = null;
    for (const [n, a, b] of cmp) if (a.length !== b.length || a.some((v, i) => !same(v, b[i]))) msg = msg || `${n} js=${a.slice(0, 3)} rust=${b.slice(0, 3)}`;
    const sc = [["groundY", g._groundTiles[0].y, o.groundY], ["ceilingY", g._ceilingTiles[0].y, o.ceilingY], ["ceilVis", g._ceilingTiles[0].vis, o.ceilVis], ["lineX", g._groundLine.x, o.lineX],
      ["c2", has2 ? g._ceiling2Tiles[0].vis : false, o.c2], ["max", g._maxGroundWorldX, o.max], ["shift", g._groundShift, o.shift], ["target", g._groundTargetValue, o.target],
      ["active", g._flyGroundActive, o.active], ["anim", g._groundAnimating, o.anim], ["floorY", g.getFloorY(), o.floorY ?? NaN], ["camT", g.flyCameraTarget, o.camT],
      ["ceilLine", g._ceilingLine.y, o.ceilVis ? o.ceilingY : g._ceilingLine.y], ["shadow", g._ceilingShadowL.y, g._ceilingShadowL.y]];
    for (const [n, a, b] of sc) if (!same(a === undefined ? null : a, b === undefined ? null : b)) msg = msg || `${n} js=${a} rust=${b}`;
    if (msg) { bad++; console.log(`MISMATCH seq ${s} step ${k}: ${msg}\n  ops: ${log.slice(-5).join(" | ")}`); }
  }
}
console.log(`${SEQ} sequences, ${checks} frames: ${bad ? bad + " mismatches" : "all identical"}`);
process.exit(bad ? 1 : 0);

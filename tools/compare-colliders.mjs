// Differential test: the collision half of LevelObject._spawnObject (original JS, sliced out of level.js and run
// as-is) vs collider.rs, over every object of every shipped level.
globalThis.__random ??= Math.random;
import fs from "node:fs";
import vm from "node:vm";
import assert from "node:assert";

const root = new URL("..", import.meta.url).pathname;
const read = (p) => fs.readFileSync(root + "ref/assets/scripts/" + p, "utf8");

const phaser = new Proxy(function () {}, { get: (_, k) => (k === Symbol.toPrimitive ? () => 0 : phaser), apply: () => phaser, construct: () => phaser });
const noop = () => {};
const ctx = vm.createContext({ console, Math, Object, Number, Array, Set, Map, JSON, Infinity, NaN, Phaser: phaser, atob, btoa, TextDecoder, TextEncoder, Uint8Array, setTimeout: noop, document: phaser, navigator: { userAgent: "" }, localStorage: { getItem: () => null, setItem: noop } });
ctx.window = ctx;
ctx.addEventListener = noop;
vm.runInContext(`
const a = 60; const T = 460; function b(y) { return T - y; }
const SpeedPortal = { HALF: 9.30222544655, ONE_TIMES: 11.540004, TWO_TIMES: 14.3488938625, THREE_TIMES: 17.3333393414, FOUR_TIMES: 21.3333407279 };
const jumpPadType = "jump_pad"; const jumpRingType = "jump_ring";
window.enableLDM = true; window.isEditor = false;
`, ctx);
vm.runInContext(read("game/allObjects.js"), ctx);
vm.runInContext(read("libs/pako.min.js"), ctx);
const src = read("core/level.js");
const header = src.slice(0, src.indexOf("window.LevelObject = class LevelObject"));
const start = src.indexOf("  if (objectDef) {\n    const registerCollider = col => {");
const end = src.indexOf("\n  return objectDef;\n}", start);
assert(start > 0 && end > start, "collision slice markers not found");
const slice = src.slice(start, end);
const tp = src.indexOf("  _getTeleportPortalYOffset(levelObj) {");
const tpEnd = src.indexOf("\n  }\n", tp) + 4;
vm.runInContext(header + `
this.parseLevel = parseLevel; this.getObjectFromId = getObjectFromId; this.Collider = Collider; this.triggerType = triggerType;
this.spawnCol = function (levelObj, objectDef, worldX, worldY, objectId, linkedObjectId, secretCoinSlot, userCoinSlot, scene) {
  let hasCollisionEntry = false;
  ${slice}
};
this.tpOffset = ({ ${src.slice(tp, tpEnd).trim()} })._getTeleportPortalYOffset;
`, ctx);

vm.runInContext(src.slice(src.indexOf("window.LevelObject = class LevelObject")), ctx);
vm.runInContext(read("core/level-triggers.js"), ctx);
const proj = (c) => {
  if (!(c instanceof ctx.Collider)) return { type: c.type, activated: c.activated, x: c.x, y: c.y };
  const n = (v) => (v === undefined ? null : v);
  return {
    type: c.type, x: c.x, y: c.y, w: c.w, h: c.h, rotationDegrees: c.rotationDegrees, activated: c.activated, objid: n(c.objid),
    slopeAngleDeg: c.slopeAngleDeg, slopeDir: c.slopeDir, slopeIsFilled: c.slopeIsFilled, slopeFlipY: c.slopeFlipY,
    hypoA: [c.hypoAx, c.hypoAy], hypoB: [c.hypoBx, c.hypoBy], rightA: [c.rightAx, c.rightAy], slopeSolidBelow: c.slopeSolidBelow,
    slopeType: n(c.slopeType), gdSolidAbove: !!c.gdSolidAbove, gdWallOnLeft: !!c.gdWallOnLeft, hitbox_radius: n(c.hitbox_radius),
    particleColor: n(c.particleColor), portalX: n(c.portalX), portalY: n(c.portalY), teleportTargetX: n(c.teleportTargetX),
    teleportTargetY: n(c.teleportTargetY), teleportYOffset: n(c.teleportYOffset), padId: n(c.padId), orbId: n(c.orbId),
    multiActivate: !!c.multiActivate, freeMode: !!c.freeMode, flipY: !!c.flipY, orbRotation: n(c.orbRotation), coinId: n(c.coinId), speedValue: n(c.speedValue), speedId: n(c.speedId),
    _baseX: c._baseX, _baseY: c._baseY, _baseRotationDegrees: c._baseRotationDegrees,
    cameraSettings: c.cameraSettings ? [c.cameraSettings.easing, c.cameraSettings.padding] : null,
    toggleGroup: n(c.toggleGroup), toggleActivate: n(c.toggleActivate), claimTouch: n(c.claimTouch), spawnOnly: n(c.spawnOnly),
    force: n(c.force), forceMin: n(c.forceMin), forceMax: n(c.forceMax), forceRadial: n(c.forceRadial), forceRange: n(c.forceRange), forceId: n(c.forceId), forceFlipY: n(c.forceFlipY),
  };
};

function jsColliders(level) {
  return level.objects.map((lo) => {
    const def = ctx.getObjectFromId(lo.id);
    if (parseInt(lo?.id ?? 0, 10) === 749 && !lo?._generatedTeleportExit) return null;
    if (parseInt(lo.id, 10) === 3643) {
      const self = { objects: [], _addCollisionToSection() {} };
      ctx.LevelObject.prototype._spawnToggleBlock.call(self, lo, 0);
      return self.objects.map(proj);
    }
    const bid = parseInt(lo?.id ?? 0, 10);
    if (def && def.type === ctx.triggerType && bid !== 1813 && bid !== 2069 && bid !== 3645) return null;
    if (!def) return null; // `if (objectDef)` guards the whole collision block
    const self = { objects: [], objectSprites: {}, _groupColliders: {}, _addCollisionToSection() {}, _getTeleportPortalYOffset: ctx.tpOffset,
      _secretCoinCompleted: new Set(), _userCoinCompleted: new Set(), _eeObjectId: 0 };
    ctx.spawnCol.call(self, lo, def, lo.x * 2, lo.y * 2, parseInt(lo.id ?? 0, 10), 0, 0, 0, null);
    return self.objects.map(proj);
  });
}

const { instance } = await WebAssembly.instantiate(fs.readFileSync(root + "target/wasm32-unknown-unknown/release/wd_core.wasm"), { env: { js_sin: Math.sin, js_cos: Math.cos, js_tan: Math.tan, js_acos: Math.acos, js_atan: Math.atan, js_atan2: Math.atan2, js_pow: Math.pow, js_random: () => globalThis.__random() } });
const w = instance.exports;
const rustColliders = (str) => {
  const bytes = new TextEncoder().encode(str);
  const p = w.wd_alloc(bytes.length);
  new Uint8Array(w.memory.buffer, p, bytes.length).set(bytes);
  const n = w.wd_level_colliders(p, bytes.length);
  return JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), n)));
};

const files = fs.readdirSync(root + "ref/assets/levels").filter((f) => f.endsWith(".txt")).sort();
let sseed = 99;
const srnd = () => ((sseed = (sseed * 1664525 + 1013904223) >>> 0) / 4294967296);
const synth = (n) => {
  const parts = ["kA4,0"];
  const ids = [3643, 3643, 2069, 3645, 1813, 2866, 1829, 1859, 10, 11, 12, 13, 47, 111, 660, 745, 2926];
  for (let i = 0; i < n; i++) {
    const id = ids[Math.floor(srnd() * ids.length)];
    const o = ["1", id, "2", Math.floor(srnd() * 400) * 0.5, "3", Math.floor(srnd() * 40) * 0.5];
    const add = (k, v) => o.push(String(k), String(v));
    if (srnd() < 0.4) add(6, Math.floor(srnd() * 4) * 90);
    if (srnd() < 0.3) add(32, (0.5 + srnd() * 2).toFixed(3));
    if (id === 3643) { add(99, srnd() < 0.5 ? 1 : 0); add(51, 1 + Math.floor(srnd() * 8)); add(56, srnd() < 0.5 ? 1 : 0); add(445, srnd() < 0.5 ? 1 : 0); add(504, srnd() < 0.5 ? 1 : 0); }
    if (id === 2069 || id === 3645) { add(149, (srnd() * 10 - 5).toFixed(2)); add(526, (srnd() * 3).toFixed(2)); add(527, (srnd() * 3).toFixed(2)); add(528, srnd() < 0.5 ? 1 : 0); add(529, srnd() < 0.5 ? 1 : 0); add(530, Math.floor(srnd() * 6)); if (srnd() < 0.5) add(5, 1); }
    if ([10, 11, 12, 13, 47, 111, 660, 745].includes(id) && srnd() < 0.7) { add(111, srnd() < 0.5 ? 1 : 0); add(112, srnd() < 0.7 ? 1 : 0); add(113, (srnd() * 60 - 10).toFixed(2)); add(114, (srnd() * 1.6 - 0.3).toFixed(2)); }
    parts.push(o.join(","));
  }
  return parts.join(";");
};

let objs = 0, cols = 0, bad = 0;
const byType = {};
for (const f of files) {
  const str = fs.readFileSync(root + "ref/assets/levels/" + f, "utf8");
  const lvl = ctx.parseLevel(str);
  const js = JSON.parse(JSON.stringify(jsColliders(lvl)));
  const rs = rustColliders(str);
  if (js.length !== rs.length) { console.log("LENGTH MISMATCH", f, js.length, rs.length); bad++; continue; }
  for (let i = 0; i < js.length; i++) {
    objs++;
    try { assert.deepStrictEqual(rs[i], js[i]); (js[i] || []).forEach((c) => { cols++; byType[c.type] = (byType[c.type] || 0) + 1; }); }
    catch (e) {
      if (bad++ < 6) console.log("MISMATCH", f, "object", i, "id", lvl.objects[i].id, "\n js:", JSON.stringify(js[i]).slice(0, 500), "\n rs:", JSON.stringify(rs[i]).slice(0, 500));
    }
  }
}
{
  const str = synth(600);
  const lvl = ctx.parseLevel(str);
  const js = JSON.parse(JSON.stringify(jsColliders(lvl)));
  const rs = rustColliders(str);
  let sb = 0;
  for (let i = 0; i < js.length; i++) { objs++; try { assert.deepStrictEqual(rs[i], js[i]); (js[i] || []).forEach((c) => { cols++; byType[c.type] = (byType[c.type] || 0) + 1; }); } catch (e) { if (sb++ < 3) console.log("SYNTH MISMATCH", i, lvl.objects[i].id, (js[i] || []).map((c, k) => [...new Set([...Object.keys(c), ...Object.keys(rs[i]?.[k] || {})])].filter((key) => JSON.stringify(c[key]) !== JSON.stringify(rs[i]?.[k]?.[key])).map((key) => `${key}: js=${JSON.stringify(c[key])} rs=${JSON.stringify(rs[i]?.[k]?.[key])}`)).join(" | ")); } }
  bad += sb;
}
console.log(byType);
console.log(`${files.length} levels, ${objs} objects, ${cols} colliders: ${bad ? bad + " MISMATCHED" : "all identical"}`);
process.exit(bad ? 1 : 0);

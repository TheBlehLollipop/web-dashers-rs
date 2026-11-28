// Differential test of `_setUpSettings` (level header: colour channels, blending, background, ground) on every shipped level.
import fs from "node:fs";
import { makeRecorder } from "./sprite-ref.mjs";
const root = new URL("..", import.meta.url).pathname;
const { H, ctx, scene } = await makeRecorder();
const w = H.w;
import vm from "node:vm";
vm.runInContext("window.LevelObject.prototype._initGenericTriggers = function () {};", ctx);
const files = fs.readdirSync(root + "ref/assets/levels").filter((f) => f.endsWith(".txt")).sort();
const enc = new TextEncoder();
let bad = 0;
const hsv = (h) => (h ? [h.h, h.s, h.v, h.sAdd, h.vAdd] : null);
for (const f of files) {
  const str = fs.readFileSync(root + "ref/assets/levels/" + f, "utf8");
  const lvl = ctx.parseLevel(str);
  const L = new ctx.LevelObject(scene, { _v: 0 });
  ctx.window.settingsMap = {};
  L._setUpSettings(lvl.settings);
  const js = {
    colors: Object.fromEntries(Object.entries(L._initialColors).map(([k, c]) => [k, [c.r, c.g, c.b]])),
    copies: Object.fromEntries(Object.entries(L._initialCopies).map(([k, c]) => [k, [c.source, hsv(c.hsv)]])),
    opacity: L._initialOpacity, blending: L._channelBlending, legacy: !!L._legacyColorLevel, bg: ctx.window._backgroundId ?? null, ground: ctx.window._groundId ?? null,
  };
  const b = enc.encode(str); const p = w.wd_alloc(b.length); new Uint8Array(w.memory.buffer, p, b.length).set(b);
  const n = w.wd_level_settings(p, b.length);
  const rs = JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), n)));
  const canon = (o) => JSON.stringify(JSON.parse(JSON.stringify(o, (k, v) => (typeof v === "number" && !Number.isFinite(v) ? null : v))), (k, v) => (v && typeof v === "object" && !Array.isArray(v) ? Object.fromEntries(Object.entries(v).sort()) : v));
  if (canon(js) !== canon(rs)) { bad++; console.log("MISMATCH", f, "\n js  ", canon(js).slice(0, 400), "\n rust", canon(rs).slice(0, 400)); }
}
console.log(`${files.length} levels: ${bad ? bad + " MISMATCHED" : "all identical"}`);
process.exit(bad ? 1 : 0);

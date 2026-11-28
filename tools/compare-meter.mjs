// Differential test of the music level meter: the original AudioManager.update (audio.js, run as-is on a stand-in analyser) vs audio.rs.
import fs from "node:fs";
import vm from "node:vm";
import { load } from "./physics-harness.mjs";
const root = new URL("..", import.meta.url).pathname;
const { ctx, w } = await load();
const src = fs.readFileSync(root + "ref/assets/scripts/core/audio.js", "utf8");
const a0 = src.indexOf("  update(timeSeconds) {"), a1 = src.indexOf("  getMeteringValue() {");
const a2 = src.indexOf("  reset() {");
vm.runInContext(`this.Meter = class Meter {
  constructor() { this._meterValue = 0.1; this._lastAudio = 0.1; this._lastPeak = 0; this._silenceCounter = 0; this._meteringEnabled = true; this._vol = 1; this._cur = 0; this._analyser = { getFloatTimeDomainData: (b) => { b.fill(0); b[3] = this._cur; } }; this._meterBuffer = new Float32Array(16); }
  _effectiveVolume() { return this._vol; }
  _stepTriggerSong() {}
${src.slice(a0, a1)}
${src.slice(a1, src.indexOf("  reset() {"))}
};`, ctx);
let seed = 5;
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const p = w.wd_alloc(32);
const m = () => new Float64Array(w.memory.buffer, p, 4);
let bad = 0, checks = 0;
for (let s = 0; s < 400; s++) {
  const js = new ctx.Meter();
  m().fill(0); w.wd_meter(0, p);
  let level = rnd();
  for (let k = 0; k < 600; k++) {
    const dt = [1 / 60, 1 / 60, 1 / 30, 1 / 120, 0.02][Math.floor(rnd() * 5)];
    const beat = rnd() < 0.08;
    level = beat ? rnd() : level * (0.6 + rnd() * 0.4);
    const vol = rnd() < 0.1 ? 0 : 0.2 + rnd() * 0.8;
    const peak = Math.fround(level * vol * (0.5 + rnd()));
    js._cur = peak; js._vol = vol; js.update(dt);
    m().set([dt, Math.abs(peak), vol, 0]); w.wd_meter(1, p);
    checks++;
    if (!Object.is(js.getMeteringValue(), m()[3])) { bad++; console.log(`MISMATCH seq ${s} step ${k}: js=${js.getMeteringValue()} rust=${m()[3]}`); if (bad > 5) process.exit(1); }
  }
}
console.log(`${checks} meter updates: ${bad ? bad + " mismatches" : "all identical"}`);
process.exit(bad ? 1 : 0);

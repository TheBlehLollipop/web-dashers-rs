// Differential test of the move / rotate / spawn / stop / toggle triggers: the original LevelObject methods (level.js, run as-is on a
// LevelObject prototype with the real trigger-building and collider-building code of `_spawnObject`) versus triggers.rs, frame by frame
// in the update order of game-scene.js, on every shipped level.
// usage: node tools/compare-triggers.mjs [seed] [levelIdx|-] [maxFramesPerLevel]
import fs from "node:fs";
import vm from "node:vm";
import assert from "node:assert";
import { load } from "./physics-harness.mjs";
import { makeRecorder } from "./sprite-ref.mjs";
import { makeFuzz, GENERIC_IDS } from "./fuzz-level.mjs";

const root = new URL("..", import.meta.url).pathname;
// REAL=1: the original LevelObject loads the level itself (real sprites, via a recording stand-in for Phaser) and the sprites are compared too
const REAL = !!process.env.REAL;
const REC = REAL ? await makeRecorder() : null;
const H = REAL ? REC.H : await load();
const { ctx, w, ptr, layout, colLayout, colRow } = H;
const putStr = (str) => { const b = new TextEncoder().encode(str); const p = w.wd_alloc(Math.max(1, b.length)); new Uint8Array(w.memory.buffer, p, b.length).set(b); return [p, b.length]; };
let seed = Number(process.argv[2] || 1);
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
// spawn triggers with a random delay draw random numbers: both sides get identical, separate streams
const stream = (seed0) => { let s = seed0 >>> 0; return () => ((s = (s * 1664525 + 1013904223) >>> 0) / 4294967296); };
ctx.__rnd = stream(12345); vm.runInContext("Math.random = __rnd", ctx);
globalThis.__random = stream(12345);
const ONLY_ARG = process.argv[3];
const ONLY = ONLY_ARG !== undefined && ONLY_ARG !== "-" && /^\d+$/.test(ONLY_ARG) ? Number(ONLY_ARG) : null;
const ONLY_LABEL = ONLY_ARG && /\.txt$/.test(ONLY_ARG) ? ONLY_ARG : null;
const MAXF = Number(process.argv[4] || 4000);

if (!REAL) {
  vm.runInContext("var fs = 1000; var gs = 1001;", ctx);
  vm.runInContext(fs.readFileSync(root + "ref/assets/scripts/core/triggers.js", "utf8") + "\nthis.Easing = Easing; this.ColorManager = ColorManager;", ctx);
}
// the trigger branch of `_spawnObject`, verbatim
const src = fs.readFileSync(root + "ref/assets/scripts/core/level.js", "utf8");
const t0 = src.indexOf("  if (objectDef && objectDef.type === triggerType && parseInt(levelObj.id, 10) !== J_BLOCK_OBJECT_ID");
const t1 = src.indexOf("\n  if (this._nextObjectId === undefined) {\n    this._nextObjectId = 0;\n  }\n  const linkedObjectId", t0 + 10);
assert(t0 > 0 && t1 > t0, "trigger slice markers not found");
vm.runInContext(`this.spawnTrig = function (levelObj) {\n const scene = this._scene; const objectDef = getObjectFromId(levelObj.id);\n ${src.slice(t0, t1)}\n};`, ctx);

if (REAL) {
  const enc = new TextEncoder();
  const put = (str) => { const b = enc.encode(str); const p = w.wd_alloc(Math.max(1, b.length)); new Uint8Array(w.memory.buffer, p, b.length).set(b); return [p, b.length]; };
  for (const f of fs.readdirSync(root + "ref/assets/sheets").filter((n) => n.endsWith(".json") && !/AnimDesc/.test(n))) {
    const [np, nl] = put(f.replace(/\.json$/, "")); const [jp, jl] = put(fs.readFileSync(root + "ref/assets/sheets/" + f, "utf8"));
    w.wd_atlas_add(np, nl, jp, jl);
  }
}
const gsrc = REAL ? fs.readFileSync(root + "ref/assets/scripts/core/game-scene.js", "utf8") : "";
if (vm.runInContext("typeof screenHeight", ctx) === "undefined") vm.runInContext("var screenHeight = 640;", ctx);
const NOOP_GFX = new Proxy(function () {}, { get: (t, k) => (k === Symbol.toPrimitive || k === "then" ? undefined : NOOP_GFX), apply: () => NOOP_GFX });
const camSrc = fs.readFileSync(root + "ref/assets/scripts/core/game-scene.js", "utf8");
const camS0 = camSrc.indexOf("  _applyCameraEdges() {"), camU = camSrc.indexOf("  _updateCameraY(", camS0);
const camS1 = camSrc.slice(camU + 3).search(/\n  [_a-zA-Z][A-Za-z0-9_]*\([^)]*\) \{ *\n/) + camU + 3 + 1;
const CamCls = vm.runInContext(`(class C {\n${camSrc.slice(camS0, camS1)}\n})`, ctx);
const camB0 = camSrc.indexOf("      if (this._followCameraY !== undefined && this._level.staticCamera) this._cameraY = this._followCameraY;");
const camB1 = camSrc.indexOf("      this._applyCameraEdges();\n", camB0) + "      this._applyCameraEdges();\n".length;
const camBlock = vm.runInContext(`(function (quantizedDelta) {\n${camSrc.slice(camB0, camB1)}\n})`, ctx);
let animBlock = null, AnimCls = null;
if (REAL) {
  const s0 = gsrc.indexOf("  _stepObjectAnimations(deltaTime) {"), s1 = gsrc.indexOf("  _stepBeastAnimations(deltaTime) {");
  AnimCls = vm.runInContext(`(class A {\n${gsrc.slice(s0, s1)}\n})`, ctx);
  const a0 = gsrc.indexOf("    this._stepObjectAnimations(deltaTime);\n    if (this._level && this._level._orbSprites) {");
  const a1 = gsrc.indexOf("    this._level.updateAudioScale(this._audio.getMeteringValue(), this._cameraX, screenWidth);", a0);
  const a2 = gsrc.indexOf("\n", a1);
  animBlock = vm.runInContext(`(function (deltaTime) {\n${gsrc.slice(a0, a2)}\n})`, ctx);
}
const REAL_GENERIC_IDS = GENERIC_IDS.filter((id) => ![2067, 3662, 1913, 2015].includes(id));
const COINS = !!process.env.COINS;
const lsStore = new Map();
if (COINS) ctx.localStorage = { getItem: (k) => (lsStore.has(k) ? lsStore.get(k) : null), setItem: (k, v) => lsStore.set(k, String(v)) };
const MIRROR = !!process.env.MIRROR;
let MirrorCls = null;
if (REAL && MIRROR) {
  const m0 = gsrc.indexOf("  _mirrorProgress() {"), m1 = gsrc.indexOf("_labelTextCurrent(label, key) {");
  MirrorCls = vm.runInContext(`(class M {\n${gsrc.slice(m0, m1)}\n})`, ctx);
}
const animLog = [];
let coinSeed = null;
const crnd = stream(31337);
ctx.window.BeastAnim = { IDS: [], commandMany: (states, id) => { animLog.push(id); }, defaultBase() {}, command() {} };
// BEAST=1: the real BeastAnim (animations.js) and descriptions on both sides
const BEAST = REAL && !!process.env.BEAST;
if (BEAST) {
  for (let k = 1; k <= 5; k++) {
    const name = `GJBeast0${k}`, src = fs.readFileSync(root + `ref/assets/sheets/${name}_AnimDesc.json`, "utf8");
    ctx[`${name}_AnimDesc`] = JSON.parse(src);
    const [np, nl] = putStr(name), [jp, jl] = putStr(src);
    if (!w.wd_beast_desc_add(np, nl, jp, jl)) throw new Error("desc " + name);
  }
  vm.runInContext(fs.readFileSync(root + "ref/assets/scripts/core/animations.js", "utf8"), ctx);
  const real = ctx.window.BeastAnim, cm = real.commandMany;
  real.commandMany = (states, id) => { animLog.push(id); cm(states, id); };
}
const files = fs.readdirSync(root + "ref/assets/levels").filter((f) => f.endsWith(".txt")).sort();
const same = (a, b) => Object.is(a, b) || a === b || (Number.isNaN(a) && Number.isNaN(b));
const canon = (v) => JSON.stringify(v, (k, x) => (typeof x === "number" && Object.is(x, -0) ? 0 : x && typeof x === "object" && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) : x));
const text = (n) => new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), n));
const rjson = (n) => JSON.parse(text(n));
const LISTS = { move: "_moveTriggers", rotate: "_rotateTriggers", spawn: "_spawnTriggers", stop: "_stopTriggers", toggle: "_toggleTriggers", followY: "_followPlayerYTriggers", follow: "_followTriggers", onDeath: "_onDeathTriggers", touch: "_touchTriggers", pickup: "_pickupTriggers", count: "_countTriggers", instantCount: "_instantCountTriggers", collision: "_collisionTriggers", blocks: "_collisionBlocks", color: "_colorTriggers", pulse: "_pulseTriggers", alpha: "_alphaTriggers", shake: "_shakeTriggers", playerVis: "_playerVisTriggers", animation: "_animationTriggers", enterEffect: "_enterEffectTriggers" };
const OTHER = [];

function newLevel() {
  animLog.length = 0;
  if (REAL) {
    REC.scene.add = REC.scene.add || {}; if (!REC.scene.add.graphics) REC.scene.add.graphics = () => NOOP_GFX; REC.scene._pcCount = 0; REC.scene._applyPlayerControlTrigger = function () { this._pcCount++; }; REC.scene._state = { y: 0 }; REC.scene._cameraY = 0; REC.scene._playerWorldX = 0; REC.scene.time = { now: 0, delayedCall() {} };
    const L = new ctx.LevelObject(REC.scene, { _v: 0 });
    L.resetAll = () => {
      L.resetColorTriggers(); L.resetAlphaTriggers(); L.resetRotateTriggers(); L.resetPulseTriggers(); L.resetEnterEffectTriggers(); L.resetSpawnTriggers(); L.resetMoveTriggers(); L.resetStopTriggers(); L.resetFollowPlayerYTriggers(); L.resetFollowTriggers(); L.resetToggleTriggers(); L.resetAnimationTriggers(); L.resetShakeTriggers(); L.resetChannelBlending(); L.resetPlayerVisibilityTriggers(); L.resetTimeWarpTriggers(); L.resetOnDeathTriggers(); L.resetTouchTriggers(); L.resetPickupTriggers(); L.resetCountTriggers(); L.resetCollisionTriggers(); L.resetInstantCountTriggers(); L.resetGenericTriggers();
      for (const s of L._audioScaleSprites) ctx.gdApplyEffectScale(s, 0.1);
    };
    return L;
  }
  const L = Object.create(ctx.LevelObject.prototype);
  Object.assign(L, {
    objects: [], objectSprites: [], _groupColliders: {}, _groupSprites: {}, _collisionSections: [], _nearbyBuffer: [], _nextObjectId: 0, _startPositions: [],
    _secretCoinCompleted: new Set(), _userCoinCompleted: new Set(), _secretCoinSlotCount: 0, _userCoinSlotCount: 0,
    _spawnTextObject() {}, _spawnTriggerEditorVisual() {}, _getSectionIndexForWorldX: () => 0, _refreshSpriteSection() {},
    _scene: { _playerWorldX: 0, _state: { y: 0 }, _cameraY: 0, time: { now: 0 }, _pcCount: 0, _applyPlayerControlTrigger() { this._pcCount++; }, add: { graphics: () => NOOP_GFX } }, _cameraXRef: { _v: 0 }, _channelBlending: { 1007: true }, _colorChannelSprites: {}, _activePulses: [],
    _timeWarpTriggers: [], _activeAlphaTweens: [], _getGlowAlphaMultiplier: () => 0.425, _groupOpacity: {}, _sections: [], _beastSprites: [], _touchPlayerVisActivated: new Set(), shakeOffsetX: 0, shakeOffsetY: 0, _shakeState: null,
  });
  for (const k of [...Object.values(LISTS), ...OTHER]) L[k] = [];
  L.freeModeCamera = false; L.freeCamEasing = 10; L.freeCamPadding = 0.5; L._triggerGroupsByUid = new Map(); L._spawnFiredUids = new Set(); L._genericTriggers = []; L._activeEnterEffect = 0;
  ctx.LevelObject.prototype._initGenericTriggers.call(L);
  L._getTriggerFrameName = () => null;
  { const orig = L._remapTrigger; L._remapTrigger = function (t, r) { const c = orig.call(this, t, r); if (c !== t) { const g = (globalThis.__gen ||= {}); g.remapCopies = (g.remapCopies || 0) + 1; } return c; }; }
  L.resetAll = () => {
    L.resetGenericTriggers();
    L.resetTimeWarpTriggers(); L.resetAlphaTriggers(); L.resetShakeTriggers(); L.resetPlayerVisibilityTriggers(); L.resetAnimationTriggers(); L.resetEnterEffectTriggers(); L._colorTriggerIdx = 0; L._pulseTriggerIdx = 0; L._activePulses = []; L._touchColorTriggerActivated = new Set(); L._channelBlending = { 1007: true };
    L._moveTriggerIdx = L._rotateTriggerIdx = L._spawnTriggerIdx = L._stopTriggerIdx = L._toggleTriggerIdx = L._followPlayerYTriggerIdx = L._followTriggerIdx = 0;
    L._onDeathTriggerIdx = L._touchTriggerIdx = L._pickupTriggerIdx = L._countTriggerIdx = L._instantCountTriggerIdx = L._collisionTriggerIdx = 0;
    L._armedOnDeathTriggers = []; L._activeTouchTriggers = []; L._activeCountTriggers = []; L._activeCollisionTriggers = []; L._itemCounts = {};
    L._touchOnDeathTriggerActivated = new Set(); L._touchTouchTriggerActivated = new Set(); L._touchPickupTriggerActivated = new Set(); L._touchCountTriggerActivated = new Set(); L._touchInstantCountTriggerActivated = new Set();
    L._activeFollowPlayerYTweens = []; L._activeFollowTweens = []; L._touchFollowPlayerYTriggerActivated = new Set(); L._touchFollowTriggerActivated = new Set();
    L._activeMoveTweens = []; L._activeRotateTweens = []; L._activeSpawnDelays = []; L._spawnFiredUids = new Set();
    L._groupOffsets = {}; L._groupRotations = {}; L._groupToggledOff = {};
    L._touchMoveTriggerActivated = new Set(); L._touchSpawnTriggerActivated = new Set(); L._touchStopTriggerActivated = new Set(); L._touchToggleTriggerActivated = new Set();
  };
  L.resetAll();
  return L;
}

const fuzzLevel = makeFuzz(rnd, { beast: !!process.env.BEAST, text: !!process.env.TEXT, generic: !process.env.NOGENERIC, spawny: !!process.env.SPAWNY, genericIds: REAL ? REAL_GENERIC_IDS : undefined });
const FUZZ = Number(process.env.FUZZ || 0);
const sources = FUZZ ? Array.from({ length: FUZZ }, (_, i) => ({ label: `fuzz#${i}`, str: fuzzLevel() })) : files.map((f) => { const raw = fs.readFileSync(root + "ref/assets/levels/" + f, "utf8"); return { label: f, str: raw.includes("\0") ? raw.replace(/\0/g, "").trim() : raw }; });

const UNPORTED = new Set(["changeBackground", "changeGround", "shader", "teleport", "song", "sfx", "editSong", "editSfx", "area", "bgSpeed", "mgSpeed", "advFollow", "gradient", "keyframeAnim"]);
const GENERIC_COMMON = ["uid", "spawnTriggered", "multiTrigger", "triggerChannel", "ord", "groups", "kind", "x", "y", "touchTriggered", "targetGroup", "centerGroup"];
const strip = (t) => JSON.parse(JSON.stringify(t, (k, v) => (v instanceof Map ? Object.fromEntries(v) : v)));
let bad = 0, framesTotal = 0, levelsDone = 0, checks = 0;
const report = (msg) => { if (bad++ < 12) console.log(msg); };

for (let li = 0; li < sources.length; li++) {
  if (ONLY !== null && li !== ONLY) continue;
  if (ONLY_LABEL && sources[li].label !== ONLY_LABEL) continue;
  seed = (Number(process.argv[2] || 1) * 2654435761 + li * 40503 + 17) >>> 0;
  const str = sources[li].str;
  const lvl = ctx.parseLevel(str);
  ctx.window.enableLDM = true; // object spawning in this harness uses the low-detail collider path
  const L = newLevel();
  if (!REAL) { L._fireGradientTrigger = () => {}; L.updateGradients = () => {}; L._destroyGradient = () => {}; }
  const sprites = []; // [x, y, groups]
  let realImgs = null;
  if (REAL) {
    ctx.window.enableLDM = false; ctx.window.isEditor = false; ctx.window.settingsMap = {};
    const official = !/^\d{5,}\.txt$/.test(sources[li].label); // the numbered files are online levels
    ctx.window.currentlevel = ["x", "X", "the_level", []]; ctx.window.allLevels = official ? [ctx.window.currentlevel] : [];
    w.wd_set_official(official ? 1 : 0);
    ctx.__rnd = stream(12345); vm.runInContext("Math.random = __rnd", ctx);
    REC.created.length = 0; ctx.window._animTimer = 0;
    if (COINS) {
      // saved coin progress: load once to learn the linked ids, then keep a random part of them
      lsStore.clear();
      const T = newLevel(); T.loadLevel(str);
      const pick = (ids) => ids.filter(() => crnd() < 0.5).map(String);
      const sIds = pick(T._coinSprites.filter((s) => s._secretCoinId !== undefined).map((s) => s._secretCoinId)), uIds = pick(T._coinSprites.filter((s) => s._userCoinId !== undefined).map((s) => s._userCoinId));
      if (sIds.length) lsStore.set("gd_secretCoins_the_level", JSON.stringify(sIds));
      if (uIds.length) lsStore.set("gd_userCoins_the_level", JSON.stringify(uIds));
      coinSeed = { official, secret: sIds, user: uIds, totalS: Math.floor(crnd() * 5), totalU: Math.floor(crnd() * 5) };
      lsStore.set("gd_totalsecretcoins", String(coinSeed.totalS)); lsStore.set("gd_totalusercoins", String(coinSeed.totalU));
      REC.created.length = 0; ctx.window._animTimer = 0;
      ctx.__rnd = stream(12345); vm.runInContext("Math.random = __rnd", ctx);
    }
    L.loadLevel(str);
    if (COINS) { L._scene._totalsecretcoins = coinSeed.totalS; L._scene._totalusercoins = coinSeed.totalU; }
    const padEmitter = (c) => c.kind === "particles" && c.args?.[3]?.lifespan?.min === 250 && c.args?.[3]?.lifespan?.max === 450 && c.args?.[3]?.frequency === 24;
    realImgs = REC.created.filter((c) => (c.kind === "image" && !c._eeEditorTrigger && !String(c.frameName).startsWith("edit_")) || padEmitter(c) || (c.kind === "text" && c._eeTextObject));
    ctx.__rnd = stream(777 + li); vm.runInContext("Math.random = __rnd", ctx); globalThis.__random = stream(777 + li);
  } else {
    const objs = ctx.LevelObject.prototype._normalizeTeleportPortals.call(L, lvl.objects);
    for (const lo of objs) {
      const def = ctx.getObjectFromId(lo.id);
      if (def && def.type === ctx.triggerType && ![1813, 2069, 3645].includes(Number(lo.id))) { ctx.spawnTrig.call(L, lo); continue; }
      const linked = L._nextObjectId++;
      lo._eeObjectId = linked;
      const objectId = parseInt(lo.id ?? 0, 10);
      const secretSlot = objectId === 142 ? L._secretCoinSlotCount++ : null;
      const userSlot = objectId === 1329 ? L._userCoinSlotCount++ : null;
      if (def) ctx.spawnCol.call(L, lo, def, lo.x * 2, lo.y * 2, objectId, linked, secretSlot, userSlot, null);
      if (def && lo.groups) {
        const gids = [...new Set(lo.groups.split(".").map(Number).filter((n) => n > 0))];
        if (gids.length) {
          const n = 1 + (objectId % 3);
          for (let k = 0; k < n; k++) {
            const x = lo.x * 2 + 10 * k, y = 460 - lo.y * 2 + 7 * k;
            const spr = { active: true, _eeGroups: gids, _eeInitialWorldX: x, _eeInitialBaseY: y, _origWorldX: x, _origBaseY: y, x, y, rotation: 0, scaleX: 1, scaleY: 1, setVisible() {}, setScale(sx, sy = sx) { this.scaleX = sx; this.scaleY = sy; } };
            for (const g of gids) (L._groupSprites[g] ||= []).push(spr);
            sprites.push(x, y, gids.length, ...gids);
          }
        }
      }
    }
    for (const k of OTHER) L[k] = []; // the kinds that are not ported
    for (const [kind, k] of Object.entries(LISTS)) if (kind !== "blocks" && kind !== "spawn") L[k].sort((a, b) => a.x - b.x);

  }
  ctx.window.mainColor = 327424; ctx.window.secondaryColor = 64511;
  const cm = new ctx.ColorManager();
  if (!REAL) { ctx.window.settingsMap = {}; L._setUpSettings(lvl.settings); }
  for (const ch in L._initialColors) cm.setInitialColor(parseInt(ch, 10), L._initialColors[ch]);
  for (const ch in L._initialCopies) cm.setInitialCopy(parseInt(ch, 10), L._initialCopies[ch]);
  for (const ch in L._initialOpacity) cm.setInitialOpacity(parseInt(ch, 10), L._initialOpacity[ch]);
  L._scene._colorManager = cm; if (!REAL) L._channelBlendingInitial = { ...L._channelBlending };
  const applyCT = (c) => { cm.triggerColor(c.index, c.color, c.duration, c.copy, c.opacity); L.setChannelBlending?.(c.index, !!c.blending); if (c.tintGround) cm.triggerColor(1001, c.color, c.duration, c.copy); };
  const cmChannels = [...new Set([1, 2, 3, 4, 1000, 1001, 1002, 1003, 1004, 1005, 1006, 1007, 1009, ...L._colorTriggers.map((c) => c.index), ...L._pulseTriggers.map((p) => p.targetChannel || 1)])];
  const cmBuf = w.wd_alloc(5 * 8);
  ctx.window.enableLDM = false; // from here on the game plays (pulse triggers are off in low-detail mode)
  const cols = L.objects.filter((c) => c instanceof ctx.Collider);
  cols.forEach((c, i) => (c.uid = i));

  // ---- Rust ----
  const bytes = new TextEncoder().encode(str);
  const sp = w.wd_alloc(bytes.length);
  new Uint8Array(w.memory.buffer, sp, bytes.length).set(bytes);
  if (REAL) w.wd_visual_seed(12345);
  if (COINS) { const [kp, kl] = putStr("the_level"), [sp2, sl] = putStr(coinSeed.secret.join(",")), [up, ul] = putStr(coinSeed.user.join(",")); w.wd_coins_config(coinSeed.official ? 1 : 0, 0, 0, kp, kl, sp2, sl, up, ul, coinSeed.totalS, coinSeed.totalU); }
  const n = w.wd_game_load_level(sp, bytes.length);
  w.wd_set_rotate_gameplay(typeof L._scene?._rotateGameplay === "function" ? 1 : 0);
  if (REAL) w.wd_game_after_load();
  const aptr = w.wd_alloc(Math.max(8, sprites.length * 8));
  new Float64Array(w.memory.buffer, aptr, Math.max(1, sprites.length)).set(sprites);
  if (!REAL) w.wd_trig_set_anchors(aptr, sprites.length);
  const rowLen = colLayout.length;
  const cbuf = w.wd_alloc(Math.max(1, n * rowLen) * 8);
  const lbuf = w.wd_alloc(Math.max(1, layout.length) * 8);
  const rustRows = () => { w.wd_game_get_state(lbuf, cbuf); return new Float64Array(w.memory.buffer, cbuf, Math.max(1, n * rowLen)); };
  let levelBad = false, skipped = false;
  const seen = { disabled: 0, moved: 0, rotated: 0, resorted: 0 };
  const fail = (msg) => { report(`MISMATCH ${sources[li].label}: ${msg}`); levelBad = true; };

  if (n !== cols.length) { fail(`collider count rust=${n} js=${cols.length}`); continue; }
  const rows0 = rustRows();
  outer: for (let i = 0; i < cols.length; i++) {
    const row = colRow(cols[i]);
    for (let j = 0; j < rowLen; j++) if (!same(row[j], rows0[i * rowLen + j])) { fail(`collider ${i} (${cols[i].type}) ${colLayout[j].name} rust=${rows0[i * rowLen + j]} js=${row[j]}`); break outer; }
  }
  const jsLists = Object.fromEntries(Object.entries(LISTS).map(([k, v]) => [k, strip(L[v])]));
  L._timeWarpTriggers.sort((a, b) => a.x - b.x);
  jsLists.timeWarp = strip(L._timeWarpTriggers.map((t) => ({ uid: t.uid, spawnTriggered: t.spawnTriggered, groups: t.groups, x: t.x, y: t.y, touchTriggered: t.touchTriggered, value: Number(t.raw?.[120] ?? 1) })));
  jsLists.generic = strip([...L._genericTriggers].map((t) => (UNPORTED.has(t.kind) ? Object.fromEntries(GENERIC_COMMON.filter((k) => k in t).map((k) => [k, t[k]])) : t)));
  try { assert.deepStrictEqual(JSON.parse(canon(rjson(w.wd_trig_lists()))), JSON.parse(canon(jsLists))); } catch (e) { fail("trigger lists differ: " + String(e.message).split("\n").slice(0, 60).join(" | ")); }
  if (levelBad) continue;

  // REAL: sprite state, group by group and frame by frame
  let spriteCheck = () => null, groupSpriteIdx = [];
  if (REAL) {
    const rrows = () => JSON.parse(text(w.wd_fx_sprites()));
    const first = rrows();
    if (first.length !== realImgs.length) { fail(`sprite count rust=${first.length} js=${realImgs.length}`); continue; }
    const idxOf = new Map(realImgs.map((c, i) => [c, i]));
    const jsGroups = {};
    for (const g of Object.keys(L._groupSprites)) jsGroups[g] = L._groupSprites[g].filter((c) => idxOf.has(c._raw)).map((c) => idxOf.get(c._raw));
    const rg = JSON.parse(text(w.wd_fx_groups()));
    for (const k of Object.keys(jsGroups)) if (!jsGroups[k].length) delete jsGroups[k]; // groups with only text objects (not ported yet)
    if (canon(jsGroups) !== canon(rg)) {
      const ks = new Set([...Object.keys(jsGroups), ...Object.keys(rg)]);
      for (const k of ks) if (canon(jsGroups[k]) !== canon(rg[k])) { fail(`group ${k} sprites: js=${canon(jsGroups[k])?.slice(0, 150)} rust=${canon(rg[k])?.slice(0, 150)}`); break; }
      continue;
    }
    groupSpriteIdx = [...new Set(Object.values(jsGroups).flat())].sort((a, b) => a - b);
    const nz = (v) => (Object.is(v, -0) ? 0 : v);
    const frameTable = JSON.parse(text(w.wd_render_frames()));
    const fields = (c) => [c.x, c.y, c.rotation, c.scaleX, c.scaleY, c.alpha, c.visible, c.active !== false, c.tintv === undefined ? null : c.tintv, c.blend ?? 0, c.kind === "image" ? c.frameName : c.kind === "text" ? `${c.args[4]}|${c.args[3]}` : "__pad_emitter__"];
    const names = ["x", "y", "rotation", "scaleX", "scaleY", "alpha", "visible", "active", "tint", "blend", "frame"];
    spriteCheck = (all) => {
      const rows = rrows();
      const list = all ? realImgs.map((_, i) => i) : groupSpriteIdx;
      for (const i of list) {
        const c = realImgs[i], r = rows[i], f = fields(c);
        for (let k = 0; k < 11; k++) {
          if (names[k] === "tint" && !(c._eeSectionIndex >= L._visMinSec && c._eeSectionIndex <= L._visMaxSec)) continue; // only what is on screen is kept up to date
          if (names[k] === "tint") { globalThis.__tintChecks = (globalThis.__tintChecks || 0) + 1; if (f[k] !== null) globalThis.__tintSet = (globalThis.__tintSet || 0) + 1; if (c._eePulsed) globalThis.__tintPulsed = (globalThis.__tintPulsed || 0) + 1; }
          const a = typeof f[k] === "number" ? nz(f[k]) : f[k], b = names[k] === "frame" ? frameTable[r[10]][1] : (typeof r[k] === "number" ? nz(r[k]) : r[k]);
          if (!(a === b || (typeof a === "number" && typeof b === "number" && Number.isNaN(a) && b === null) || (a === null && b === null) || (Number.isNaN(a) && Number.isNaN(b)))) return `sprite ${i} (${c.frameName}) ${names[k]}: js=${a} rust=${b}` + (process.env.DBG ? ` | js orb=${JSON.stringify({ inOrbs: L._orbSprites.some((m) => m._raw === c), active: c.active, wx: c._eeWorldX, guide: c._eeOrbGuide, baseScale: c._eeBaseScale, last: c._eeLastOrbScale, cam: L._cameraXRef._v })} js memb=${JSON.stringify(Object.entries(L._groupSprites).filter(([g, l]) => l.some((m) => m._raw === c)).map(([g]) => g))} js em=${JSON.stringify({ alpha: c.alpha, sec: c._eeSectionIndex, eeActive: c._eeActive, wx: c._eeWorldX, x: c.x, groups: c._eeGroups, dontFade: c._eeDontFade, cam: L._cameraXRef._v })} js centers=${JSON.stringify(Object.entries(L._groupRotations).map(([g, r]) => { const cg = r.centerGroupId; const jc = cg > 0 ? L._getGroupCenter(cg) : null; const out = w.wd_alloc(16); if (cg > 0) w.wd_fx_center(BigInt(cg), out); const rc = new Float64Array(w.memory.buffer, out, 2); const members = (L._groupSprites[cg] || []).map((m) => `${m.kind}:${m.active}:${m._eeInitialWorldX ?? m._eeWorldX ?? m.x},${m._eeInitialBaseY ?? m._eeBaseY ?? m.y}`); return { g, cg, js: jc, rust: [rc[0], rc[1]], members: members.slice(0, 8), n: members.length }; }))} js alphaDbg=${JSON.stringify({ groupOpacity: L._groupOpacity, chOp: c._eeColorChannel ? cm.getOpacity(c._eeColorChannel) : null, force: L._alphaForceApply, applied: c._eeChannelAlphaApplied, glowMult: L._getGlowAlphaMultiplier(), calc: L._getGroupOpacityForSprite(c), approach: c._eeApproachAlpha, eeActive: c._eeActive })} js dbg=${(() => { const before = c.tintv; const hid = L._isSpriteHidden(c); L._applyChannelTint(c, String(c._eeColorChannel), cm.getHex(c._eeColorChannel), cm); const r = { hidden: hid, stamp: c._eeTintStamp, idxStamp: L._colorIndexStamp, before, after: c.tintv, inSection: (L._sections[c._eeSectionIndex] || []).includes(c), parent: !!c.parentContainer, hex: cm.getHex(c._eeColorChannel) }; c.tintv = before; return JSON.stringify(r); })()} js flags=${JSON.stringify({ black: c._isBlack, can: c._canColor, bd: c._blackDefault, cant: c._cantColor, ch: c._eeColorChannel, tch: c._eeTintChannels, pulsed: c._eePulsed, saw: c._isSaw, sc: c._SawColor, audio: c._eeAudioScale, sec: c._eeSectionIndex, vmin: L._visMinSec, vmax: L._visMaxSec, active: c.active })} vis=${c.visible} active=${c.active} toggled=${c._eeToggledOffGroups ? [...c._eeToggledOffGroups] : null} rot=${c.rotation} jsRotations=${JSON.stringify(Object.fromEntries((c._eeGroups || []).map((g) => [g, L._groupRotations[g]])))} initY=${c._eeInitialBaseY} baseY=${c._eeBaseY} visOffY=${c._eeVisOffY} groups=${c._eeGroups} rustRow=${JSON.stringify(rows[i])} rustSprite=${(() => { const l = w.wd_fx_sprite(i); return text(l); })()} offsets=${JSON.stringify(L._groupOffsets)} | js approach=${c._eeApproachAlpha} origAlpha=${c._eeOrigAlpha} glow=${c._eeIsGlowSprite} entry=${JSON.stringify(c._eeInvisibleBlockEntry && { x: c._eeInvisibleBlockEntry.x, a: c._eeInvisibleBlockEntry.alpha, rem: c._eeInvisibleBlockEntry.removed })} px=${L._scene._playerWorldX} jsEntries=${JSON.stringify(L._invisibleBlockEntries.map((e) => [e.x, e.sprites.length, e.alpha]))} rustEntries=${text(w.wd_fx_invisible())}` : "");
        }
      }
      return null;
    };
  }

  // COINS=1: coin pickups, commits to localStorage, restarts and the coin sprite looks, versus coins.rs
  const rstore = new Map(lsStore);
  const coinCols = REAL && COINS ? cols.filter((c) => c.secretCoinId !== undefined || c.userCoinId !== undefined) : [];
  const coinKeys = (m) => [...m.keys()].filter((k) => /^gd_(secretCoins|userCoins|totalsecretcoins|totalusercoins)/.test(k)).sort().map((k) => [k, m.get(k)]);
  let coinOpCount = 0;
  const coinOps = () => {
    if (!coinCols.length && crnd() < 0.8) return null;
    const r = crnd();
    if (r < 0.55 && coinCols.length) {
      const c = coinCols[Math.floor(crnd() * coinCols.length)];
      let ok = !c.activated;
      if (ok && c.secretCoinId !== undefined && !c._secretCoinCompleted && !L.tryCollectSecretCoin(c)) ok = false;
      if (ok && c.userCoinId !== undefined && !c._userCoinCompleted && !L.tryCollectUserCoin(c)) ok = false;
      if (ok) c.activated = true;
      const rr = w.wd_coins_touch(c.uid);
      if (!!rr !== ok) return `touch coin ${c.uid}: js=${ok} rust=${!!rr}`;
    } else if (r < 0.8) {
      L.commitSecretCoins(); L.commitUserCoins();
      w.wd_coins_commit();
      for (const [k, v] of JSON.parse(text(w.wd_coins_outbox()))) rstore.set(k, v);
      const a = JSON.stringify(coinKeys(lsStore)), b = JSON.stringify(coinKeys(rstore));
      if (a !== b) return `storage after commit: js=${a} rust=${b}`;
    } else {
      for (const c of L.objects) if (c) c.activated = false;
      L._secretCoinRunCollected.clear(); L._userCoinRunCollected.clear();
      for (const cs of L._coinSprites) { if (cs) { cs.setVisible(true); cs.setAlpha(1); cs.setScale(cs._coinBaseScale || 1); if (cs._coinWorldY !== undefined) cs.y = 460 - cs._coinWorldY; } }
      w.wd_game_reset_objects();
    }
    coinOpCount++;
    const rs = JSON.parse(text(w.wd_coins_state()));
    const toN = (set) => [...set].map(Number);
    const js = { secretRun: toN(L._secretCoinRunCollected), userRun: toN(L._userCoinRunCollected), secretDone: toN(L._secretCoinCompleted), userDone: toN(L._userCoinCompleted) };
    if (canon(js) !== canon(rs)) return `coin sets js=${canon(js)} rust=${canon(rs)}`;
    return spriteCheck(true);
  };
  // MIRROR=1: the scene's mirror portal code (toggleMirror, _stepMirror, _applyMirrorEffect) on the real sprites versus mirror.rs
  const mrnd = stream(4242 + li);
  const mgs = REAL && MIRROR ? Object.assign(Object.create(MirrorCls.prototype), { _level: L, _cameraX: 0, _mirrorAnimating: false, _mirrorP: 0, _mirrorTarget: false, _mirrorFrom: 0, _mirrorTime: 0, _state: { mirrored: false }, _state2: { mirrored: false }, _bg: { flipX: false, setFlipX() {} }, events: { once() {} }, _mirrorSaved: [] }) : null;
  if (mgs) w.wd_mirror_enable(1);
  let mirrorSeen = 0;
  const mirrorCheck = () => {
    // random portal touches and animation steps (the same calls on both sides)
    if (mrnd() < 0.02) { const on = mrnd() < 0.5; const a = mgs.toggleMirror(on), b = w.wd_mirror_toggle(on ? 1 : 0); if (a !== !!b) return `toggleMirror(${on}) js=${a} rust=${b}`; }
    if (mrnd() < 0.7) { const dt = [1 / 60, 1 / 30, 0.05, 0.4, 0.2][Math.floor(mrnd() * 5)]; mgs._stepMirror(dt); w.wd_mirror_step(dt); }
    const rs = JSON.parse(text(w.wd_mirror_state()));
    const js = [mgs._mirrorAnimating ? 1 : 0, mgs._mirrorTarget ? 1 : 0, mgs._mirrorFrom, mgs._mirrorP, mgs._mirrorTime, mgs._state.mirrored ? 1 : 0, mgs._state2.mirrored ? 1 : 0];
    // (target/from/time are only meaningful once a toggle happened)
    for (let k = 0; k < 7; k++) { if ((k === 1 || k === 2 || k === 4) && !mgs._mirrorEverToggled && js[0] === 0) continue; if (!same(js[k], rs[k]) && !(js[0] === 0 && (k === 2 || k === 4))) return `state[${k}] js=${js[k]} rust=${rs[k]} (js ${JSON.stringify(js)} rust ${JSON.stringify(rs)})`; }
    const pj = mgs._mirrorProgress(); w.wd_mirror_cam(cx);
    if (!same(pj, w.wd_mirror_progress())) return `progress js=${pj} rust=${w.wd_mirror_progress()}`;
    if (pj <= 0) return null;
    mirrorSeen++;
    mgs._cameraX = cx;
    L.additiveContainer.x = L.container.x = L.topContainer.x = -cx + (L.shakeOffsetX || 0);
    for (const c of [L.additiveContainer, L.container, L.topContainer]) c.scaleX = 1;
    mgs._applyMirrorEffect();
    let bad = null;
    const srows = JSON.parse(text(w.wd_fx_sprites()));
    const sk = L.shakeOffsetX || 0;
    for (let i = 0; i < realImgs.length && !bad; i++) {
      const c = realImgs[i];
      if (c.kind !== "image") continue;
      const top = c._eeLayer === 2;
      const inRange = top || (c._eeSectionIndex >= L._visMinSec && L._visMinSec >= 0 && c._eeSectionIndex <= L._visMaxSec);
      if (!inRange) continue; // only what is on screen is drawn
      const v = JSON.parse(text(w.wd_mirror_view(i)));
      const rustVisible = !!srows[i][6] && v[4] === 1;
      const jsVisible = !!c.visible;
      if (rustVisible !== jsVisible) { bad = `sprite ${i} (${c.frameName}) visible js=${jsVisible} rust=${rustVisible} (p=${pj}, x=${c.x}, layer=${c._eeLayer}, dontFade=${c._eeDontFade}, worldX=${c._eeWorldX})`; break; }
      if (!jsVisible) continue;
      // with a flipped container the sprite is drawn at (container.x + scaleX * x); the renderer wants the world x that gives the same screen x
      const cs = pj >= 1 ? L.container.scaleX : 1;
      const jx = pj >= 1 ? (L.container.x + cs * c.x) + cx - sk : c.x;
      const jsx = cs * c.scaleX, jr = cs * c.rotation;
      const tol = (x, y) => Math.abs(x - y) <= 1e-9 * Math.max(1, Math.abs(x), Math.abs(y));
      if (!tol(jx, v[0])) bad = `sprite ${i} (${c.frameName}) x js=${jx} rust=${v[0]} (p=${pj} base=${c._eeWorldX} layer=${c._eeLayer})`;
      else if (!tol(jsx, v[1])) bad = `sprite ${i} scaleX js=${jsx} rust=${v[1]} (p=${pj})`;
      else if (!tol(jr, v[2])) bad = `sprite ${i} rotation js=${jr} rust=${v[2]} (p=${pj})`;
      else if (!tol(c.alpha, v[3])) bad = `sprite ${i} alpha js=${c.alpha} rust=${v[3]} (p=${pj})`;
    }
    mgs._restoreMirrorTransforms();
    for (const c of [L.additiveContainer, L.container, L.topContainer]) c.scaleX = 1;
    return bad;
  };

  const tw = (t) => (t ? [t.from, t.to, t.elapsed] : null);
  const src = (t) => (t ? t._remapSource || t : t);
  const genericState = () => {
    const G = [...L._genericTriggers];
    const idxOf = (t) => G.indexOf(src(t));
    const sax = (a) => (a ? [a.group, a.elapsed, !!a.exiting, a.from ?? null, !!a.instant] : null);
    const asc = (a, b) => a - b;
    return {
      idx: [L._genericTriggerIdx, L._reverseIdx, !!L._directionDirty],
      fired: [...L._genericFired].sort(asc),
      touching: [...L._genericTouching].sort((a, b) => a[0] - b[0]),
      sequence: [...L._sequenceStates].map(([t, s]) => [G.indexOf(t), s.index, s.lastTime]).filter((e) => e[0] >= 0).sort((a, b) => a[0] - b[0]),
      scaleTweens: L._activeScaleTweens.map((a) => [idxOf(a.trig), a.trig.targetGroup, a.trig.centerGroup, a.elapsed, a.prevX, a.prevY]),
      timeEvents: L._activeTimeEvents.map((e) => [idxOf(e.trig), e.trig._remapSource ? e.trig.targetGroup : null]),
      levelTime: L._levelTime, points: L._points, timeWarp: L.timeWarp,
      timers: L._timers,
      persistentItems: [...L._persistentItems].sort(asc), persistentTimers: [...L._persistentTimers].sort(asc),
      eventLinks: L._eventLinks.map((e) => [idxOf(e), e._remapSource ? e.targetGroup : null]),
      eventStamps: [...L._eventStamps].sort((a, b) => a[0] - b[0]),
      groupScales: Object.fromEntries(Object.entries(L._groupScales).map(([k, v]) => [k, [v.sx, v.sy, v.centerGroupId, v.onlyMove]])),
      hasGroupScales: !!L._hasGroupScales,
      scaledColliders: [...L._scaledColliders].map((c) => c.uid).sort(asc),
      linkVisible: [...L._linkVisibleGroups].sort(asc),
      endTriggered: L.endTriggered ? idxOf(L.endTriggered) : -1,
      cameraOffset: [L.cameraOffsetX, L.cameraOffsetY, L.gameplayOffsetX, L.gameplayOffsetY],
      offsetTweens: [tw(L._cameraOffsetTweens.x), tw(L._cameraOffsetTweens.y)],
      zoom: [L.cameraZoom, tw(L._cameraZoomTween)],
      angle: [L.cameraAngle, L._cameraAngleTarget, tw(L._cameraAngleTween)],
      flags: [!!L.groundHidden, !!L.optionHideP1, !!L.optionHideP2, !!L.p1ControlsDisabled, !!L.p2ControlsDisabled, !!L.attemptsHidden, !!L.freeModeCamera],
      direction: L.direction, gravity: [L.p1GravityScale, L.p2GravityScale], respawnTime: L.respawnTime,
      staticCamera: L.staticCamera ? [sax(L.staticCamera.x), sax(L.staticCamera.y)] : null,
      cameraEdges: L.cameraEdges,
      freeCam: [L.freeCamEasing, L.freeCamPadding, L._scene._pcCount || 0],
      spawnFired: [...L._spawnFiredUids].sort(asc),
    };
  };
  const jsState = () => canon({
    offsets: Object.fromEntries(Object.entries(L._groupOffsets).map(([k, v]) => [k, [v.x, v.y]])),
    rotations: Object.fromEntries(Object.entries(L._groupRotations).map(([k, v]) => [k, [v.totalRad, v.centerGroupId, v.lockRotation]])),
    active: [L._activeMoveTweens.length, L._activeRotateTweens.length, L._activeSpawnDelays.length, L._activeFollowPlayerYTweens.length, L._activeFollowTweens.length],
    followY: L._activeFollowPlayerYTweens.map((a) => [a.elapsed, a.groupInitialGDY, a.history.length]),
    follow: L._activeFollowTweens.map((a) => [a.elapsed, a.lastFollowX, a.lastFollowY]),
    spawnDelays: L._activeSpawnDelays.map((s) => [s.targetGroup, s.delay, s.elapsed, s.remap === undefined ? 0 : s.remap === null ? 1 : 2, s.remap instanceof Map ? [...s.remap].sort((a, b) => a[0] - b[0]) : []]),
    idx: [L._moveTriggerIdx, L._rotateTriggerIdx, L._spawnTriggerIdx, L._stopTriggerIdx, L._toggleTriggerIdx, L._followPlayerYTriggerIdx, L._followTriggerIdx, L._onDeathTriggerIdx, L._touchTriggerIdx, L._pickupTriggerIdx, L._countTriggerIdx, L._instantCountTriggerIdx, L._collisionTriggerIdx],
    generic: genericState(),
    toggledOff: Object.keys(L._groupToggledOff).map(Number).sort((a, b) => a - b),
    armedOnDeath: L._armedOnDeathTriggers.map((t) => L._onDeathTriggers.indexOf(src(t))),
    activeTouch: L._activeTouchTriggers.map((t) => L._touchTriggers.indexOf(src(t))),
    holding: L._activeTouchTriggers.map((t) => !!t._holding),
    activeCount: L._activeCountTriggers.map((t) => L._countTriggers.indexOf(src(t))),
    activeCollision: L._activeCollisionTriggers.map((e) => [L._collisionTriggers.indexOf(src(e.trig)), e.colliding]),
    itemCounts: L._itemCounts,
    blending: Object.fromEntries(Object.entries(L._channelBlending).map(([k, v]) => [k, !!v])),
    colorIdx: [L._colorTriggerIdx, L._pulseTriggerIdx],
    groupOpacity: L._groupOpacity,
    shake: [L._shakeTriggerIdx, L.shakeOffsetX, L.shakeOffsetY, L._shakeState ? [L._shakeState.strength, L._shakeState.endTime, L._shakeState.interval, Number.isFinite(L._shakeState.lastStepTime) ? L._shakeState.lastStepTime : "-inf"] : null, L._touchShakeTriggerActivated.size],
    vis: [L._playerVisTriggerIdx, !!L.playerHidden, L._touchPlayerVisActivated.size],
    anim: [L._animationTriggerIdx, L._touchAnimationTriggerActivated.size, animLog.slice()],
    enter: [L._enterEffectTriggerIdx, L._activeEnterEffect, L._activeExitEffect],
    pulses: L._activePulses.map((p) => [L._pulseTriggers.indexOf(src(p.trig)), p.elapsed, p.totalDuration]),
    groupOps: [...(L._pulseGroupOps || new Map())].map(([g, op]) => [Number(g), op ? [L._pulseTriggers.indexOf(src(op.trig)), op.intensity] : null]),
    alpha: [L._alphaTriggerIdx, ...L._activeAlphaTweens.map((a) => [L._alphaTriggers.indexOf(src(a.trig)), a.elapsed, a.startOpacity])],
  });
  const rustState = () => { const s = rjson(w.wd_trig_state()); return canon(s); };
  const heavy = () => {
    const rows = rustRows();
    for (let i = 0; i < cols.length; i++) {
      for (const [name, v] of [["x", cols[i].x], ["y", cols[i].y], ["rotation_degrees", cols[i].rotationDegrees]]) {
        const j = colLayout.findIndex((f) => f.name === name);
        if (!same(v, rows[i * rowLen + j])) return `collider ${i} (${cols[i].type}) ${name} rust=${rows[i * rowLen + j]} js=${v}`;
      }
    }
    const rs = rjson(w.wd_trig_sections());
    seen.disabled = Math.max(seen.disabled, rs.disabled.length);
    seen.moved = Math.max(seen.moved, cols.filter((c) => c.x !== c._eeInitialBaseX || c.y !== c._eeInitialBaseY).length);
    seen.rotated = Math.max(seen.rotated, cols.filter((c) => c.rotationDegrees !== c._eeInitialRotationDegrees).length);
    seen.resorted = Math.max(seen.resorted, cols.filter((c) => c._eeCollisionSectionIndex !== Math.max(0, Math.floor(c._eeInitialBaseX / 400))).length);
    const js = { sections: {}, length: L._collisionSections.length, disabled: cols.map((c, i) => (c.disabled ? i : -1)).filter((i) => i >= 0) };
    for (const k of Object.keys(L._collisionSections)) { const sec = L._collisionSections[k]; if (Array.isArray(sec) && /^\d+$/.test(k) && Number(k) < 4294967295) js.sections[k] = sec.map((c) => c.uid); }
    if (canon(js) !== canon(rs)) {
      const ks = new Set([...Object.keys(js.sections), ...Object.keys(rs.sections)]);
      for (const k of ks) if (canon(js.sections[k]) !== canon(rs.sections[k])) return `section ${k}: rust=${canon(rs.sections[k])?.slice(0, 120)} js=${canon(js.sections[k])?.slice(0, 120)}`;
      return `sections meta: rust length=${rs.length} disabled=${rs.disabled.length} js length=${js.length} disabled=${js.disabled.length}`;
    }
    return null;
  };

  const toggleCols = cols.filter((c) => c.orbId === 3643);
  const touchTrigs = Object.values(jsLists).flat().filter((t) => t.touchTriggered);
  const lastX = Math.max(0, ...Object.values(jsLists).flat().map((t) => t.x)) + 300;
  let prevCx = undefined, now = 1000, px = -419, py = 30, frame = 0, cx = 0, h1 = false, h2 = false;
  const CS = Object.assign(Object.create(CamCls.prototype), { _cameraX: 0, _cameraY: 0, _followCameraY: undefined, _state: {}, _level: L });
  const camBuf = w.wd_alloc(16); w.wd_cam_reset(); let camModes = 0;
  if (rnd() < 0.5) px = Math.floor(rnd() * lastX * 0.2);
  if (rnd() < 0.4) { // start from a start position: the level fast-forwards its triggers to there
    const ffx = Math.floor(rnd() * lastX); var ffxUsed = ffx;
    try { L.fastForwardTriggers(ffx, cm); w.wd_trig_ff(ffx); px = ffx; if (process.env.DEBUGCM) { w.wd_trig_cm(1001, cmBuf); const m = new Float64Array(w.memory.buffer, cmBuf, 5); console.log(`after ff ${ffx}: js 1001 = ${JSON.stringify(cm.getColor(1001))} rust = ${m[0]},${m[1]},${m[2]}`); } } catch (e) { if (/call stack/.test(String(e.message))) skipped = true; else throw e; }
    if (!skipped && jsState() !== rustState()) fail(`after fast-forward to ${ffx}: state differs\n  js  =${jsState().slice(0, 500)}\n  rust=${rustState().slice(0, 500)}`);
  }
  for (frame = 0; frame < MAXF && px < lastX && !levelBad; frame++) {
    const dt = [1 / 60, 1 / 60, 1 / 60, 1 / 30, 1 / 120, 0.0166667, 0.02][Math.floor(rnd() * 7)];
    px += 623.2 * dt * (rnd() < 0.05 ? 3 : 1) * (rnd() < 0.2 ? 0 : 1) + (rnd() < 0.003 ? rnd() * 800 : 0);
    py = Math.max(-100, Math.min(900, py + (rnd() - 0.5) * 60));
    const near = touchTrigs.find((t) => Math.abs(t.x - px) < 70);
    if (near && rnd() < 0.6) py = near.y + (rnd() - 0.5) * 100;
    cx = px - 419 + (rnd() < 0.1 ? rnd() * 50 : 0);
    const cy = rnd() * 40;
    const dual = rnd() < 0.15, y2 = rnd() * 600;
    const aGrav = rnd() < 0.3, aCam = typeof prevCx === "number" ? prevCx : cx;
    prevCx = cx;
    if (rnd() < 0.002) { // an attempt restarts: everything is reset
      L.resetMoveTriggers(); L.resetRotateTriggers(); L.resetToggleTriggers(); L.resetSpawnTriggers(); L.resetStopTriggers(); L.resetFollowPlayerYTriggers(); L.resetFollowTriggers();
      L.resetColorTriggers(); L.resetPulseTriggers(); L.resetChannelBlending(); cm.reset(); L.resetTimeWarpTriggers(); L.resetOnDeathTriggers(); L.resetTouchTriggers(); L.resetPickupTriggers(); L.resetCountTriggers(); L.resetInstantCountTriggers(); L.resetCollisionTriggers(); L.resetAlphaTriggers(); L.resetShakeTriggers(); L.resetPlayerVisibilityTriggers(); L.resetAnimationTriggers(); L.resetEnterEffectTriggers(); L.resetGenericTriggers();
      if (REAL) for (const s of L._audioScaleSprites) ctx.gdApplyEffectScale(s, 0.1);
      for (let k = 0; k < 20; k++) w.wd_trig_reset(k);
      CS._cameraY = 0; CS._followCameraY = undefined; w.wd_cam_reset();
      px = -419; continue;
    }
    const animMeter = rnd() < 0.5 ? 0 : rnd() * 1.3;
    const nh1 = rnd() < 0.15 ? !h1 : h1, nh2 = rnd() < 0.15 ? !h2 : h2;
    const pr1 = nh1 && !h1, pr2 = nh2 && !h2;
    h1 = nh1; h2 = nh2;
    const died = rnd() < 0.004;
    if (process.env.TRACE) fs.writeSync(2, `frame ${frame} px=${px}\n`);
    // JS: the trigger calls of game-scene.js `update`
    try {
    now += dt * 1000 * (rnd() < 0.1 ? 3 : 1); L._scene.time.now = now; w.wd_trig_time(now);
    L._scene._physicsTick = Math.round(now / (1000 / 240)); w.wd_trig_tick(L._scene._physicsTick);
    if (rnd() < 0.02) { L._scene._attempts = Math.floor(rnd() * 20); w.wd_trig_attempts(L._scene._attempts); }
    if (toggleCols.length && rnd() < 0.05) { const tc = toggleCols[Math.floor(rnd() * toggleCols.length)]; L.activateToggleBlock(tc); w.wd_toggle_block(tc.uid); }
    for (let ge = rnd() < 0.15 ? 1 + Math.floor(rnd() * 3) : 0; ge > 0; ge--) { const ev = Math.floor(rnd() * 80), e1 = rnd() < 0.7 ? 0 : Math.floor(rnd() * 2), e2 = rnd() < 0.7 ? 0 : Math.floor(rnd() * 2); L.gameEvent(ev, e1, e2); w.wd_game_event(ev, e1, e2); }
    if (REAL) { const gs = Object.assign(Object.create(AnimCls.prototype), { _level: L, _state: { gravityFlipped: !!aGrav }, _audio: { getMeteringValue: () => animMeter }, _cameraX: aCam }); animBlock.call(gs, dt * 1000); w.wd_fx_animate(dt * 1000, aCam, aGrav ? 1 : 0, animMeter); }
    L._scene._playerWorldX = px; L._scene._state.y = py; L._scene._cameraY = cy; L._cameraXRef._v = cx;
    for (const t of L.checkColorTriggers(px)) applyCT(t);
    for (const t of L.checkTouchColorTriggers(px, py)) applyCT(t); if (dual) for (const t of L.checkTouchColorTriggers(px, y2)) applyCT(t);
    L.checkMoveTriggers(px); L.checkSpawnTriggers(px);
    L.checkTouchSpawnTriggers(px, py); if (dual) L.checkTouchSpawnTriggers(px, y2);
    L.checkTouchMoveTriggers(px, py); if (dual) L.checkTouchMoveTriggers(px, y2);
    L.beginGroupTransformBatch();
    L.stepMoveTriggers(dt); L.updateGenericTriggers(px, py, dual ? y2 : null, dt, cm); L.stepSpawnTriggers(dt, cm);
    L.checkAlphaTriggers(px); L.checkTouchAlphaTriggers(px, py); if (dual) L.checkTouchAlphaTriggers(px, y2); L.stepAlphaTriggers(dt);
    L.checkRotateTriggers(px); L.stepRotateTriggers(dt);
    L.checkStopTriggers(px); L.checkTouchStopTriggers(px, py); if (dual) L.checkTouchStopTriggers(px, y2);
    L.checkFollowPlayerYTriggers(px); L.checkTouchFollowPlayerYTriggers(px, py); if (dual) L.checkTouchFollowPlayerYTriggers(px, y2);
    L.stepFollowPlayerYTriggers(dt);
    L.checkFollowTriggers(px); L.checkTouchFollowTriggers(px, py); if (dual) L.checkTouchFollowTriggers(px, y2);
    L.stepFollowTriggers(dt);
    L.flushGroupTransformBatch();
    L.checkToggleTriggers(px);
    L.checkAnimationTriggers(px); L.checkTouchAnimationTriggers(px, py); if (dual) L.checkTouchAnimationTriggers(px, y2);
    L.checkTouchToggleTriggers(px, py); if (dual) L.checkTouchToggleTriggers(px, y2);
    L.checkShakeTriggers(px); L.checkPlayerVisibilityTriggers(px); L.checkTouchPlayerVisibilityTriggers(px, py); if (dual) L.checkTouchPlayerVisibilityTriggers(px, y2);
    L.checkTouchShakeTriggers(px, py); if (dual) L.checkTouchShakeTriggers(px, y2);
    L.checkOnDeathTriggers(px); L.checkTouchOnDeathTriggers(px, py); if (dual) L.checkTouchOnDeathTriggers(px, y2);
    L.checkTouchTriggers(px); L.checkTouchTouchTriggers(px, py); if (dual) L.checkTouchTouchTriggers(px, y2);
    const dualIn = L.isTouchTriggerDualMode();
    L.stepTouchTriggers(h1, pr1, dualIn && h2, dualIn && pr2, cm);
    L.checkPickupTriggers(px, cm); L.checkTouchPickupTriggers(px, py, cm); if (dual) L.checkTouchPickupTriggers(px, y2, cm);
    L.checkCountTriggers(px, cm); L.checkCollisionTriggers(px); L.updateCollisionTriggers();
    L.checkTouchCountTriggers(px, py, cm); if (dual) L.checkTouchCountTriggers(px, y2, cm);
    L.checkInstantCountTriggers(px, cm); L.checkTouchInstantCountTriggers(px, py, cm); if (dual) L.checkTouchInstantCountTriggers(px, y2, cm);
    L.checkPulseTriggers(px); L.stepPulseTriggers(dt, cm); cm.step(dt);
    L.checkEnterEffectTriggers(px);
    L.checkTimeWarpTriggers(px, py, dual ? y2 : null);
    L.applyShakeToOffsets(now);
    if (REAL) L.updateVisibility(cx);
    if (REAL) L.applyColorChannels(cm, true);
    if (REAL) L.applyEnterEffects(cx);
    if (BEAST) ctx.window.BeastAnim.update({ _level: L }, dt * 1000);
    } catch (e) { if (/call stack/.test(String(e.message))) { skipped = true; break; } throw e; }
    if (process.env.TRACE) fs.writeSync(2, `js done\n`);
    if (process.env.PHASE) w.wd_trig_phase_limit(frame >= Number(process.env.AT) ? Number(process.env.PHASE) : 1e9);
    // Rust
    w.wd_trig_frame(px, py, cx, cy, dt, dual ? 1 : 0, y2, (h1 ? 1 : 0) | (pr1 ? 2 : 0) | (h2 ? 4 : 0) | (pr2 ? 8 : 0));
    if (REAL) w.wd_fx_enter(cx, px);
    if (BEAST) w.wd_beast_update(dt * 1000);
    if (died) { L.triggerOnDeath(cm); w.wd_trig_death(); }
    framesTotal++;
    if (rnd() < 0.05) camModes = Math.floor(rnd() * 128);
    {
      const st = { y: py, gravityFlipped: !!(camModes & 1), isFlying: !!(camModes & 2), isUfo: !!(camModes & 4), isWave: !!(camModes & 8), isBall: !!(camModes & 16), isSwing: !!(camModes & 32), isSpider: !!(camModes & 64) };
      if (frame === 0 && process.env.CAMDBG) console.error("flyCameraTarget", L.flyCameraTarget);
      if (L.flyCameraTarget === undefined) L.flyCameraTarget = null;
      CS._state = st; CS._cameraX = cx + (L.cameraOffsetX || 0) + (L.gameplayOffsetX || 0);
      const q = dt * 60;
      camBlock.call(CS, q);
      w.wd_cam_step(cx, py, camModes, q, camBuf);
      const rc = new Float64Array(w.memory.buffer, camBuf, 2);
      const same = (a, b) => a === b || (Number.isNaN(a) && Number.isNaN(b));
      if (!same(CS._cameraX, rc[0]) || !same(CS._cameraY, rc[1])) fail(`frame ${frame}: camera differs js=(${CS._cameraX}, ${CS._cameraY}) rust=(${rc[0]}, ${rc[1]}) static=${JSON.stringify(L.staticCamera)} edges=${JSON.stringify(L.cameraEdges)} free=${L.freeModeCamera} zoom=${L.cameraZoom}`);
      const gstc = (globalThis.__cam ||= {});
      if (L.staticCamera && (L.staticCamera.x || L.staticCamera.y)) gstc.staticFrames = (gstc.staticFrames || 0) + 1;
      if (L.cameraEdges) gstc.edgeFrames = (gstc.edgeFrames || 0) + 1;
      if (L.freeModeCamera) gstc.freeFrames = (gstc.freeFrames || 0) + 1;
      if ((L.cameraZoom || 1) !== 1) gstc.zoomFrames = (gstc.zoomFrames || 0) + 1;
    }
    const a = jsState(), b = rustState();
    if (L._activePulses.length) globalThis.__pulseFrames = (globalThis.__pulseFrames || 0) + 1;
    if (L._activeAlphaTweens.length) globalThis.__alphaFrames = (globalThis.__alphaFrames || 0) + 1;
    const gst = (globalThis.__gen ||= {});
    gst.timerFrames = (gst.timerFrames || 0) + (Object.keys(L._timers).length ? 1 : 0);
    gst.scaleFrames = (gst.scaleFrames || 0) + (L._activeScaleTweens.length ? 1 : 0);
    gst.reversed = (gst.reversed || 0) + (L.direction < 0 ? 1 : 0);
    gst.scaledCols = Math.max(gst.scaledCols || 0, L._scaledColliders.size);
    gst.items = Math.max(gst.items || 0, Object.keys(L._itemCounts).length);
    gst.events = Math.max(gst.events || 0, L._eventLinks.length);
    gst.camTween = (gst.camTween || 0) + (L._cameraOffsetTweens.x || L._cameraOffsetTweens.y || L._cameraZoomTween || L._cameraAngleTween ? 1 : 0);
    gst.fired = Math.max(gst.fired || 0, L._genericFired.size);
    gst.seq = Math.max(gst.seq || 0, L._sequenceStates.size);
    gst.timeWarp = (gst.timeWarp || 0) + (L.timeWarp !== 1 ? 1 : 0);
    gst.end = (gst.end || 0) + (L.endTriggered ? 1 : 0);
    gst.static = (gst.static || 0) + (L.staticCamera ? 1 : 0);
    gst.spawnFired = Math.max(gst.spawnFired || 0, L._spawnFiredUids.size);
    checks++;
    for (const ch of cmChannels) { const c = cm.getColor(ch); w.wd_trig_cm(ch, cmBuf); const m = new Float64Array(w.memory.buffer, cmBuf, 5); if (!same(c.r, m[0]) || !same(c.g, m[1]) || !same(c.b, m[2]) || !same(cm.getOpacity(ch), m[3]) || !same(cm.getHex(ch), m[4])) { if (process.env.DEBUGCM) console.log(`px=${px} idx js=${L._colorTriggerIdx} ffx=${typeof ffxUsed !== 'undefined' ? ffxUsed : 'none'}`, JSON.stringify(L._colorTriggers.filter((t) => t.x <= px + 1 && (t.tintGround || t.index === ch)).map((t) => ({ x: t.x, idx: t.index, tg: t.tintGround, c: t.color, d: t.duration, copy: t.copy, sp: t.spawnTriggered, touch: t.touchTriggered })))); fail(`frame ${frame} colour channel ${ch}: js ${c.r},${c.g},${c.b} op ${cm.getOpacity(ch)} | rust ${m[0]},${m[1]},${m[2]} op ${m[3]}`); break; } }
    if (levelBad) break;
    if (REAL && process.env.TRACE_SPRITE && (process.env.TRACE_LI === undefined || Number(process.env.TRACE_LI) === li)) { const ti = Number(process.env.TRACE_SPRITE); const c = realImgs[ti]; const r = JSON.parse(text(w.wd_fx_sprites()))[ti]; console.log(`f${frame} cx=${cx.toFixed(1)} js scale=${c.scaleX} last=${c._eeLastOrbScale} active=${c._eeActive} mult=${c._eeEffectMult} | rust x=${r[0]} scale=${r[3]} active=${r[7]} vis=${r[6]} frame=${r[10]} jsvis=${c.visible} x=${c.x} wx=${c._eeWorldX} dirty=${c._eeXformDirty} sec=${c._eeSectionIndex} vmin=${L._visMinSec} vmax=${L._visMaxSec}`); }
    if (REAL && COINS && frame % 9 === 0) { const m = coinOps(); if (m) { fail(`frame ${frame} coins: ${m}`); break; } }
    if (REAL && MIRROR) { const m = mirrorCheck(); if (m) { fail(`frame ${frame} mirror: ${m}`); break; } }
    if (REAL && !process.env.NOSPRITE) { const m = spriteCheck(frame % 60 === 0); if (m) { fail(`frame ${frame} (px=${px.toFixed(1)}) ${m}`); break; } }
    if (a !== b) {
      const ja = JSON.parse(a), jb = JSON.parse(b);
      const keys = Object.keys(ja).filter((k) => JSON.stringify(ja[k]) !== JSON.stringify(jb[k]));
      if (keys.includes("generic")) { const gk = Object.keys(ja.generic).filter((k) => JSON.stringify(ja.generic[k]) !== JSON.stringify(jb.generic[k])); keys.splice(keys.indexOf("generic"), 1, ...gk.map((k) => `generic.${k}`)); ja.generic = Object.fromEntries(gk.map((k) => [k, ja.generic[k]])); jb.generic = Object.fromEntries(gk.map((k) => [k, jb.generic[k]])); for (const k of gk) { ja[`generic.${k}`] = ja.generic[k]; jb[`generic.${k}`] = jb.generic[k]; } }
      fail(`frame ${frame} (px=${px.toFixed(1)}) state differs in ${keys.join(", ")}\n` + keys.map((k) => `  ${k}: js=${JSON.stringify(ja[k]).slice(0, 400)}\n  ${k}: rust=${JSON.stringify(jb[k]).slice(0, 400)}`).join("\n"));
      if (process.env.DBG) console.log("followY tweens:", JSON.stringify(L._activeFollowPlayerYTweens.map((a) => ({ target: a.trig.targetGroup, speed: a.trig.speed, delay: a.trig.delay, offset: a.trig.offset, maxSpeed: a.trig.maxSpeed, duration: a.trig.duration, elapsed: a.elapsed, gdy: a.groupInitialGDY, hist: a.history.length, hy: a.history.map((h) => h.y).slice(0, 3) }))), "py", py, "dt", dt, "offs", JSON.stringify(L._groupOffsets[79]));
      break;
    }
    if (frame % 40 === 0 || (L._activeMoveTweens.length + L._activeRotateTweens.length + L._activeFollowPlayerYTweens.length + L._activeFollowTweens.length) > 0 && frame % 7 === 0) {
      const h = heavy();
      if (h) { fail(`frame ${frame} (px=${px.toFixed(1)}) ${h}`); break; }
    }
  }
  if (!levelBad && !skipped) { const h = heavy(); if (h) fail(`end ${h}`); }
  if (BEAST) { const rb = JSON.parse(text(w.wd_beast_info())); globalThis.__beasts = (globalThis.__beasts || 0) + rb.length; if (rb.length !== L._beastSprites.length) { console.log(`MISMATCH beast count rust=${rb.length} js=${L._beastSprites.length}`); bad++; } }
  globalThis.__coinOps = (globalThis.__coinOps || 0) + (typeof coinOpCount === "number" ? coinOpCount : 0);
  levelsDone++; globalThis.__mirrorSeen = (globalThis.__mirrorSeen || 0) + (typeof mirrorSeen === "number" ? mirrorSeen : 0);
  const stats = [L._moveTriggers.length, L._rotateTriggers.length, L._spawnTriggers.length, L._stopTriggers.length, L._toggleTriggers.length, L._followPlayerYTriggers.length, L._followTriggers.length];
  const moved = cols.filter((c) => c.x !== c._eeInitialBaseX || c.y !== c._eeInitialBaseY).length, rotated = cols.filter((c) => c.rotationDegrees !== c._eeInitialRotationDegrees).length, off = cols.filter((c) => c.disabled).length;
  console.log(`${sources[li].label}: ${levelBad ? "MISMATCH" : skipped ? "skipped (the original overflows its stack)" : "ok"}  triggers move/rot/spawn/stop/toggle/followY/follow=${stats.join("/")} colliders=${cols.length} groups=${Object.keys(L._groupColliders).length} frames=${frame} (most at once: moved=${seen.moved} rotated=${seen.rotated} disabled=${seen.disabled} in another section=${seen.resorted})`);
}
if (REAL) console.log(`(sprite tints compared: ${globalThis.__tintChecks || 0}, non-null: ${globalThis.__tintSet || 0}, pulsed: ${globalThis.__tintPulsed || 0})`);
if (globalThis.__gen) console.log(`(generic: ${JSON.stringify(globalThis.__gen)})`);
if (globalThis.__cam) console.log(`(camera: ${JSON.stringify(globalThis.__cam)})`);
console.log(`(frames with a group pulse running: ${globalThis.__pulseFrames || 0}, with an alpha tween running: ${globalThis.__alphaFrames || 0})`);
if (BEAST) console.log(`(beasts spawned: ${globalThis.__beasts || 0})`);
if (COINS) console.log(`(coin operations compared: ${globalThis.__coinOps || 0})`);
if (MIRROR) console.log(`(frames compared with the mirror effect on: ${globalThis.__mirrorSeen || 0})`);
console.log(`${levelsDone} levels, ${framesTotal} frames, ${checks} state checks: ${bad ? bad + " MISMATCHED" : "all identical"}`);
process.exit(bad ? 1 : 0);

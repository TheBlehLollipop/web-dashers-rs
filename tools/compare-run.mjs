// Differential test of whole runs: the original physics step (game-scene.js, verbatim) with the real PlayerObject and the real
// LevelObject floor/ceiling/nearby-object code, versus game.rs, frame by frame, on real levels with random inputs.
import fs from "node:fs";
import { load } from "./physics-harness.mjs";

const root = new URL("..", import.meta.url).pathname;
const H = await load();
const { ctx, w, mem, ptr, layout, colLayout, TYPES, sceneMethods, jsPlayer } = H;
let seed = Number(process.argv[2] || 1);
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const pick = (a) => a[Math.floor(rnd() * a.length)];
const RUNS = Number(process.argv[3] || 30);
const FRAMES = Number(process.argv[4] || 1500);
const ONLY_LEVEL = process.argv[5] !== undefined && process.argv[5] !== "-" ? Number(process.argv[5]) : null;
const SMART = !!process.env.SMART; // smart start pos (needs OPTS=1)
const smartMethods = SMART ? (() => { const gs = fs.readFileSync(root + "ref/assets/scripts/core/game-scene.js", "utf8"); const a = gs.indexOf("_getSmartStartPosSettings(pos) {"); return ctx.vm.runInContext(`(class { ${gs.slice(a, gs.indexOf("_updateProgressBar(percent) {", a))} })`, ctx.vmContext).prototype; })() : null;
const MACRO = !!process.env.MACRO; // macro bot: record the first half of a run (compare the exported macros), replay it in the second half (compare every frame)
const MacroBotJS = MACRO ? (() => { const gs = fs.readFileSync(root + "ref/assets/scripts/core/game-scene.js", "utf8"); const a = gs.indexOf("class MacroBot {"); const b = gs.indexOf("\n}\n", a) + 3; return ctx.vm.runInContext(gs.slice(a, b) + "\nMacroBot", ctx.vmContext); })() : null;
const START = process.argv[6] === "start"; // begin like a real attempt (player reset, start speed/game mode, x = -centerX)

const files = fs.readdirSync(root + "ref/assets/levels").filter((f) => f.endsWith(".txt")).sort();
const parsed = files.map((f) => ctx.parseLevel(fs.readFileSync(root + "ref/assets/levels/" + f, "utf8")));

function buildColliders(lvl) {
  const cols = [];
  for (const lo of lvl.objects) {
    const def = ctx.getObjectFromId(lo.id);
    if (!def || def.type === ctx.triggerType || parseInt(lo.id) === 749) continue;
    const self = { objects: [], objectSprites: {}, _groupColliders: {}, _addCollisionToSection() {}, _getTeleportPortalYOffset: ctx.tpOffset, _secretCoinCompleted: new Set(), _userCoinCompleted: new Set() };
    ctx.spawnCol.call(self, lo, def, lo.x * 2, lo.y * 2, parseInt(lo.id ?? 0, 10), 0, 0, 0, null);
    for (const c of self.objects) if (c instanceof ctx.Collider) cols.push(c);
  }
  cols.forEach((c, i) => (c.uid = i));
  return cols;
}

const same = (x, y) => Object.is(x, y) || x === y || (Number.isNaN(x) && Number.isNaN(y));
const SP = 11.540004;
let macroCarry = null;
const macroStats = { frames: 0, dual: 0, played: 0 };
let respawns = 0, checkpointsSaved = 0, wonRuns = 0, overrideFrames = 0, dualFrames = 0, restarts = 0, totalFrames = 0, runsDone = 0, bad = 0, deaths = 0, modeSeen = {}, effects = {};

const PRACTICE = !!process.env.PRACTICE; // practice mode: random checkpoints, respawn from them after deaths (needs CAMERA=1)
const CAMERA = !!process.env.CAMERA; // also compare the camera, end-of-level detection and the whole-frame update
const PORTAL = !!process.env.PORTAL;
const SIDEWAYS = !!process.env.SIDEWAYS; // sideways gameplay (rotate gameplay ground 3/4) switched on and off between frames
const swapSrc = fs.readFileSync(root + "ref/assets/scripts/core/game-scene.js", "utf8");
const OPTS = !!process.env.OPTS; // restarts go through the level options / start positions like _restartLevel
let portalLevels = null;
for (let r = 0; r < RUNS; r++) {
  let li = ONLY_LEVEL !== null ? ONLY_LEVEL : Math.floor(rnd() * parsed.length);
  if (MACRO && macroCarry) li = macroCarry.li;
  if (PORTAL) { portalLevels ??= parsed.map((_, i) => i).filter((i) => buildColliders(parsed[i]).some((c) => c.type === "portal_dual_on")); li = portalLevels[Math.floor(rnd() * portalLevels.length)]; }
  const cols = buildColliders(parsed[li]);
  if (!cols.length) continue;
  const noClip = MACRO && macroCarry ? macroCarry.noClip : PORTAL || rnd() < 0.7;
  const rows = cols.map(H.colRow);
  // initial values
  const v = {};
  for (const f of layout) v[f.name] = f.type === "Option<f64>" || f.type === "Option<bool>" ? NaN : 0;
  Object.assign(v, { y: 30, last_y: 30, last_ground_y: NaN, last_land_ty: TYPES.indexOf("deco"), last_ground_pos_y: 30, on_ground: 1, can_jump: 1, ball_normal_rotate: 0.86, ball_rotation2: 0.86, nc_accuracy: 100, spider_flash_duration: 0.5, gd_last_slope_steepness: NaN, gd_slope_rotation: NaN });
  Object.assign(v, { "g.player_speed": SP, "g.center_x": 419, "g.slope_jump_boost": 1, "g.slope_ramp_time": 0.1, "g.slope_ramp_floor": 0.4, "g.slope_launch_num": 0.8, "g.slope_launch_cap": 1.1, "g.slope_launch_gain": 1.4,
    "g.no_clip": +noClip, "g.no_clip_accuracy": 0, "g.slope_boost_skip_snap": 1, "g.slope_exit_cap": Infinity, "g.slope_swing_cap": 13, "g.robot_pad_touch": 0, "g.gd_cube_slopes": process.env.LEGACY ? 0 : 1, "g.slope_reach_guard": 1, "g.slope_flat_cap": 1, "g.slope_back_wall": 1, "g.slope_tangent_d": 1,
    "e.floor_y": 0, "e.ceiling_y": NaN, "e.player_world_x": 0, "e.manage_layer": 1, "e.try_collect_secret": 1, "e.try_collect_user": 1, "e.mirror_toggles": 1, "e.physics_tick": 0 });

  // ---- Rust game
  const cptr = w.wd_alloc(Math.max(1, rows.length * colLayout.length) * 8);
  const cpBuf = w.wd_alloc(Math.max(1, rows.length * colLayout.length) * 8); // allocate everything first: growing wasm memory detaches older views
  const carr = new Float64Array(w.memory.buffer, cptr, Math.max(1, rows.length * colLayout.length));
  rows.forEach((row, i) => row.forEach((x, j) => (carr[i * colLayout.length + j] = x)));
  w.wd_game_load(cptr, rows.length);
  layout.forEach((f, i) => (mem()[i] = v[f.name]));
  w.wd_game_set_state(ptr);

  // ---- original JS
  const L = Object.create(ctx.LevelObject.prototype);
  Object.assign(L, { _collisionSections: [], _nearbyBuffer: [], _flyCeilingY: null, _flyFloorY: 0, _flyGroundActive: false, _flyVisualOnly: false, _groundTargetValue: 0,
    _flyVisualFloorInset: 0, _flyVisualCeilingInset: 0, tryCollectSecretCoin: () => true, tryCollectUserCoin: () => true, _coinSprites: [], _orbSprites: [], _persistentItems: new Set(), _persistentTimers: new Set(), resetGenericTriggers() {} });
  cols.forEach((c) => L._addCollisionToSection(c));
  const { o, finish } = jsPlayer(v, cols);
  const v2 = { ...v, "e.second_player": 1, is_dead: 1 };
  const { o: o2, finish: finish2 } = jsPlayer(v2, cols);
  o2.__log = o.__log;
  o2.p.reset = ctx.PlayerState.prototype.reset; o2._cleanupExplosion = () => {}; o2._allLayers = []; o2._playerLayers = []; o2.setInvertedColors = () => {};
  layout.forEach((f, i) => (mem()[i] = v2[f.name]));
  w.wd_game_set_state2(ptr);
  layout.forEach((f, i) => (mem()[i] = v[f.name]));
  w.wd_game_set_state(ptr);
  const S = Object.assign({}, o._scene);
  for (const k of Object.getOwnPropertyNames(sceneMethods)) if (k !== "constructor") S[k] = sceneMethods[k];
  if (SIDEWAYS) {
    const a = swapSrc.indexOf("  _swapSidewaysFrame() {"), b = swapSrc.indexOf("  _rotateGameplay(", a);
    S._swapSidewaysFrame = (0, eval)(`(class { ${swapSrc.slice(a, b)} })`).prototype._swapSidewaysFrame;
    L.gameplaySideways = false; w.wd_set_sideways(0);
  }
  const held = { v: false };
  ctx.window.kbIsDown = () => held.v;
  Object.assign(S, {
    _state: o.p, _state2: o2.p, _player: o, _player2: o2, _dualBallSpawnGravityLock: false, _dualBallOverlapResolved: false, _level: L, _isDual: false, _physicsFrame: 0, _physicsTick: undefined,
    _playerWorldX: 0, _deltaBuffer: 0, _spaceWasDown: false, _slideIn: false, _menuActive: false, _updateLogPopup: false, _macroBot: undefined,
    input: { manager: { hitTest: () => [] }, activePointer: { isDown: false } }, _startPosGui: { list: [] }, cameras: { main: {} }, _audio: { playEffect() {} },
    _totalJumps: 0, _levelJumps: 0, _bumpLevelStat() {}, _pushHitboxTrail() {}, _triggerEndPortal() {},
  });
  S._applyJumpInput = () => { const jumpHeld = ctx.window.kbIsDown("jump"); if (!S._updateLogPopup && jumpHeld && !S._spaceWasDown) S._pushButton(); else if (!jumpHeld && S._spaceWasDown) S._releaseButton(); S._spaceWasDown = jumpHeld; };
  o._scene = S;
  o._gameLayer = L;
  o2._scene = S;
  o2._gameLayer = L;
  const origSet = L.setFlyMode.bind(L);
  L.setFlyMode = (...args) => { o.__log.push("fly:" + JSON.stringify(args)); return origSet(...args); };
  S._scene = S;
  S._cameraX = -419; S._cameraY = 0; S._cameraXRef = { _v: -419 }; S._levelWon = false; S._endCameraOverride = false; S._endCamTween = null; S.__tw = null;
  S.tweens = { add(cfg) { S.__tw = { cfg, t: 0 }; } };
  if (PRACTICE) {
    L.objects = cols; L._resetremovedobject = {}; L._secretCoinRunCollected = new Set(); L._userCoinRunCollected = new Set(); L._audioScaleSprites = []; L.objectSprites = [];
    for (const k of ["_toggleTriggers", "_pickupTriggers", "_countTriggers", "_collisionTriggers", "_instantCountTriggers", "_onDeathTriggers", "_touchTriggers"]) L[k] = [];
    L._timeWarp = 1; L._timeWarpTriggerIdx = 0; L._timeWarpTouchActivated = new Set(); L._timeWarpTriggers = []; L._portalFreeMode = false;
    L._armedOnDeathTriggers = []; L._activeTouchTriggers = []; L._activeCountTriggers = []; L._activeCollisionTriggers = []; L._itemCounts = {}; L._groupToggledOff = {}; L._groupOffsets = {}; L._groupRotations = {};
    for (const k of ["resetGroundTiles", "resetColorTriggers", "resetAlphaTriggers", "resetPulseTriggers", "resetEnterEffectTriggers", "resetAnimationTriggers", "resetShakeTriggers", "resetTimeWarpTriggers", "checkTimeWarpTriggers", "resetChannelBlending", "resetPlayerVisibilityTriggers", "resetVisibility", "applyShakeToOffsets", "updateVisibility", "updateObjectDebugIds", "_updateGlowVisibility"]) L[k] = () => {};
    L.additiveContainer = { x: 0, y: 0 }; L.container = { x: 0, y: 0 }; L.topContainer = { add() {} };
    Object.assign(S, { _updateBackground() {}, _applyMirrorEffect() {}, time: { now: 0 }, add: { image: () => ({ setOrigin() { return this; }, setScrollFactor() { return this; }, setDepth() { return this; }, setScale() { return this; } }) }, _slideIn: false, _mirrorAnimating: false, _practiceBypassPending: false });
    S._audio = { playEffect() {}, startMusic() {}, musicPlaying: true };
    S._practicedMode = new ctx.PracticeMode(); S._practicedMode.practiceMode = true;
    ctx.window.practiceMusicSync = false;
    o._cleanupExplosion = () => {}; o._allLayers = []; o._playerLayers = [];
    w.wd_game_practice(0);
  }
  if (CAMERA) {
    let lastX = 0;
    for (const lo of parsed[li].objects) { const d = ctx.getObjectFromId(lo.id); if (d && d.type === ctx.triggerType) continue; if (lo.x * 2 > lastX) lastX = lo.x * 2; }
    L.endXPos = Math.max(1138 + 1200, lastX + 680);
    L._endPortalContainer = { y: 0 }; L._endPortalShine = { y: 0 };
    w.wd_game_set_end(L.endXPos);
  }

  let macroPhase = 0;
  const rustMacro = () => { const n = w.wd_macro_export(); return JSON.parse(new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), n))); };
  const macroDiff = (label) => {
    const jsM = JSON.parse(JSON.stringify(S._macroBot.exportObject())), rsM = rustMacro();
    const d = [];
    if (jsM.frames.length !== rsM.frames.length) d.push(`frame count: rust=${rsM.frames.length} js=${jsM.frames.length}`);
    const KEYS = ["y", "lastY", "lastGroundPosY", "yVelocity", "onGround", "canJump", "isJumping", "gravityFlipped", "isFlying", "isBall", "isWave", "isUfo", "isSpider", "isBird", "isDart", "isRobot", "isSwing", "isJetpack", "isMini", "wasBoosted", "gdAccel", "robotAccel", "robotTouchedPad", "_slopeBounceActive", "pendingVelocity", "collideTop", "collideBottom", "onCeiling", "upKeyDown", "upKeyPressed", "queuedHold", "_orbActivationConsumedForPress", "isDead", "mirrored", "isDashing", "dashYVelocity", "isDual", "ignorePortals", "_robotHold", "_robotHoldTimer", "_spiderTeleportNoclipDeathPending", "ballShouldRotate", "ballRotateOpposite", "ballNormalRotate", "ballHitPad", "ballRotation2", "_gdFallStartY", "_gdLandFlag", "_gdPrevFallVel", "gravityScale", "_gdWasFalling", "_gdPrevHeld", "_gdPrevCeiling", "_pressCount", "forceTicks", "safeHeadTicks", "flipHeadTicks", "jBlockTicks"];
    const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
    for (let i = 0; i < Math.min(jsM.frames.length, rsM.frames.length) && d.length < 6; i++) {
      const a = jsM.frames[i], b = rsM.frames[i];
      for (const k of ["frame", "playerWorldX", "cameraX", "cameraY", "speed", "dual"]) if (!eq(a[k], b[k])) d.push(`frame ${i} ${k}: rust=${b[k]} js=${a[k]}`);
      for (const pk of ["player", "dualPlayer"]) {
        if ((a[pk] === null) !== (b[pk] === null)) { d.push(`frame ${i} ${pk} null differs`); continue; }
        if (a[pk] === null) continue;
        for (const k of ["rotation", "mode"]) if (!eq(a[pk][k], b[pk][k])) d.push(`frame ${i} ${pk}.${k}: rust=${b[pk][k]} js=${a[pk][k]}`);
        for (const k of KEYS) if (!eq(a[pk].state[k], b[pk].state[k])) d.push(`frame ${i} ${pk}.state.${k}: rust=${b[pk].state[k]} js=${a[pk].state[k]}`);
      }
    }
    if (!eq(Object.keys(jsM.meta).sort(), Object.keys(rsM.meta).sort())) d.push(`meta keys: rust=${Object.keys(rsM.meta)} js=${Object.keys(jsM.meta)}`);
    macroStats.frames += jsM.frames.length; macroStats.dual += jsM.frames.filter((x) => x.dual).length;
    if (d.length) { bad++; runBad = true; console.log(`MACRO ${label} MISMATCH level ${files[li]}\n  ` + d.join("\n  ")); }
    return jsM;
  };
  let runBad = false, f = 0, hold = false, startInfo = "";
  const begin = (gm, sk, isRestart) => {
    const startX = isRestart ? 0 : (portalStart ? portalStart.x : -419);
    startInfo = `gm=${gm} sk=${sk}${isRestart ? " restart" : ""}`;
    if (isRestart) { L.objects = cols; L._resetremovedobject = {}; L._secretCoinRunCollected = new Set(); L._userCoinRunCollected = new Set(); L._audioScaleSprites = []; L.objectSprites = []; L.resetObjects(); }
    S._isDual = false;
    if (isRestart) { ctx.PlayerState.prototype.reset.call(o2.p); o2._cleanupExplosion = () => {}; o2._allLayers = []; o2._playerLayers = []; o2.reset(); ctx.PlayerState.prototype.reset.call(o.p); S._deltaBuffer = 0; S._physicsFrame = 0; S._spaceWasDown = false; S._cameraX = -419; S._cameraY = 0; S._cameraXRef._v = -419; S._levelWon = false; S._endCameraOverride = false; S._endCamTween = null; S.__tw = null; o.noclipStats.totalFrames = 0; o.noclipStats.deathFrames = 0; o.noclipStats.deaths = 0; } else Object.assign(o.p, new ctx.PlayerState());
    o._cleanupExplosion = () => {}; o._allLayers = []; o._playerLayers = [];
    S._playerWorldX = startX;
    o.reset();
    const SPEEDS = [11.540004, 9.30222544655, 14.3488938625, 17.3333393414, 21.3333407279];
    const enter = (gmode, y) => ({ 1: () => o.enterShipMode(), 2: () => o.enterBallMode({ y }), 3: () => o.enterUfoMode(), 4: () => o.enterWaveMode(), 5: () => o.enterRobotMode(), 6: () => o.enterSpiderMode() })[gmode]?.();
    let useRestartLevel = false;
    if (OPTS && isRestart) {
      useRestartLevel = true;
      const so = startOpts;
      ctx.window.settingsMap = { kA2: String(gm), kA4: String(sk), kA3: so.mini ? "1" : "0", kA11: so.grav ? "1" : "0", kA28: so.mir ? "1" : "0", kA8: so.dual ? "1" : "0" };
      w.wd_game_set_start(gm, sk, +so.mini, +so.grav, +so.mir, +so.dual);
      let pos = startPosIdx >= 0 ? startPositions[startPosIdx] : null;
      if (pos && SMART) pos = { ...pos, ...smartMethods._getSmartStartPosSettings.call({}, pos) }; // window.smartStartPos
      if (pos) {
        const y0 = Number.isFinite(Number(pos.y)) ? Number(pos.y) : 30;
        S._playerWorldX = pos.x; o.p.y = y0; o.p.lastY = y0; o.p.lastGroundPosY = y0;
        L.resetGroundState();
        if (pos.gameMode === 2) o.p.y = y0;
        enter(pos.gameMode, y0);
        o.p.gravityFlipped = pos.gravityFlipped; o.p.isMini = pos.miniMode; ctx.vm.runInContext(`playerSpeed = ${SPEEDS[pos.speed]};`, ctx.vmContext);
        o.p.mirrored = pos.mirrored;
        if (pos.dualMode) S._enableDualMode();
        o._lastCollisionWorldX = Number.isFinite(Number(S._playerWorldX)) ? Number(S._playerWorldX) : null; o._lastCollisionWorldY = y0;
        startInfo += ` startpos#${startPosIdx}`;
      } else {
        if (SPEEDS[sk] !== undefined) ctx.vm.runInContext(`playerSpeed = ${SPEEDS[sk]};`, ctx.vmContext);
        L.resetGroundState();
        enter(gm, 30);
        S._applyLevelStartOptions();
      }
      if (CAMERA) S._updateCameraY(0, true);
      startInfo += ` opts=${JSON.stringify(so)}`;
    } else {
      ctx.vm.runInContext(`playerSpeed = ${SPEEDS[sk]};`, ctx.vmContext);
      L.resetGroundState();
      enter(gm, 30);
    }
    if (SIDEWAYS) { L.gameplaySideways = false; L._sidewaysWorldX = 0; S._inSidewaysStep = false; }
    const slog = useRestartLevel ? w.wd_game_restart_level(startPosIdx) : isRestart ? w.wd_game_restart(sk, gm, startX) : w.wd_game_start(sk, gm, startX);
    const out0 = finish(); const jl = out0.__log.slice(); o.__log.length = 0;
    const rl = slog ? new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), slog)).split("|") : [];
    w.wd_game_get_state(ptr, cpBuf);
    const rs0 = mem(), d0 = [];
    layout.forEach((ff, i) => { if (!ff.name.includes(".") && !same(out0[ff.name], rs0[i])) d0.push(`${ff.name}: rust=${rs0[i]} js=${out0[ff.name]}`); });
    if (!same(out0["g.player_speed"], rs0[layout.findIndex((x) => x.name === "g.player_speed")])) d0.push("player_speed differs");
    if (!same(L.getFloorY(), rs0[layout.findIndex((x) => x.name === "e.floor_y")]) || !same(L.getCeilingY() ?? NaN, rs0[layout.findIndex((x) => x.name === "e.ceiling_y")])) d0.push(`floor/ceiling differ (js ${L.getFloorY()}/${L.getCeilingY()})`);
    if (jl.join("|") !== rl.join("|")) d0.push(`log: rust=${rl.join("|")} js=${jl.join("|")}`);
    if (isRestart) { const back = new Float64Array(w.memory.buffer, cpBuf, Math.max(1, rows.length * colLayout.length)); cols.forEach((c, i) => H.colRow(c).forEach((x, j) => { if (!same(x, back[i * colLayout.length + j]) && d0.length < 5) d0.push(`collider ${i} ${colLayout[j].name}: rust=${back[i * colLayout.length + j]} js=${x}`); })); }
    if (d0.length) { bad++; runBad = true; console.log(`START MISMATCH level ${files[li]} gamemode=${gm} speedKey=${sk}\n  ` + d0.slice(0, 10).join("\n  ")); }
  };
  let startGm = 0, startSk = 0, portalStart = null;
  const startOpts = { mini: rnd() < 0.2, grav: rnd() < 0.2, mir: rnd() < 0.15, dual: rnd() < 0.2 };
  let startPosIdx = -1;
  const startPositions = Array.from({ length: Math.floor(rnd() * 3) }, () => ({ x: Math.floor(rnd() * 20000), y: Math.floor(rnd() * 600), gameMode: pick([0, 1, 2, 3, 4, 5, 6]), miniMode: pick([0, 1]), speed: pick([0, 1, 2, 3, 4]), dualMode: pick([0, 0, 1]), mirrored: pick([0, 0, 1]), gravityFlipped: rnd() < 0.2 })).sort((a, b) => a.x - b.x);
  if (SMART) {
    ctx.window.levelObjects = parsed[li].objects;
    const flat = parsed[li].objects.flatMap((o) => [Number(o.x), Number(o.id)]);
    const lp = w.wd_alloc(Math.max(1, flat.length) * 8); new Float64Array(w.memory.buffer, lp, Math.max(1, flat.length)).set(flat);
    w.wd_game_set_level_objects(lp, flat.length / 2); w.wd_game_set_smart(1);
  }
  if (OPTS) {
    const sp = w.wd_alloc(Math.max(1, startPositions.length * 8) * 8);
    new Float64Array(w.memory.buffer, sp, Math.max(1, startPositions.length * 8)).set(startPositions.flatMap((q) => [q.x, q.y, q.gameMode, q.miniMode, q.speed, q.dualMode, q.mirrored, +q.gravityFlipped]));
    w.wd_game_set_start_positions(sp, startPositions.length);
  }
  if (PORTAL) { const ps = cols.filter((c) => c.type === "portal_dual_on"); const pc = ps[Math.floor(rnd() * ps.length)]; portalStart = { x: pc.x - 60 - rnd() * 200, y: pc.y + (rnd() - 0.5) * 40 }; }
  if (START) { startGm = pick([0, 0, 1, 2, 3, 4, 5, 6]); startSk = pick([0, 0, 1, 2, 3, 4]); if (MACRO && macroCarry) { startGm = macroCarry.gm; startSk = macroCarry.sk; } begin(startGm, startSk, !!OPTS);
    if (portalStart) { const y = portalStart.y; o.p.y = y; o.p.lastY = y; o.p.lastGroundPosY = y; w.wd_game_get_state(ptr, cpBuf); for (const nm of ["y", "last_y", "last_ground_pos_y"]) mem()[layout.findIndex((x) => x.name === nm)] = y; w.wd_game_set_state(ptr); } }
  if (MACRO) {
    S._macroBot = new MacroBotJS(S);
    if (!macroCarry) { S._macroBot.startRecording({ level: "t" }); const lp = w.wd_alloc(4); new Uint8Array(w.memory.buffer, lp, 1)[0] = 116; w.wd_macro_record(lp, 1); macroPhase = 1; }
    else {
      const pb = new TextEncoder().encode(macroCarry.json); const pp = w.wd_alloc(pb.length); new Uint8Array(w.memory.buffer, pp, pb.length).set(pb);
      S._macroBot.startPlayback(macroCarry.json); if (!w.wd_macro_play(pp, pb.length)) { bad++; runBad = true; console.log("MACRO: Rust could not parse the macro"); }
      macroPhase = 2;
    }
  }
  for (f = 0; f < FRAMES && !runBad; f++) {
    if (rnd() < 0.12) hold = !hold;
    const delta = process.env.DELTA ? Number(process.env.DELTA) : pick([16.6667, 16.6667, 16.6667, 16, 17, 33.3, 8, 20]);
    // JS
    if (SIDEWAYS && !S._isDual && rnd() < (L.gameplaySideways ? 0.01 : 0.02)) {
      L.gameplaySideways = !L.gameplaySideways; w.wd_set_sideways(L.gameplaySideways ? 1 : 0);
      globalThis.__sidewaysToggles = (globalThis.__sidewaysToggles || 0) + 1;
    }
    if (SIDEWAYS && L.gameplaySideways) globalThis.__sidewaysFrames = (globalThis.__sidewaysFrames || 0) + 1;
    if (process.env.DUAL && START && rnd() < (S._isDual ? 0.004 : 0.01)) { // switch dual mode on / off between frames
      const on = !S._isDual;
      if (on) S._enableDualMode(); else S._disableDualMode();
      const jl = o.__log.slice(); o.__log.length = 0;
      const n = w.wd_game_dual(on ? 1 : 0);
      const rl = n ? new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), n)).split("|") : [];
      if (jl.join("|") !== rl.join("|")) { bad++; runBad = true; console.log(`DUAL ${on ? "ON" : "OFF"} log mismatch level ${files[li]} frame ${f}: rust=${rl.join("|")} js=${jl.join("|")}`); }
    }
    held.v = hold;
    if (CAMERA) {
      // Phaser's tween manager advances before the scene's update, with this frame's delta
      if (S.__tw) { S.__tw.t += delta; const cfg = S.__tw.cfg; cfg.targets.p = cfg.ease(Math.min(1, S.__tw.t / cfg.duration)); }
      L.updateEndPortalY(S._cameraY, o.p.isFlying || o.p.isWave || o.p.isUfo || o.p.isSpider);
      S._winCheck();
      if (S._levelWon) {
        w.wd_game_update(delta, hold ? 1 : 0);
        if (w.wd_game_scene(3) !== 1) { bad++; runBad = true; console.log(`MISMATCH level ${files[li]} run ${r} frame ${f}: the level was won in JS but not in Rust`); }
        wonRuns++;
        break;
      }
    }
    const qd = S._physicsBlock(delta);
    if (CAMERA) {
      S._cameraBlock(qd);
      if (S._state.isFlying || S._state.isSwing) o.updateShipRotation(qd);
      if (S._isDual && (S._state2.isFlying || S._state2.isSwing) && !S._state2.isDead) o2.updateShipRotation(qd);
    }
    vmSet: { const out = finish(); out.__logNow = out.__log.slice(); o.__log.length = 0;
      out.floor = L.getFloorY(); out.ceil = L.getCeilingY() ?? NaN; out.db = S._deltaBuffer; out.pf = S._physicsFrame; globalThis.__js = out; }
    const js = globalThis.__js;
    // Rust
    const logLen = CAMERA ? (w.wd_game_update(delta, hold ? 1 : 0) & ((1 << 30) - 1)) : w.wd_game_step(delta, hold ? 1 : 0);
    const rlogAll = logLen ? new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), logLen)).split("|") : [];
    const rlog = rlogAll.filter((x) => !x.startsWith("dbg:")); const dbg = rlogAll.filter((x) => x.startsWith("dbg:"));
    w.wd_game_get_state(ptr, cpBuf);
    const rs = mem(), rv = {};
    layout.forEach((ff, i) => (rv[ff.name] = rs[i]));
    const diff = [];
    globalThis.__rv = rv;
    for (const ff of H.playerFields) if (!(MACRO && macroPhase === 2 && ff.name === "last_ground_y") && !same(js[ff.name], rv[ff.name])) diff.push(`${ff.name}: rust=${rv[ff.name]} js=${js[ff.name]}`);
    if (!same(js["e.player_world_x"], rv["e.player_world_x"])) diff.push(`player_world_x: rust=${rv["e.player_world_x"]} js=${js["e.player_world_x"]}`);
    if (!same(js["g.player_speed"], rv["g.player_speed"])) diff.push(`player_speed: rust=${rv["g.player_speed"]} js=${js["g.player_speed"]}`);
    if (!same(js.floor, rv["e.floor_y"])) diff.push(`floor: rust=${rv["e.floor_y"]} js=${js.floor}`);
    if (!same(js.ceil, rv["e.ceiling_y"])) diff.push(`ceil: rust=${rv["e.ceiling_y"]} js=${js.ceil}`);
    if (!same(js.db, w.wd_game_state_extra(0))) diff.push(`delta_buffer: rust=${w.wd_game_state_extra(0)} js=${js.db}`);
    if (js.__logNow.join("|") !== rlog.join("|")) diff.push(`log: rust=${rlog.join("|")} js=${js.__logNow.join("|")}`);
    js.__logNow.forEach((e) => { const k = e.split(":")[0]; effects[k] = (effects[k] || 0) + 1; });
    for (const m of ["is_flying", "is_wave", "is_ball", "is_ufo", "is_spider", "is_robot"]) if (js[m]) modeSeen[m] = (modeSeen[m] || 0) + 1;
    if (!same(+S._isDual, rv["e.is_dual"])) diff.push(`is_dual: rust=${rv["e.is_dual"]} js=${+S._isDual}`);
    if (!same(+S._dualBallSpawnGravityLock, rv["e.dual_ball_spawn_lock"]) || !same(+S._dualBallOverlapResolved, rv["e.dual_ball_overlap_resolved"])) diff.push(`dual ball flags: rust=${rv["e.dual_ball_spawn_lock"]}/${rv["e.dual_ball_overlap_resolved"]} js=${+S._dualBallSpawnGravityLock}/${+S._dualBallOverlapResolved}`);
    if (S._isDual) {
      dualFrames++;
      const js2 = finish2(); js2.__log.length = 0;
      w.wd_game_get_state2(ptr);
      const r2 = mem();
      if (process.env.DEBUG_DUAL && dualFrames % 4000 === 1) console.log(`  dual frame ${dualFrames}: p1.y=${js.y} p2.y js=${js2.y} rust=${r2[layout.findIndex((x) => x.name === "y")]} p2 flying=${js2.is_flying} ball=${js2.is_ball} grav=${js2.gravity_flipped} dead=${js2.is_dead}`);
      layout.forEach((ff, i) => { if (!ff.name.includes(".") && !(MACRO && macroPhase === 2 && ff.name === "last_ground_y") && !same(js2[ff.name], r2[i]) && diff.length < 12) diff.push(`p2.${ff.name}: rust=${r2[i]} js=${js2[ff.name]}`); });
    }
    if (CAMERA) {
      for (const [nm, a, b2] of [["camera_x", S._cameraX, w.wd_game_scene(0)], ["camera_y", S._cameraY, w.wd_game_scene(1)], ["end_camera_override", +S._endCameraOverride, w.wd_game_scene(2)], ["end_portal_y", L._endPortalGameY, w.wd_game_scene(4)]]) if (!same(a, b2)) diff.push(`${nm}: rust=${b2} js=${a}`);
      if (S._endCameraOverride) overrideFrames++;
      if (process.env.DEBUG_CAM && S._endCameraOverride) console.log(`  cam frame ${f}: js p=${S._endCamTween.p} t=${S.__tw?.t} x=${S._cameraX} rust x=${w.wd_game_scene(0)} delta=${delta}`);
    }
    if (MACRO && macroPhase === 2) { macroStats.played++; if (!!S._macroBot.playing !== (w.wd_macro_state() === 2)) diff.push(`macro playing: rust=${w.wd_macro_state() === 2} js=${!!S._macroBot.playing}`); }
    if (process.env.MDBG && macroPhase === 2 && f > 425 && f < 445) console.log(`  f${f} js y=${js.y} fly=${js.is_flying} ball=${js.is_ball} og=${js.on_ground} lgy=${js.last_ground_y} cursor=${S._macroBot.cursor} | rust y=${rv.y} fly=${rv.is_flying} og=${rv.on_ground} lgy=${rv.last_ground_y} | log js=${js.__logNow.join(',')} rust=${rlog.join(',')}`);
    totalFrames++;
    if (diff.length) {
      bad++; runBad = true;
      if (process.env.ROWDIFF) { const post = cols.map(H.colRow); const out = []; post.forEach((row, i2) => row.forEach((x, j2) => { if (!same(x, rows[i2][j2])) out.push(`col ${i2} (${cols[i2].type}) ${colLayout[j2].name}: pre=${rows[i2][j2]} post=${x}`); })); console.log('  ROWDIFF:', out.slice(0, 8).join(' | ') || 'none'); }
      if (process.env.REPLAY) { console.log('  activated in JS:', cols.filter((c) => c.activated || c._activatedByPlayer).map((c) => `${c.type}#${c.uid}@(${c.x},${c.y}) w=${c.w} h=${c.h} rot=${c.rotationDegrees} by=${JSON.stringify(c._activatedByPlayer)}`).join(' | '), ' js pw=', js['e.player_world_x']); const rows2 = cols.map(H.colRow); const cp3 = w.wd_alloc(Math.max(1, rows2.length * colLayout.length) * 8); const ca3 = new Float64Array(w.memory.buffer, cp3, Math.max(1, rows2.length * colLayout.length)); rows2.forEach((row, i2) => row.forEach((x, j2) => (ca3[i2 * colLayout.length + j2] = x))); w.wd_game_load(cp3, rows2.length); layout.forEach((ff, i2) => (mem()[i2] = v[ff.name])); w.wd_game_set_state(ptr); w.wd_game_start(3, 5, -419); if (process.env.REPLAY === '2') w.wd_game_get_state(ptr, cp3); const rl3 = w.wd_game_step(Number(process.env.DELTA), 0); console.log('  REPLAY log:', rl3 ? new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), rl3)) : '(empty)'); w.wd_game_get_state(ptr, cp3); console.log('  REPLAY rust died_this_frame =', mem()[layout.findIndex((x) => x.name === 'died_this_frame')], 'with rows built AFTER the js run'); }
      if (process.env.DUMP_V) fs.writeFileSync("/tmp/v.json", JSON.stringify(v, (k, x) => (typeof x === "number" && !Number.isFinite(x) ? String(x) : x)));
      if (process.env.DEBUG_RUN) { const pw = js["e.player_world_x"]; const rv = globalThis.__rv; console.log("  js player y=", js.y, "x=", pw, "mode flags:", ["is_flying","is_ball","is_wave","is_ufo","is_spider","is_robot","gravity_flipped","is_mini"].filter((k)=>js[k]).join(",")); const nb = L.getNearbySectionObjects(pw).slice(); console.log("  section list:", nb.length, "closest:", nb.map((c) => [Math.hypot(c.x - pw, c.y - js.y), c]).sort((a, b) => a[0] - b[0]).slice(0, 4).map(([d, c]) => `${c.type}#${c.uid}@(${c.x},${c.y}) w=${c.w} h=${c.h} d=${d.toFixed(0)}`).join(" | ")); console.log("  rust collide_top/bottom:", rv.collide_top, rv.collide_bottom); console.log("  near:", cols.filter((c) => Math.abs(c.x - pw) < 130 && Math.abs(c.y - js.y) < 130).map((c) => `${c.type} uid=${c.uid} x=${c.x} y=${c.y} w=${c.w} h=${c.h} rot=${c.rotationDegrees} r=${c.hitbox_radius} act=${c.activated}`).join("\n        ")); }
      if (dbg.length) console.log('  rust dbg:', dbg.join(' ')); console.log(`MISMATCH level ${files[li]} run ${r} frame ${f} (${startInfo} noClip=${noClip}, hold=${hold}, delta=${delta})\n  ` + diff.slice(0, 10).join("\n  "));
    }
    if (PRACTICE && !js.is_dead && rnd() < 0.006) {
      const a = S._practicedMode.saveCheckpoint(S._state, S._playerWorldX, S._cameraX, S);
      const b2 = w.wd_game_practice(1) === 1;
      if (a !== b2) { bad++; runBad = true; console.log(`MISMATCH level ${files[li]} frame ${f}: saveCheckpoint js=${a} rust=${b2}`); }
      else if (a) checkpointsSaved++;
    }
    if (PRACTICE && js.is_dead && S._practicedMode.checkpoints.length) {
      deaths++;
      S._deathTimer = 0; o.__log.length = 0;
      S._respawnFromCheckpoint();
      const jl2 = o.__log.slice(); o.__log.length = 0;
      const n2 = w.wd_game_practice(3);
      const rl2 = n2 ? new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), n2)).split("|") : [];
      respawns++;
      const outR = finish(); w.wd_game_get_state(ptr, cpBuf); const rsR = mem(); const dR = [];
      layout.forEach((ff, i) => { if (!ff.name.includes(".") && !same(outR[ff.name], rsR[i])) dR.push(`${ff.name}: rust=${rsR[i]} js=${outR[ff.name]}`); });
      if (!same(outR["g.player_speed"], rsR[layout.findIndex((x) => x.name === "g.player_speed")])) dR.push("player_speed differs");
      if (!same(L.getFloorY(), rsR[layout.findIndex((x) => x.name === "e.floor_y")]) || !same(L.getCeilingY() ?? NaN, rsR[layout.findIndex((x) => x.name === "e.ceiling_y")])) dR.push(`floor/ceiling differ (js ${L.getFloorY()}/${L.getCeilingY()}, rust ${rsR[layout.findIndex((x) => x.name === "e.floor_y")]}/${rsR[layout.findIndex((x) => x.name === "e.ceiling_y")]})`);
      if (!same(S._cameraX, w.wd_game_scene(0)) || !same(S._cameraY, w.wd_game_scene(1))) dR.push(`camera: rust=${w.wd_game_scene(0)}/${w.wd_game_scene(1)} js=${S._cameraX}/${S._cameraY}`);
      if (!same(+S._isDual, rsR[layout.findIndex((x) => x.name === "e.is_dual")])) dR.push("is_dual differs");
      if (S._isDual) { const j2 = finish2(); j2.__log.length = 0; w.wd_game_get_state2(ptr); const r2 = mem(); layout.forEach((ff, i) => { if (!ff.name.includes(".") && !same(j2[ff.name], r2[i]) && dR.length < 12) dR.push(`p2.${ff.name}: rust=${r2[i]} js=${j2[ff.name]}`); }); }
      if (jl2.join("|") !== rl2.join("|")) dR.push(`log: rust=${rl2.join("|")} js=${jl2.join("|")}`);
      const backR = new Float64Array(w.memory.buffer, cpBuf, Math.max(1, rows.length * colLayout.length)); cols.forEach((c, i) => H.colRow(c).forEach((x, j) => { if (!same(x, backR[i * colLayout.length + j]) && dR.length < 12) dR.push(`collider ${i} ${colLayout[j].name}: rust=${backR[i * colLayout.length + j]} js=${x}`); }));
      if (dR.length) { bad++; runBad = true; console.log(`RESPAWN MISMATCH level ${files[li]} frame ${f}\n  ` + dR.slice(0, 10).join("\n  ")); break; }
      hold = false;
      continue;
    }
    if (js.is_dead) { deaths++; if (!(START && process.env.RESTART)) break; startPosIdx = OPTS && startPositions.length && rnd() < 0.5 ? Math.floor(rnd() * startPositions.length) : -1; begin(startGm, startSk, true); if (runBad) break; restarts++; hold = false; }
  }
  if (MACRO && macroPhase === 1 && !runBad) { const jsM = macroDiff("record"); if (!runBad && jsM.frames.length > 30) macroCarry = { li, json: JSON.stringify(jsM), gm: startGm, sk: startSk, noClip }; }
  else if (MACRO && macroPhase === 2) macroCarry = null;
  if (!runBad) {
    // final collider state
    w.wd_game_get_state(ptr, cpBuf);
    const back = new Float64Array(w.memory.buffer, cpBuf, Math.max(1, rows.length * colLayout.length));
    let cb = 0;
    cols.forEach((c, i) => { const row = H.colRow(c); row.forEach((x, j) => { if (!same(x, back[i * colLayout.length + j]) && cb++ < 3) console.log(`COLLIDER MISMATCH level ${files[li]} col ${i} ${colLayout[j].name}: rust=${back[i * colLayout.length + j]} js=${x}`); }); });
    if (cb) { bad++; runBad = true; }
  }
  runsDone++;
}
console.log("modes seen (frames):", JSON.stringify(modeSeen), "\neffects:", JSON.stringify(effects));
console.log(`${checkpointsSaved} checkpoints, ${respawns} respawns from checkpoints; ${wonRuns} levels completed, ${overrideFrames} frames of the end camera; ${restarts} restarts after deaths, ${dualFrames} frames in dual mode`);
if (MACRO) console.log(`macro frames recorded ${macroStats.frames} (${macroStats.dual} dual), frames played back ${macroStats.played}`);
if (SIDEWAYS) console.log(`sideways: ${globalThis.__sidewaysToggles || 0} toggles, ${globalThis.__sidewaysFrames || 0} frames sideways`);
console.log(`${runsDone} runs, ${totalFrames} frames, ${deaths} deaths: ${bad ? bad + " MISMATCHED" : "all identical"}`);
process.exit(bad ? 1 : 0);

// Shared loader: the original PlayerObject (JS) and the Rust/wasm physics, with a field-by-field mapping between them.
globalThis.__random ??= Math.random;
import fs from "node:fs";
import vm from "node:vm";

const root = new URL("..", import.meta.url).pathname;
const read = (p) => fs.readFileSync(root + "ref/assets/scripts/" + p, "utf8");
export const HOST_MATH = { env: { js_sin: Math.sin, js_cos: Math.cos, js_tan: Math.tan, js_acos: Math.acos, js_atan: Math.atan, js_atan2: Math.atan2, js_pow: Math.pow, js_random: () => globalThis.__random() } };

export async function load() {
  const phaser = new Proxy(function () {}, { get: (_, k) => (k === Symbol.toPrimitive ? () => 0 : phaser), apply: () => phaser, construct: () => phaser });
  const noop = () => {};
  const ctx = vm.createContext({ console, Math, Object, Number, Array, Set, Map, JSON, Infinity, NaN, Phaser: phaser, atob, btoa, TextDecoder, TextEncoder, Uint8Array, setTimeout: noop, document: phaser, navigator: { userAgent: "" }, localStorage: { getItem: () => null, setItem: noop } });
  ctx.window = ctx;
  ctx.addEventListener = noop;
  vm.runInContext(`
const u = 1 / 240;
const SpeedPortal = { HALF: 9.30222544655, ONE_TIMES: 11.540004, TWO_TIMES: 14.3488938625, THREE_TIMES: 17.3333393414, FOUR_TIMES: 21.3333407279 };
let playerSpeed = SpeedPortal.ONE_TIMES;
const d = 0.9;
const p = 1.916398;
const f = 600;
const a = 60; const T = 460; function b(y) { return T - y; }
const o = 180; const g = a;
const jumpPadType = "jump_pad"; const jumpRingType = "jump_ring";
var centerX = 0; var screenWidth = 1138;
window.enableLDM = true; window.isEditor = false;
`, ctx);
  vm.runInContext(read("game/allObjects.js"), ctx);
  vm.runInContext(read("libs/pako.min.js"), ctx);
  // level.js: types, Collider, the object tables and the collision slice of _spawnObject
  const src = read("core/level.js");
  const header = src.slice(0, src.indexOf("window.LevelObject = class LevelObject"));
  const start = src.indexOf("  if (objectDef) {\n    const registerCollider = col => {");
  const end = src.indexOf("\n  return objectDef;\n}", start);
  const tp = src.indexOf("  _getTeleportPortalYOffset(levelObj) {");
  const tpEnd = src.indexOf("\n  }\n", tp) + 4;
  vm.runInContext(header + `
this.parseLevel = parseLevel; this.getObjectFromId = getObjectFromId; this.Collider = Collider; this.triggerType = triggerType;
this.spawnCol = function (levelObj, objectDef, worldX, worldY, objectId, linkedObjectId, secretCoinSlot, userCoinSlot, scene) {
  let hasCollisionEntry = false;
  ${src.slice(start, end)}
};
this.tpOffset = ({ ${src.slice(tp, tpEnd).trim()} })._getTeleportPortalYOffset;
`, ctx);
  vm.runInContext(src.slice(src.indexOf("window.LevelObject = class LevelObject")), ctx);
  for (const f of ["level-triggers", "level-shaders", "level-particles", "level-areas", "level-advanced", "level-channels", "level-gradients"]) vm.runInContext(read(`core/${f}.js`), ctx);
  // player.js (re-declares nothing from level.js)
  vm.runInContext(read("core/player.js") + "\nthis.PlayerObject = PlayerObject; this.PlayerState = PlayerState;", ctx);
  // the physics step of game-scene.js, taken verbatim
  const gs = read("core/game-scene.js");
  const q0 = gs.indexOf("  _quantizeDelta("), q1 = gs.indexOf("  update(_0x54fa47, deltaTime) {");
  const b0 = gs.indexOf("  _pushButton(ignoreMacro = false) {"), b1 = gs.indexOf("  _initMacroBot() {");
  const u0 = gs.indexOf("    let quantizedDelta = this._quantizeDelta(deltaTime);"), u1 = gs.indexOf("    if (!this._endCameraOverride) {", u0);
  // V2.1 runs the level triggers inside the substep loop; the physics harness leaves them out (compare-triggers.mjs covers them)
  const tr0 = gs.indexOf("      if (!this._state.isDead) {\n        let playerX = this._playerWorldX;", u0), tr1 = gs.indexOf("        this._level.applyShakeToOffsets?.(this.time.now);\n      }\n", tr0) + "        this._level.applyShakeToOffsets?.(this.time.now);\n      }\n".length;
  const uBody = tr0 > 0 && tr0 < u1 ? gs.slice(u0, tr0) + gs.slice(tr1, u1) : gs.slice(u0, u1);
  const e0 = gs.indexOf("  _getDualSharedSignature(state) {"), e1 = gs.indexOf("  _getMirrorXOffset(xOffset)"), f0 = gs.indexOf("  _enableDualMode(portal = null) {"), f1 = gs.indexOf("  _triggerEndPortal() {");
  const led = read("core/level-editor.js"), l0 = led.indexOf("  _applyLevelStartOptions() {"), l1 = led.indexOf("  _closeEditorStartOptionsPopup() {");
  const c0 = Math.min(gs.indexOf("  _applyCameraEdges() {"), gs.indexOf("  _updateCameraY(")), c1 = gs.indexOf("  _quantizeDelta(");
  const k0 = gs.indexOf("    if (!this._endCameraOverride) {\n      const cameraOffsetX = this._playerWorldX - centerX"), k1 = gs.indexOf("    this._level.additiveContainer.x = -this._cameraX", k0);
  const w0 = gs.indexOf("    if (!this._levelWon && !this._state.isDead && this._level.endXPos > 0) {"), w1 = gs.indexOf("    if (this._levelWon) {", w0);
  const r0 = gs.indexOf("  _respawnFromCheckpoint() {"), r1 = gs.indexOf("  _createMirroredBackgroundTexture(");
  const pm0 = gs.indexOf("class PracticeMode {"), pm1 = gs.indexOf("class MacroBot {");
  ctx.PracticeMode = vm.runInContext(gs.slice(pm0, pm1) + "\nPracticeMode", ctx);
  const sceneMethods = vm.runInContext(`(class { ${gs.slice(r0, r1)}\n ${gs.slice(c0, c1)}\n _cameraBlock(quantizedDelta) {\n${gs.slice(k0, k1)}\n}\n _winCheck() {\n${gs.slice(w0, w1)}\n}\n ${led.slice(l0, l1)}\n ${gs.slice(q0, q1)}\n ${gs.slice(b0, b1)}\n ${gs.slice(e0, e1)}\n ${gs.slice(f0, f1)}\n _physicsBlock(deltaTime) {\n${uBody}\n return quantizedDelta;\n} })`, ctx).prototype;


  const { instance } = await WebAssembly.instantiate(fs.readFileSync(root + (process.env.WASM || "target/wasm32-unknown-unknown/release/wd_core.wasm")), HOST_MATH);
  const w = instance.exports;
  const text = (n) => new TextDecoder().decode(new Uint8Array(w.memory.buffer, w.wd_result_ptr(), n));
  const layout = text(w.wd_layout()).split(",").map((s) => { const [name, type] = s.split(":"); return { name, type }; });
  const [colNames, colTypes] = text(w.wd_col_layout()).split("|");
  const colLayout = colNames.split(",").map((s) => { const [name, type] = s.split(":"); return { name, type }; });
  const TYPES = colTypes.split(",");
  const ptr = w.wd_alloc(layout.length * 8);
  const mem = () => new Float64Array(w.memory.buffer, ptr, layout.length);

  const camel = (s) => s.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
  const special = {
    robot_hold: ["p", "_robotHold"], slope_bounce_active: ["p", "_slopeBounceActive"], orb_activation_consumed_for_press: ["p", "_orbActivationConsumedForPress"],
    slope_exit_vel: ["o", "_slopeExitVel"], slope_contact_frames: ["o", "_slopeContactFrames"], prev_slope_angle: ["o", "_prevSlopeAngle"],
    rotate_action_active: ["o", "rotateActionActive"], rotate_action_time: ["o", "rotateActionTime"], rotate_action_duration: ["o", "rotateActionDuration"],
    rotate_action_start: ["o", "rotateActionStart"], rotate_action_total: ["o", "rotateActionTotal"], rotation: ["o", "_rotation"],
    slope_launch_spin: ["o", "_slopeLaunchSpin"], on_slope_angle: ["o", "_onSlopeAngle"], gd_was_on_slope: ["o", "_gdWasOnSlope"],
    skip_slope_exit: ["o", "_skipSlopeExit"], gd_flip_tick: ["o", "_gdFlipTick"],
    active_slope_obj: ["o", "_activeSlopeObj"], active_slope_angle: ["o", "_activeSlopeAngle"], gd_is_on_slope: ["o", "_gdIsOnSlope"],
    gd_mode_sig: ["o", "_gdModeSig"], gd_mode_tick: ["o", "_gdModeTick"], wave_slide_active: ["o", "_waveSlideActive"],
    last_collision_world_x: ["o", "_lastCollisionWorldX"], last_collision_world_y: ["o", "_lastCollisionWorldY"], prev_slope_obj: ["o", "_prevSlopeObj"],
    slope_riding: ["o", "_slopeRiding"], slope_exit_grace: ["o", "_slopeExitGrace"], slope_entry_x: ["o", "_slopeEntryX"],
    last_land_object: ["o", "_lastLandObject"], last_x_offset: ["o", "_lastXOffset"], ignore_teleport_until_clear: ["o", "_ignoreTeleportUntilClear"],
    nc_total_frames: ["nc", "totalFrames"], nc_death_frames: ["nc", "deathFrames"], nc_deaths: ["nc", "deaths"], nc_accuracy: ["nc", "accuracy"],
    last_blue_pad_tick: ["p", "_lastBluePadTick"], robot_hold_timer: ["p", "_robotHoldTimer"], spider_teleport_pending: ["p", "_spiderTeleportPending"], spider_teleport_noclip_death_pending: ["p", "_spiderTeleportNoclipDeathPending"],
    spider_teleport_anim_timer: ["p", "_spiderTeleportAnimTimer"], spider_flash_timer: ["p", "_spiderFlashTimer"], spider_flash_duration: ["p", "_spiderFlashDuration"],
    gd_last_slope_obj: ["o", "_gdLastSlopeObj"], gd_last_slope_top: ["o", "_gdLastSlopeTop"], gd_last_slope_y_pos: ["o", "_gdLastSlopeYPos"],
    gd_last_slope_steepness: ["o", "_gdLastSlopeSteepness"], gd_last_slope_up: ["o", "_gdLastSlopeUp"], gd_slope_rotation: ["o", "_gdSlopeRotation"],
    last_land_x: ["derived"], last_land_y: ["derived"], last_land_ty: ["derived"],
    press_count: ["p", "_pressCount"], gd_fall_start_y: ["p", "_gdFallStartY"], gd_land_flag: ["p", "_gdLandFlag"], gd_prev_fall_vel: ["p", "_gdPrevFallVel"], gd_was_falling: ["p", "_gdWasFalling"], gd_prev_held: ["p", "_gdPrevHeld"], gd_prev_ceiling: ["p", "_gdPrevCeiling"], force_acc_x: ["derived"], force_acc_y: ["derived"],
    ship_head_uid: ["derived"], ship_head_flipped: ["derived"], prev_ship_head_uid: ["derived"], prev_ship_head_flipped: ["derived"],
  };
  const uidFields = new Set(["active_slope_obj", "prev_slope_obj", "last_land_object", "gd_last_slope_obj"]);
  const noneValue = { pending_velocity: null, slope_exit_vel: null, on_slope_angle: null, gd_flip_tick: undefined, active_slope_obj: null, active_slope_angle: null,
    gd_mode_sig: undefined, gd_mode_tick: undefined, last_collision_world_x: undefined, last_collision_world_y: undefined, last_blue_pad_tick: undefined, gd_last_slope_obj: undefined, gd_last_slope_top: undefined, gd_last_slope_y_pos: undefined, gd_last_slope_up: undefined, prev_slope_obj: null, slope_entry_x: undefined, last_land_object: null, press_count: undefined, gd_fall_start_y: null };
  const playerFields = layout.filter((f) => !f.name.includes("."));
  const loc = (name) => special[name] || ["p", camel(name)];

  // colliders: flat Col arrays <-> JS objects (the real Collider class, created by the original code)
  const colField = {
    ty: (c) => TYPES.indexOf(c.type), x: (c) => c.x, y: (c) => c.y, w: (c) => c.w, h: (c) => c.h, rotation_degrees: (c) => c.rotationDegrees,
    hitbox_radius: (c) => c.hitbox_radius, act_main: (c) => !!c._activatedByPlayer?.main, act_dual: (c) => !!c._activatedByPlayer?.dual, activated: (c) => !!c.activated,
    objid: (c) => c.objid, uid: (c) => c.uid, id: (c) => c.padId ?? c.orbId ?? c.speedId ?? c.coinId, multi_activate: (c) => !!c.multiActivate,
    claim_touch: (c) => !!c.claimTouch, ee_toggle_press: (c) => c._eeTogglePress, force: (c) => c.force ?? 0, force_min: (c) => c.forceMin ?? 0, force_max: (c) => c.forceMax ?? 0,
    force_radial: (c) => !!c.forceRadial, force_range: (c) => !!c.forceRange, force_id: (c) => c.forceId ?? 0, force_flip_y: (c) => !!c.forceFlipY,
    free_mode: (c) => !!c.freeMode, flip_y: (c) => !!c.flipY, orb_rotation: (c) => c.orbRotation ?? 0, speed_value: (c) => c.speedValue, teleport_target_x: (c) => c.teleportTargetX, teleport_target_y: (c) => c.teleportTargetY,
    portal_x: (c) => c.portalX, portal_y: (c) => c.portalY, dash_hold_main: (c) => c._dashHoldTicksByPlayer?.main ?? 0, dash_hold_dual: (c) => c._dashHoldTicksByPlayer?.dual ?? 0,
    slope_angle_deg: (c) => c.slopeAngleDeg, slope_dir: (c) => c.slopeDir, slope_is_filled: (c) => !!c.slopeIsFilled, slope_flip_y: (c) => !!c.slopeFlipY,
    hypo_ax: (c) => c.hypoAx, hypo_ay: (c) => c.hypoAy, hypo_bx: (c) => c.hypoBx, hypo_by: (c) => c.hypoBy, right_ax: (c) => c.rightAx, right_ay: (c) => c.rightAy,
    secret_coin: (c) => c.secretCoinId !== undefined, user_coin: (c) => c.userCoinId !== undefined,
    secret_completed: (c) => !!c._secretCoinCompleted, user_completed: (c) => !!c._userCoinCompleted,
    slope_solid_below: (c) => !!c.slopeSolidBelow, slope_type: (c) => c.slopeType, gd_solid_above: (c) => !!c.gdSolidAbove, gd_wall_on_left: (c) => !!c.gdWallOnLeft,
  };
  const numify = (v) => (typeof v === "boolean" ? +v : v === null || v === undefined ? NaN : v);
  const colRow = (c) => colLayout.map((f) => numify(colField[f.name](c)));

  // Run `fn(o)` on a PlayerObject carrying `vals`, with the level layer showing `cols` (JS Collider objects).
  function jsPlayer(vals, cols = []) {
    const o = Object.create(ctx.PlayerObject.prototype);
    o.p = { y: 0 };
    const orbTouch = vals["e.orb_touch"] !== 0;
    const ceil = vals["e.ceiling_y"];
    const byUid = new Map(cols.map((c) => [c.uid, c]));
    const dummy = (ty, x, y) => ({ type: TYPES[ty], x, y, uid: vals.last_land_object, dummy: true });
    o.noclipStats = {};
    for (const f of playerFields) {
      const v = vals[f.name];
      const [where, key] = loc(f.name);
      if (where === "derived") continue;
      let jsv = f.type === "bool" ? v !== 0 : f.type === "Option<bool>" ? (Number.isNaN(v) ? undefined : v !== 0) : f.type === "Option<f64>" ? (Number.isNaN(v) ? noneValue[f.name] : v) : v;
      if (uidFields.has(f.name) && jsv !== null && jsv !== undefined) jsv = byUid.get(jsv) || dummy(vals.last_land_ty, vals.last_land_x, vals.last_land_y);
      const target = where === "p" ? o.p : where === "nc" ? o.noclipStats : o;
      target[key] = jsv;
    }
        const log = (x) => o.__log.push(x);
    o.__log = [];
    const setHead = (key, uid, flipped) => { o[key] = Number.isNaN(uid) ? null : { obj: byUid.get(uid) || { ...dummy(vals.last_land_ty, vals.last_land_x, vals.last_land_y), uid }, gravityFlipped: flipped !== 0 }; };
    setHead("_shipHeadSlopeContact", vals.ship_head_uid, vals.ship_head_flipped); setHead("_prevShipHeadSlopeContact", vals.prev_ship_head_uid, vals.prev_ship_head_flipped);
    o._forceAccum = { x: vals.force_acc_x, y: vals.force_acc_y };
    const second = vals["e.second_player"] !== 0;
    const anyStub = new Proxy(function () {}, { get: (_, k) => (k === Symbol.toPrimitive ? () => 0 : anyStub), apply: () => anyStub, construct: () => anyStub });
    o._scene = {
      _inSidewaysStep: vals["e.in_sideways_step"] !== 0, _playerWorldX: vals["e.player_world_x"], _isDual: vals["e.is_dual"] !== 0, _physicsTick: vals["e.physics_tick"],
      _practicedMode: { practiceMode: vals["e.practice_mode"] !== 0 }, tweens: { killTweensOf() {}, add() {} }, noclipFlash: {},
      toggleGlitter: (on) => log("glitter:" + on), _enableDualMode: () => log("dual:on"), _disableDualMode: () => log("dual:off"),
      _getGamemodePortalMode: (t) => ({ portal_cube: "cube", portal_fly: "ship", portal_ball: "ball", portal_wave: "wave", portal_ufo: "ufo", portal_robot: "robot", portal_swing: "swing", portal_spider: "spider" })[t] || null,
      toggleMirror: (on) => { log("mirror:" + on); return vals["e.mirror_toggles"] !== 0; },
      _syncDualGlobalsFromPrimary: () => log("syncdual:primary"), _syncDualGlobalsFromSecondary: () => log("syncdual:secondary"),
      _state: second ? {} : o.p, _state2: second ? o.p : {},
    };
    o._gameLayer = {
      getNearbySectionObjects: () => (cols.length ? cols : orbTouch ? [{ type: "jump_ring", multiActivate: true, hitbox_radius: 100, x: 0, y: o.p.y }] : []),
      getFloorY: () => vals["e.floor_y"], getCeilingY: () => (Number.isNaN(ceil) ? null : ceil), _breakblock: () => vals["e.break_block"] !== 0,
      setFlyMode: (...args) => log("fly:" + JSON.stringify(args)),
      _portalFreeMode: vals["e.portal_free_mode"] !== 0,
      setPortalFreeMode(on) { this._portalFreeMode = !!on; if (this._portalFreeMode && (vals["e.fly_ground_active"] !== 0 || !Number.isNaN(vals["e.fly_ceiling_y"]) || !Number.isNaN(vals["e.fly_camera_target"]))) log("fly:[false,0]"); },
      tryCollectSecretCoin: () => vals["e.try_collect_secret"] !== 0, tryCollectUserCoin: () => vals["e.try_collect_user"] !== 0, _coinSprites: [], _orbSprites: [],
    };
    for (const k of ["_syncModeIcons", "setCubeVisible", "setShipVisible", "setBallVisible", "setWaveVisible", "setBirdVisible", "setSwingVisible", "setSpiderVisible", "setRobotVisible", "_playPortalShine", "_orbpadHitEffect",
      "playGravityEffect", "spawnPortalCircle", "_makeportalguide", "_markActivatedOrbSprites", "_primeSpiderAnimationFrame", "_primeRobotAnimationFrame", "_hideRobotJumpFlame", "_spawnSpiderTeleportEffects"]) o[k] = () => {};
    for (const k of ["_streak", "_waveTrail", "_particleEmitter", "_flyParticleEmitter", "_flyParticle2Emitter", "_shipDragEmitter"]) o[k] = anyStub;
    o._playerLayers = [];
    o.killPlayer = function () { if (this.p.isDead || this._scene?._macroBot?.playing) return; this.p.isDead = true; this.p.isDashing = false; this.p.dashYVelocity = 0; o.__log.push("kill"); };
    const W = ctx.window;
    W.slopeJumpBoost = vals["g.slope_jump_boost"] !== 0; W.slopeRampTime = vals["g.slope_ramp_time"]; W.slopeRampFloor = vals["g.slope_ramp_floor"];
    W.slopeLaunchNum = vals["g.slope_launch_num"]; W.slopeLaunchCap = vals["g.slope_launch_cap"]; W.slopeLaunchGain = vals["g.slope_launch_gain"];
    W.noClip = vals["g.no_clip"] !== 0; W.noClipAccuracy = vals["g.no_clip_accuracy"] !== 0; W.slopeBoostSkipSnap = vals["g.slope_boost_skip_snap"] !== 0;
    W.slopeExitCap = vals["g.slope_exit_cap"]; W.slopeSwingCap = vals["g.slope_swing_cap"];
    W.gdCubeSlopes = vals["g.gd_cube_slopes"] !== 0; W.slopeReachGuard = vals["g.slope_reach_guard"] !== 0; W.slopeFlatCap = vals["g.slope_flat_cap"] !== 0; W.slopeBackWall = vals["g.slope_back_wall"] !== 0; W.slopeTangentD = vals["g.slope_tangent_d"];
    W.settingsMap = { kA34: vals["g.robot_pad_touch"] !== 0 ? "1" : "0" };
    vm.runInContext(`playerSpeed = ${vals["g.player_speed"]}; centerX = ${vals["g.center_x"]};`, ctx);
    return { o, finish: () => {
    const out = {};
    for (const f of playerFields) {
      const [where, key] = loc(f.name);
      let v;
      if (where === "derived" && /ship_head/.test(f.name)) {
        const c = f.name.startsWith("prev_") ? o._prevShipHeadSlopeContact : o._shipHeadSlopeContact;
        v = f.name.endsWith("uid") ? (c ? c.obj.uid : NaN) : (c ? +c.gravityFlipped : 0);
      } else if (where === "derived" && /^force_acc/.test(f.name)) {
        v = o._forceAccum ? o._forceAccum[f.name.endsWith("x") ? "x" : "y"] : vals[f.name];
      } else if (where === "derived") {
        const lo = o._lastLandObject;
        v = lo ? (f.name === "last_land_x" ? lo.x : f.name === "last_land_y" ? lo.y : TYPES.indexOf(lo.type)) : vals[f.name];
      } else v = (where === "p" ? o.p : where === "nc" ? o.noclipStats : o)[key];
      if (uidFields.has(f.name) && v && typeof v === "object") v = v.uid;
      out[f.name] = typeof v === "boolean" ? +v : v === null || v === undefined ? NaN : v;
    }
    out["e.player_world_x"] = o._scene._playerWorldX;
    out["g.player_speed"] = vm.runInContext("playerSpeed", ctx);
    out.__log = o.__log;
    return out;
    } };
  }
  function runJs(vals, fn, cols = []) {
    const { o, finish } = jsPlayer(vals, cols);
    fn(o);
    return finish();
  }

  // a missing ship-head contact has no gravity side in the original: both sides compare as 0
  const normHead = (o) => { for (const k of ["", "prev_"]) if (Number.isNaN(o[k + "ship_head_uid"])) o[k + "ship_head_flipped"] = 0; return o; };
  function runRust(vals, which, dt) {
    const a = mem();
    layout.forEach((f, i) => (a[i] = vals[f.name]));
    w.wd_call(which, ptr, dt);
    const r = mem(), out = {};
    layout.forEach((f, i) => { if (!f.name.includes(".")) out[f.name] = r[i]; });
    return normHead(out);
  }

  // Rust checkCollisions: returns state, the colliders' flat rows and the log
  function runRustCollisions(vals, cols, offset, which = 0) {
    const a = mem();
    layout.forEach((f, i) => (a[i] = vals[f.name]));
    const rows = cols.map(colRow);
    const cptr = w.wd_alloc(Math.max(1, rows.length * colLayout.length) * 8);
    const carr = new Float64Array(w.memory.buffer, cptr, Math.max(1, rows.length * colLayout.length));
    rows.forEach((r, i) => r.forEach((v, j) => (carr[i * colLayout.length + j] = v)));
    const logLen = w.wd_check_collisions(which, ptr, cptr, cols.length, offset);
    const log = logLen ? text(logLen).split("|") : [];
    const r = mem(), out = {};
    layout.forEach((f, i) => { out[f.name] = r[i]; });
    normHead(out);
    const back = new Float64Array(w.memory.buffer, cptr, Math.max(1, rows.length * colLayout.length));
    const colsOut = cols.map((_, i) => Array.from(back.slice(i * colLayout.length, (i + 1) * colLayout.length)));
    return { out, colsOut, log };
  }

  ctx.vm = vm; ctx.vmContext = ctx;
  return { sceneMethods, w, mem, ptr, jsPlayer, ctx, layout, colLayout, TYPES, runJs, runRust, runRustCollisions, colRow, playerFields };
}

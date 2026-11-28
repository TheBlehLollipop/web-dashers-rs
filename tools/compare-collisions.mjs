// Differential test: PlayerObject.checkCollisions (JS original) vs the Rust port, on real level colliders.
// Each case puts the player near a random collider of a random shipped level, with random state.
import fs from "node:fs";
import { load } from "./physics-harness.mjs";

const root = new URL("..", import.meta.url).pathname;
const H = await load();
const { ctx, layout, TYPES } = H;
const ONLY = process.argv[4] ? process.argv[4].split(",") : null; // default: every collider type

let seed = Number(process.argv[2] || 777);
const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
const pick = (a) => a[Math.floor(rnd() * a.length)];

// real colliders of every shipped level, created by the original code
const levels = fs.readdirSync(root + "ref/assets/levels").filter((f) => f.endsWith(".txt")).sort().map((f) => {
  const lvl = ctx.parseLevel(fs.readFileSync(root + "ref/assets/levels/" + f, "utf8"));
  const cols = [];
  for (const lo of lvl.objects) {
    const def = ctx.getObjectFromId(lo.id);
    if (!def || def.type === ctx.triggerType || parseInt(lo.id) === 749) continue;
    const self = { objects: [], objectSprites: {}, _groupColliders: {}, _addCollisionToSection() {}, _getTeleportPortalYOffset: ctx.tpOffset, _secretCoinCompleted: new Set(), _userCoinCompleted: new Set() };
    ctx.spawnCol.call(self, lo, def, lo.x * 2, lo.y * 2, parseInt(lo.id ?? 0, 10), 0, 0, 0, null);
    for (const c of self.objects) if (c instanceof ctx.Collider && (ONLY ? ONLY.includes(c.type) : true)) cols.push(c);
  }
  cols.forEach((c, i) => (c.uid = i));
  return cols;
}).filter((c) => c.length);
if (process.env.EXTRA) {
  // synthetic levels with the V2.1 objects (swing portal, spider pad/orb, head blocks, flipped pads/orbs, free-mode portals, rotated spikes)
  const SLOPE_IDS = [289, 291, 299, 305, 315, 321, 324, 326, 333, 337, 363, 366, 369];
  const IDS = [1, 1, 1, 1829, 1859, 1859, 8, 8, 39, 103, 392, 35, 67, 140, 1332, 3005, 3005, 36, 84, 141, 1022, 1333, 1330, 1594, 3004, 3004, 1704, 1751, 12, 13, 47, 660, 111, 745, 1331, 1933, 1933, 286, 287, 10, 11, 99, 101, 200, 201, 202, 203, 1813, 1813, 2866, 2866, 2069, 2069, 3645, 3645, 3643, ...SLOPE_IDS];
  const synth = [];
  for (let k = 0; k < 40; k++) {
    const objs = [];
    for (let n = 0; n < 60; n++) {
      const id = pick(IDS);
      const rot = rnd() < 0.5 ? 0 : pick([90, 180, 270, 45, 30, -90]);
      const flipY = id >= 35 && id !== 8 && rnd() < 0.5 ? 1 : 0;
      objs.push(`1,${id},2,${Math.floor(rnd() * 500)},3,${Math.floor(rnd() * 300) + 15},6,${rot},4,${rnd() < 0.2 ? 1 : 0},5,${flipY},111,${rnd() < 0.5 ? 1 : 0},99,${rnd() < 0.3 ? 1 : 0},149,${pick([0, 1, -1, 3.5, -2.25, 8])},526,${pick([0, -4, 2.5])},527,${pick([0, 5, -1.5, 7])},528,${rnd() < 0.4 ? 1 : 0},529,${rnd() < 0.5 ? 1 : 0},530,${pick([0, 0, 1, 2, 3])},445,${rnd() < 0.5 ? 1 : 0}`);
    }
    const lvl = ctx.parseLevel("kA4,0;" + objs.join(";"));
    const cols = [];
    for (const lo of lvl.objects) {
      const def = ctx.getObjectFromId(lo.id);
      const bid = parseInt(lo.id, 10);
      if (bid === 3643) {
        const tb = new ctx.Collider("jump_ring", lo.x * 2, lo.y * 2, 60, 60, 0);
        tb.orbId = 3643; tb.multiActivate = String(lo._raw?.[99]) === "1"; tb.claimTouch = String(lo._raw?.[445]) === "1";
        cols.push(tb);
        continue;
      }
      if (!def || (def.type === ctx.triggerType && bid !== 1813 && bid !== 2069 && bid !== 3645)) continue;
      const self = { objects: [], objectSprites: {}, _groupColliders: {}, _addCollisionToSection() {}, _getTeleportPortalYOffset: ctx.tpOffset, _secretCoinCompleted: new Set(), _userCoinCompleted: new Set() };
      ctx.spawnCol.call(self, lo, def, lo.x * 2, lo.y * 2, parseInt(lo.id ?? 0, 10), 0, 0, 0, null);
      for (const c of self.objects) if (c instanceof ctx.Collider) cols.push(c);
    }
    cols.forEach((c, i) => (c.uid = i));
    synth.push(cols);
  }
  levels.length = 0; levels.push(...synth.filter((c) => c.length));
}
console.log("levels with colliders:", levels.length);

const N = Number(process.argv[3] || 100000);
const CALL = process.argv[5] || "collide"; // "collide" = checkCollisions, "jump" = updateJump
const SP = [9.30222544655, 11.540004, 14.3488938625, 17.3333393414, 21.3333407279];
let bad = 0, kills = 0, lands = 0, onSlope = 0, slopePush = 0;
const seen = {}, events = {};
for (let t = 0; t < N; t++) {
  const cols = pick(levels);
  const anchor = pick(cols);
  const mini = rnd() < 0.2;
  const size = mini ? 18 : 30;
  const flipped = rnd() < 0.3;
  const px = anchor.x + (rnd() * 2 - 1) * (anchor.w / 2 + 40);
  const edgeY = anchor.y + pick([-1, 1]) * (anchor.h / 2 + size + (rnd() * 2 - 1) * pick([2, 12, 40]));
  const py = rnd() < 0.8 ? edgeY : anchor.y + (rnd() * 2 - 1) * 90;
  const near = cols.filter((c) => Math.abs(c.x - px) < 150);
  // activation state before the call stays as the original left it (fresh objects: not activated)
  const v = {};
  for (const f of layout) {
    if (f.type === "bool") v[f.name] = rnd() < 0.3 ? 1 : 0;
    else if (f.type === "Option<f64>") v[f.name] = NaN;
    else v[f.name] = 0;
  }
  v["e.macro_playing"] = 0; // (a random bool field otherwise)
  v["e.has_level_events"] = 0;
  Object.assign(v, {
    y: py, last_y: py - (rnd() * 2 - 1) * pick([0.5, 4, 16]), y_velocity: pick([0, rnd() * 40 - 20, -22.36, 22.36, rnd() * 4 - 2]),
    is_mini: +mini, gravity_flipped: +flipped, on_ground: rnd() < 0.4 ? 1 : 0, up_key_down: rnd() < 0.5 ? 1 : 0, was_up_key_down: rnd() < 0.5 ? 1 : 0,
    ball_normal_rotate: 1, nc_accuracy: 100, nc_total_frames: Math.floor(rnd() * 50), nc_death_frames: Math.floor(rnd() * 5),
    slope_exit_grace: pick([0, 0, 0, 5]), slope_riding: rnd() < 0.15 ? 1 : 0, slope_exit_vel: rnd() < 0.5 ? NaN : rnd() * 20 - 10,
    prev_slope_angle: pick([0, 0.5, NaN]), slope_contact_frames: Math.floor(rnd() * 30),
    rotation: rnd() * 6 - 3, last_x_offset: rnd() * 20 - 10,
  });
  for (const m of ["is_flying", "is_wave", "is_ball", "is_ufo", "is_spider", "is_robot", "is_swing"]) v[m] = 0;
  const mode = pick(["cube", "cube", "cube", "fly", "wave", "ball", "ufo", "robot", "spider"]);
  if (mode !== "cube") v["is_" + (mode === "fly" ? "flying" : mode)] = 1;
  if (process.env.EXTRA) {
    v.gravity_scale = pick([1, 1, 1, 0.5, 2, -1, 0]); v.force_ticks = pick([0, 0, 1, 2]); v.force_acc_x = pick([0, 3]); v.force_acc_y = pick([0, 0, rnd() * 10 - 5]);
    v.safe_head_ticks = pick([0, 0, 1, 2]); v.flip_head_ticks = pick([0, 0, 1, 2]); v.j_block_ticks = pick([0, 0, 1, 2]); v.press_count = pick([NaN, 0, 1, 2]);
    v["e.in_sideways_step"] = rnd() < 0.2 ? 1 : 0;
  }
  v.queued_hold = rnd() < 0.4 ? 1 : 0; v.up_key_pressed = rnd() < 0.4 ? 1 : 0; v.ignore_portals = rnd() < 0.1 ? 1 : 0; v.ignore_teleport_until_clear = rnd() < 0.2 ? 1 : 0;
  if (rnd() < 0.1) v.is_swing = 1;
  if (rnd() < 0.3 && near.length) { const l = pick(near); v.last_land_object = l.uid; v.last_land_x = l.x; v.last_land_y = l.y; v.last_land_ty = TYPES.indexOf(l.type); }
  v.last_collision_world_x = rnd() < 0.5 ? NaN : px - rnd() * 10; v.last_collision_world_y = rnd() < 0.5 ? NaN : py;
  v.gd_mode_sig = rnd() < 0.5 ? NaN : Math.floor(rnd() * 8); v.gd_mode_tick = NaN;
  v["g.player_speed"] = pick(SP); v["g.center_x"] = pick([0, 100, 300]);
  v["g.no_clip"] = rnd() < 0.1 ? 1 : 0; v["g.no_clip_accuracy"] = rnd() < 0.5 ? 1 : 0; v["g.slope_boost_skip_snap"] = 1;
  v["g.slope_exit_cap"] = pick([Infinity, 20]); v["g.slope_swing_cap"] = 13;
  v["g.slope_ramp_time"] = 0.1; v["g.slope_ramp_floor"] = 0.4; v["g.slope_launch_num"] = 0.8; v["g.slope_launch_cap"] = 1.1; v["g.slope_launch_gain"] = 1.4; v["g.slope_jump_boost"] = 1;
  v["e.is_dual"] = rnd() < 0.2 ? 1 : 0; v["e.physics_tick"] = Math.floor(rnd() * 5000);
  v["e.floor_y"] = pick([0, 0, 120, -1000, py - 40]); v["e.ceiling_y"] = rnd() < 0.5 ? NaN : pick([600, py + 40, py + 10]);
  v["e.manage_layer"] = 0; v["e.fly_ground_active"] = 0; v["e.fly_visual_only"] = 0; v["e.fly_floor_y"] = NaN; v["e.fly_ceiling_y"] = NaN;
  v["e.second_player"] = rnd() < 0.2 ? 1 : 0; v["e.practice_mode"] = rnd() < 0.3 ? 1 : 0; v["e.try_collect_secret"] = rnd() < 0.5 ? 1 : 0; v["e.try_collect_user"] = rnd() < 0.5 ? 1 : 0; v["e.mirror_toggles"] = rnd() < 0.5 ? 1 : 0;
  v["g.gd_cube_slopes"] = process.env.LEGACY ? (rnd() < 0.8 ? 0 : 1) : 1; v["g.slope_reach_guard"] = process.env.LEGACY && rnd() < 0.2 ? 0 : 1; v["g.slope_flat_cap"] = rnd() < 0.9 ? 1 : 0; v["g.slope_back_wall"] = rnd() < 0.9 ? 1 : 0; v["g.slope_tangent_d"] = pick([1, 1, 0.5]);
  v.gd_last_slope_steepness = rnd() < 0.5 ? NaN : pick([0.5, 1, 2]); v.gd_slope_rotation = rnd() < 0.5 ? NaN : rnd() * 1.2; v.gd_is_on_slope = rnd() < 0.3 ? 1 : 0; v.gd_was_on_slope = rnd() < 0.3 ? 1 : 0;
  v.gd_last_slope_up = rnd() < 0.4 ? NaN : +(rnd() < 0.5); v.gd_last_slope_top = rnd() < 0.4 ? NaN : +(rnd() < 0.5); v.gd_last_slope_y_pos = rnd() < 0.5 ? NaN : py + (rnd() * 2 - 1) * 20; v.gd_mode_tick = rnd() < 0.5 ? NaN : v["e.physics_tick"] - Math.floor(rnd() * 40);
  if (rnd() < 0.5 && near.length) v.gd_last_slope_obj = pick(near).uid; else v.gd_last_slope_obj = NaN;
  v["g.robot_pad_touch"] = rnd() < 0.5 ? 1 : 0; v.last_blue_pad_tick = rnd() < 0.5 ? NaN : v["e.physics_tick"] - Math.floor(rnd() * 8);
  v["e.player_world_x"] = px + (rnd() * 2 - 1) * 8; v["e.break_block"] = rnd() < 0.3 ? 1 : 0;
  let offset = px - v["g.center_x"];
  const dt = pick([0.9 / 240, 0.9 / 960, rnd() * 0.05, 0.0037]);
  if (CALL === "jump") { for (const m of ["is_flying", "is_wave", "is_ball", "is_ufo", "is_spider", "is_robot"]) v[m] = 0; v["is_" + pick(["spider", "spider", "spider", "ball", "ufo", "robot", "flying"])] = 1; v.is_dashing = 0; v.pending_velocity = NaN; }
  const isJump = CALL === "jump";
  if (isJump) v["e.orb_touch"] = +H.jsPlayer(v, near).o._isTouchingAvailableOrbForInput(false);

  const cloneCol = (c) => Object.assign(Object.create(Object.getPrototypeOf(c)), c, { _activatedByPlayer: c._activatedByPlayer && { ...c._activatedByPlayer }, _dashHoldTicksByPlayer: c._dashHoldTicksByPlayer && { ...c._dashHoldTicksByPlayer } });
  const pre = near.map(cloneCol);
  // JS first (it mutates the real collider objects), Rust gets rows captured beforehand
  const rustCols = near.map((c) => H.colRow(c));
  const rs = (() => {
    const copy = near.map((c) => Object.assign(Object.create(Object.getPrototypeOf(c)), c, { _activatedByPlayer: c._activatedByPlayer && { ...c._activatedByPlayer }, _dashHoldTicksByPlayer: c._dashHoldTicksByPlayer && { ...c._dashHoldTicksByPlayer } }));
    return H.runRustCollisions(v, copy, isJump ? dt : offset, isJump ? 1 : 0);
  })();
  const js = H.runJs(v, (o) => (isJump ? o.updateJump(dt) : o.checkCollisions(offset)), near);
  const jsColsAfter = near.map((c) => H.colRow(c));

  const same = (x, y) => Object.is(x, y) || x === y || (Number.isNaN(x) && Number.isNaN(y));
  const diff = [];
  for (const f of H.playerFields) if (!same(js[f.name], rs.out[f.name])) diff.push(`${f.name}: rust=${rs.out[f.name]} js=${js[f.name]}`);
  if (!same(js["e.player_world_x"], rs.out["e.player_world_x"])) diff.push(`player_world_x: rust=${rs.out["e.player_world_x"]} js=${js["e.player_world_x"]}`);
  if (js.__log.join("|") !== rs.log.join("|")) diff.push(`log: rust=${rs.log.join("|")} js=${js.__log.join("|")}`);
  jsColsAfter.forEach((row, i) => row.forEach((x, j) => { if (!same(x, rs.colsOut[i][j])) diff.push(`col${i}.${H.colLayout[j].name}: rust=${rs.colsOut[i][j]} js=${x}`); }));
  near.forEach((c) => (seen[c.type] = (seen[c.type] || 0) + 1));
  js.__log.forEach((e) => { const k = e.split(":")[0]; events[k] = (events[k] || 0) + 1; });
  if (Number.isFinite(js.active_slope_obj)) onSlope++;
  if (js.gd_is_on_slope) slopePush++;
  if (js.is_dead) kills++; else if (js.on_ground) lands++;
  if (diff.length && process.env.MINIMIZE && bad < 1) {
    let set = pre.slice();
    const differs = (list) => { const a = H.runJs(v, (o) => o.checkCollisions(offset), list.map(cloneCol)); const b = H.runRustCollisions(v, list.map(cloneCol), offset, 0); return H.playerFields.some((f) => !same(a[f.name], b.out[f.name])); };
    for (let i = set.length - 1; i >= 0; i--) { const t = set.filter((_, k) => k !== i); if (t.length && differs(t)) set = t; }
    console.log("MINIMAL SET:", set.map((c) => `${c.type} uid=${c.uid} x=${c.x} y=${c.y} w=${c.w} h=${c.h} rot=${c.rotationDegrees} byP=${JSON.stringify(c._activatedByPlayer)} act=${c.activated}`).join("\n  "), "\n player x=", offset + v["g.center_x"], "y=", v.y, "lastY=", v.last_y);
  }
  if (diff.length && process.env.DEBUG_COLS && bad < 2) { const pw = offset + v["g.center_x"]; console.log("NEAR:", near.filter((c) => Math.abs(c.x - pw) < 120 && Math.abs(c.y - js.y) < 120).map((c) => `${c.type} uid=${c.uid} x=${c.x} y=${c.y} w=${c.w} h=${c.h} rot=${c.rotationDegrees} act=${c.activated} byP=${JSON.stringify(c._activatedByPlayer)} r=${c.hitbox_radius}`).join("\n  "), "\n pw=", pw, "py=", v.y, "lastY=", v.last_y, "second=", v["e.second_player"], "ignP=", v.ignore_portals, "ignTele=", v.ignore_teleport_until_clear); }
  if (diff.length && bad++ < 4) console.log("MISMATCH", mode, "\n ", diff.slice(0, Number(process.env.ND || 8)).join("\n  "), "\n input:", JSON.stringify(v), "\n offset:", offset, "near:", near.length);
}
console.log("collider types in range:", JSON.stringify(seen), "\neffects:", JSON.stringify(events));
console.log(`${N} cases (${kills} deaths, ${lands} landings, ${onSlope} ended on a slope): ${bad ? bad + " MISMATCHED" : "all identical"}`);
process.exit(bad ? 1 : 0);

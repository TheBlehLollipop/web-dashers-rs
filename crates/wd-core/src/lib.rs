//! Web Dashers core: level parsing, ported from assets/scripts/core/level.js.
pub mod js;
pub mod fasthash;
pub mod editor;
pub mod editor_menu;
pub mod editor_scene;
pub mod editor_pause;
pub mod editor_ui;
pub mod editor_color;
pub mod editor_fields;
pub mod editor_objects;
pub mod editor_trigger;
pub mod editor_ctrig;
pub mod menukeys;
pub mod hitbox;
pub mod beast;
pub mod coins;
pub mod mirror;
pub mod popups_a;
pub mod online;
pub mod macrobot;
pub mod startpos;
pub mod garage;
pub mod lscard;
pub mod uifx;
pub mod slide;
pub mod popups_b;
pub mod jsmath;
pub mod json;
pub mod level;
pub mod physics;
pub mod objects;
pub mod objects_data;
pub mod slopes_data;
pub mod collider;
pub mod pcol;
pub mod pmodes;
pub mod pslope;
pub mod pspider;
pub mod game;
pub mod triggers;
pub mod generic;
pub mod pdual;
pub mod jsonread;
pub mod visual;
pub mod colors;
pub mod settings;
pub mod ground;
pub mod fx;
pub mod audio;
pub mod ui;
pub mod background;

// ---- C ABI used by the JS test harness (and later the browser glue) ----
use std::sync::Mutex;
static OUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[no_mangle]
pub extern "C" fn wd_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// Editor serialization test hook: parse a level string, re-serialize it (`_serializeLevel`); `plain` = 1 returns the uncompressed text.
#[no_mangle]
pub unsafe extern "C" fn wd_ed_roundtrip(ptr: *const u8, len: usize, plain: u32) -> usize {
    let input = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    let l = editor::EdLevel::parse(&input);
    out_str(if plain != 0 { l.plain() } else { l.serialize() })
}

/// Parses a level string at `ptr..ptr+len`; returns the byte length of the JSON result.
#[no_mangle]
pub unsafe extern "C" fn wd_parse_level(ptr: *const u8, len: usize) -> usize {
    let input = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    let json = level::parse_level(&input).to_json();
    let mut out = OUT.lock().unwrap();
    *out = json.into_bytes();
    out.len()
}

#[no_mangle]
pub extern "C" fn wd_result_ptr() -> *const u8 {
    OUT.lock().unwrap().as_ptr()
}

// ---- physics test hooks: whole state moves as a flat f64 array in `physics::layout()` order ----
use physics::{Env, Globals, Player, Sim};
use json::num;

#[no_mangle]
pub extern "C" fn wd_layout() -> usize {
    let json = physics::layout().join(",");
    let mut out = OUT.lock().unwrap();
    *out = json.into_bytes();
    out.len()
}

/// Runs one physics function on the state at `ptr`. `which`: 0 = update_jump.
#[no_mangle]
pub unsafe extern "C" fn wd_call(which: u32, ptr: *mut f64, dt: f64) {
    let n = physics::layout().len();
    let a = std::slice::from_raw_parts_mut(ptr, n);
    let (mut p, mut g, mut e) = (Player::default(), Globals::default(), Env::default());
    let mut it = a.iter().copied();
    p.unpack(&mut it); g.unpack(&mut it); e.unpack(&mut it);
    {
        let mut sim = Sim::new(&mut p, &mut g, &mut e);
        match which {
            0 => sim.update_jump(dt, &[]),
            _ => panic!("unknown physics call"),
        }
    }
    let mut out = Vec::with_capacity(n);
    p.pack(&mut out); g.pack(&mut out); e.pack(&mut out);
    a.copy_from_slice(&out);
}

/// Level string -> JSON of the colliders every object spawns (see `collider::level_colliders_json`).
#[no_mangle]
pub unsafe extern "C" fn wd_level_colliders(ptr: *const u8, len: usize) -> usize {
    let input = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    let json = collider::level_colliders_json(&level::parse_level(&input));
    let mut out = OUT.lock().unwrap();
    *out = json.into_bytes();
    out.len()
}

#[no_mangle]
pub extern "C" fn wd_col_layout() -> usize {
    let mut names = Vec::new();
    pcol::Col::names(&mut names);
    let json = format!("{}|{}", names.join(","), pcol::TYPES.join(","));
    let mut out = OUT.lock().unwrap();
    *out = json.into_bytes();
    out.len()
}

/// `checkCollisions`: state at `ptr` (physics layout), `n` colliders at `cptr` (Col layout). Colliders are
/// written back; the effects log goes to the result buffer as a `|`-joined string.
#[no_mangle]
pub unsafe extern "C" fn wd_check_collisions(which: u32, ptr: *mut f64, cptr: *mut f64, n: usize, offset: f64) -> usize {
    let len = physics::layout().len();
    let a = std::slice::from_raw_parts_mut(ptr, len);
    let cl = pcol::Col::len();
    let carr = std::slice::from_raw_parts_mut(cptr, n * cl);
    let (mut p, mut g, mut e) = (Player::default(), Globals::default(), Env::default());
    let mut it = a.iter().copied();
    p.unpack(&mut it); g.unpack(&mut it); e.unpack(&mut it);
    let mut cols: Vec<pcol::Col> = Vec::with_capacity(n);
    let mut cit = carr.iter().copied();
    for _ in 0..n { cols.push(pcol::Col::unpack(&mut cit)); }
    let log;
    {
        let mut sim = Sim::new(&mut p, &mut g, &mut e);
        match which {
            0 => sim.check_collisions(offset, &mut cols),
            1 => sim.update_jump(offset, &cols),
            _ => panic!("unknown call"),
        }
        log = sim.log;
    }
    let mut out = Vec::with_capacity(len);
    p.pack(&mut out); g.pack(&mut out); e.pack(&mut out);
    a.copy_from_slice(&out);
    let mut cout = Vec::with_capacity(n * cl);
    for c in &cols { c.pack(&mut cout); }
    carr.copy_from_slice(&cout);
    let mut o = OUT.lock().unwrap();
    *o = log.join("|").into_bytes();
    o.len()
}

// ---- whole-game test hooks ----
static GAME: Mutex<Option<game::Game>> = Mutex::new(None);

/// Loads the level's colliders (Col layout rows) and resets the game state.
#[no_mangle]
pub unsafe extern "C" fn wd_game_load(cptr: *mut f64, n: usize) {
    let cl = pcol::Col::len();
    let carr = std::slice::from_raw_parts(cptr, n * cl);
    let mut it = carr.iter().copied();
    let mut cols = Vec::with_capacity(n);
    for _ in 0..n { cols.push(pcol::Col::unpack(&mut it)); }
    *GAME.lock().unwrap() = Some(game::Game::new(cols));
}

/// Sets player/globals/env from a flat array in `physics::layout()` order.
#[no_mangle]
pub unsafe extern "C" fn wd_game_set_state(ptr: *mut f64) {
    let len = physics::layout().len();
    let a = std::slice::from_raw_parts(ptr, len);
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    let mut it = a.iter().copied();
    gm.p.unpack(&mut it); gm.g.unpack(&mut it); gm.e.unpack(&mut it);
    gm.e.has_level_events = true;
}

/// Runs one frame; returns the length of the log (`|`-joined) left in the result buffer.
#[no_mangle]
pub extern "C" fn wd_game_step(delta_ms: f64, held: u32) -> usize {
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    gm.log.clear();
    gm.step_frame(delta_ms, held != 0);
    let mut o = OUT.lock().unwrap();
    *o = gm.log.join("|").into_bytes();
    o.len()
}

/// Writes player/globals/env back to `ptr`, then the colliders to `cptr`.
#[no_mangle]
pub unsafe extern "C" fn wd_game_get_state(ptr: *mut f64, cptr: *mut f64) {
    let guard = GAME.lock().unwrap();
    let gm = guard.as_ref().unwrap();
    let len = physics::layout().len();
    let a = std::slice::from_raw_parts_mut(ptr, len);
    let mut out = Vec::with_capacity(len);
    gm.p.pack(&mut out); gm.g.pack(&mut out); gm.e.pack(&mut out);
    a.copy_from_slice(&out);
    let cl = pcol::Col::len();
    let carr = std::slice::from_raw_parts_mut(cptr, gm.cols.len() * cl);
    let mut cout = Vec::with_capacity(gm.cols.len() * cl);
    for c in &gm.cols { c.pack(&mut cout); }
    carr.copy_from_slice(&cout);
}

#[no_mangle]
pub extern "C" fn wd_game_state_extra(which: u32) -> f64 {
    let guard = GAME.lock().unwrap();
    let gm = guard.as_ref().unwrap();
    match which { 0 => gm.delta_buffer, 1 => gm.physics_frame, 2 => gm.space_was_down as u8 as f64, _ => f64::NAN }
}

/// Start an attempt: speed key (`kA4`), game mode (`kA2`), start x. Returns the length of the log.
#[no_mangle]
pub extern "C" fn wd_game_start(speed_key: f64, game_mode: f64, start_x: f64) -> usize {
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    gm.log.clear();
    gm.start_attempt(speed_key as i64, game_mode as i64, start_x);
    let mut o = OUT.lock().unwrap();
    *o = gm.log.join("|").into_bytes();
    o.len()
}

// ---- trigger test hooks ----
pub(crate) fn with_game<R>(f: impl FnOnce(&mut game::Game) -> R) -> R {
    let mut guard = GAME.lock().unwrap();
    f(guard.as_mut().unwrap())
}
fn out_str(s: String) -> usize { let mut o = OUT.lock().unwrap(); *o = s.into_bytes(); o.len() }

/// Builds the whole game (colliders and triggers) from a level string; returns the number of colliders.
#[no_mangle]
pub unsafe extern "C" fn wd_game_load_level(ptr: *const u8, len: usize) -> usize {
    let mut input = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    if input.contains('\0') { input = input.replace('\0', "").trim().to_string(); }
    let lvl = level::parse_level(&input);
    let official = *OFFICIAL.lock().unwrap();
    let mut g = game::Game::from_level_official(&lvl, official);
    if let Some(init) = COINS_INIT.lock().unwrap().clone() {
        g.coins.enabled = true;
        g.coins.configure(init);
        for (uid, (user, id)) in g.coins.link.clone() {
            let done = g.coins.is_done(user, id);
            if user { g.cols[uid].user_completed = done; } else { g.cols[uid].secret_completed = done; }
        }
    }
    attach_level_sprites(&lvl, &mut g, official);
    let n = g.cols.len();
    *GAME.lock().unwrap() = Some(g);
    n
}
#[no_mangle]
pub extern "C" fn wd_trig_lists() -> usize { out_str(with_game(|g| g.trigger_lists_json())) }
#[no_mangle]
pub extern "C" fn wd_trig_state() -> usize { out_str(with_game(|g| g.trigger_state_json())) }
/// Sprite anchors: per anchor `x, y, group count, groups...`.
#[no_mangle]
pub unsafe extern "C" fn wd_trig_set_anchors(ptr: *const f64, len: usize) {
    let a = std::slice::from_raw_parts(ptr, len);
    let mut v = Vec::new();
    let mut i = 0;
    while i < a.len() {
        let (x, y, n) = (a[i], a[i + 1], a[i + 2] as usize);
        v.push(triggers::Anchor { x, y, groups: a[i + 3..i + 3 + n].iter().map(|g| *g as i64).collect(), toggled: Default::default() });
        i += 3 + n;
    }
    with_game(|g| g.trig.set_anchors(v));
}
#[no_mangle]
pub extern "C" fn wd_trig_frame(px: f64, py: f64, cx: f64, cy: f64, dt: f64, has_dual: u32, dual_y: f64, inputs: u32) {
    with_game(|g| {
        g.trigger_frame(triggers::Pos { player_x: px, player_y: py, camera_x: cx, camera_y: cy }, if has_dual != 0 { Some(dual_y) } else { None }, dt,
            triggers::Input { p1_hold: inputs & 1 != 0, p1_press: inputs & 2 != 0, p2_hold: inputs & 4 != 0, p2_press: inputs & 8 != 0 });
        g.update_visibility(cx);
    });
}
#[no_mangle]
pub extern "C" fn wd_cam_step(cx: f64, py: f64, flags: u32, q: f64, out: *mut f64) {
    with_game(|g| {
        g.camera_x = cx + triggers::or0(g.trig.gen.camera_offset.0) + triggers::or0(g.trig.gen.gameplay_offset.0);
        g.p.y = py;
        g.p.gravity_flipped = flags & 1 != 0; g.p.is_flying = flags & 2 != 0; g.p.is_ufo = flags & 4 != 0; g.p.is_wave = flags & 8 != 0;
        g.p.is_ball = flags & 16 != 0; g.p.is_swing = flags & 32 != 0; g.p.is_spider = flags & 64 != 0;
        g.camera_y_block(q);
        unsafe { *out = g.camera_x; *out.add(1) = g.camera_y; }
    });
}
#[no_mangle]
pub extern "C" fn wd_set_rotate_gameplay(on: u32) { with_game(|g| g.rotate_gameplay_enabled = on != 0); }
#[no_mangle]
pub extern "C" fn wd_set_sideways(on: u32) { with_game(|g| { g.trig.gen.gameplay_sideways = on != 0; g.sync_floor_ceiling(); }); }
#[no_mangle]
pub extern "C" fn wd_cam_reset() { with_game(|g| { g.camera_y = 0.0; g.follow_camera_y = None; }); }
#[no_mangle]
pub extern "C" fn wd_trig_reset(which: u32) {
    with_game(|g| match which { 0 => g.reset_move(), 1 => g.reset_rotate(), 2 => g.reset_toggle(), 3 => g.reset_spawn(), 4 => g.reset_stop(), 5 => g.reset_follow_y(), 6 => g.reset_follow(), 13 => g.reset_color(), 14 => g.reset_pulse(), 15 => g.reset_channel_blending(), 16 => g.cm.reset(), 17 => g.reset_alpha(), 18 => { g.reset_shake(); g.reset_player_vis(); g.reset_animation(); g.reset_enter_effect() } 19 => g.reset_generic(false), 7 => g.reset_on_death(), 8 => g.reset_touch(), 9 => g.reset_pickup(), 10 => g.reset_count(), 11 => g.reset_instant_count(), _ => g.reset_collision() });
}
/// Collision sections (`{index: [collider indices]}`) and the disabled colliders.
#[no_mangle]
pub extern "C" fn wd_trig_sections() -> usize {
    out_str(with_game(|g| {
        let secs: Vec<String> = g.sections.iter().map(|(k, v)| format!("\"{}\":[{}]", k, v.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(","))).collect();
        let dis: Vec<String> = g.disabled.iter().enumerate().filter(|(_, d)| **d).map(|(i, _)| i.to_string()).collect();
        format!("{{\"sections\":{{{}}},\"length\":{},\"disabled\":[{}]}}", secs.join(","), g.sections_len, dis.join(","))
    }))
}
#[no_mangle]
pub extern "C" fn wd_game_event(event: f64, extra: f64, extra2: f64) { with_game(|g| g.game_event(event as i64, extra as i64, extra2 as i64)); }
#[no_mangle]
pub extern "C" fn wd_toggle_block(ci: f64) { with_game(|g| g.activate_toggle_block(ci as usize)); }
#[no_mangle]
pub extern "C" fn wd_trig_attempts(v: f64) { with_game(|g| g.trig.gen.attempts = v); }
#[no_mangle]
pub extern "C" fn wd_trig_death() { with_game(|g| g.trigger_on_death()); }
#[no_mangle]
pub extern "C" fn wd_trig_phase_limit(n: u32) { with_game(|g| g.trig.phase_limit = n); }
/// Restart after a death (see `Game::restart`); returns the length of the log.
#[no_mangle]
pub extern "C" fn wd_game_restart(speed_key: f64, game_mode: f64, start_x: f64) -> usize {
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    gm.log.clear();
    gm.restart(speed_key as i64, game_mode as i64, start_x);
    let mut o = OUT.lock().unwrap();
    *o = gm.log.join("|").into_bytes();
    o.len()
}

/// Writes the second player (then globals and env) to `ptr`, in the same layout as `wd_game_get_state`.
#[no_mangle]
pub unsafe extern "C" fn wd_game_get_state2(ptr: *mut f64) {
    let guard = GAME.lock().unwrap();
    let gm = guard.as_ref().unwrap();
    let len = physics::layout().len();
    let a = std::slice::from_raw_parts_mut(ptr, len);
    let mut out = Vec::with_capacity(len);
    gm.p2.pack(&mut out); gm.g.pack(&mut out); gm.e.pack(&mut out);
    a.copy_from_slice(&out);
}

/// Sets the second player from `ptr` (player part only).
#[no_mangle]
pub unsafe extern "C" fn wd_game_set_state2(ptr: *mut f64) {
    let len = physics::layout().len();
    let a = std::slice::from_raw_parts(ptr, len);
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    let mut it = a.iter().copied();
    gm.p2.unpack(&mut it);
}

/// Calls the scene's enable / disable dual mode directly (tests).
#[no_mangle]
pub extern "C" fn wd_game_dual(on: u32) -> usize {
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    gm.log.clear();
    gm.set_dual(on != 0);
    let mut o = OUT.lock().unwrap();
    *o = gm.log.join("|").into_bytes();
    o.len()
}

/// Sets the level start options (tests); start positions come from `wd_game_load_level`.
#[no_mangle]
pub extern "C" fn wd_game_set_start(game_mode: f64, speed_key: f64, mini: u32, grav: u32, mirrored: u32, dual: u32) {
    with_game(|g| g.start = game::StartOptions { game_mode: game_mode as i64, speed_key: speed_key as i64, mini: mini != 0, gravity_flipped: grav != 0, mirrored: mirrored != 0, dual: dual != 0 });
}
/// `_restartLevel`; `start_pos` < 0 means no start position. Returns the log length.
#[no_mangle]
pub extern "C" fn wd_game_restart_level(start_pos: f64) -> usize {
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    gm.log.clear();
    gm.restart_level(if start_pos >= 0.0 { Some(start_pos as usize) } else { None });
    let mut o = OUT.lock().unwrap();
    *o = gm.log.join("|").into_bytes();
    o.len()
}

/// Start positions (tests): per entry `x, y, game_mode, mini, speed, dual, mirrored, gravity_flipped` (NaN = missing).
#[no_mangle]
pub unsafe extern "C" fn wd_game_set_start_positions(ptr: *const f64, n: usize) {
    let a = std::slice::from_raw_parts(ptr, n * 8);
    let o = |v: f64| if v.is_nan() { None } else { Some(v as i64) };
    let v: Vec<game::StartPos> = (0..n).map(|i| { let r = &a[i * 8..i * 8 + 8]; game::StartPos { x: r[0], y: r[1], game_mode: o(r[2]), mini: o(r[3]), speed: o(r[4]), dual: o(r[5]), mirrored: o(r[6]), gravity_flipped: r[7] != 0.0 } }).collect();
    with_game(|g| g.start_positions = v);
}
/// Smart start pos on/off (`window.smartStartPos`).
#[no_mangle]
pub extern "C" fn wd_game_set_smart(on: u32) { with_game(|g| g.sp.smart = on != 0); }
/// The (x, id) pairs of `window.levelObjects` (tests; `wd_game_load_level` fills them itself).
#[no_mangle]
pub unsafe extern "C" fn wd_game_set_level_objects(ptr: *const f64, n: usize) {
    let a = std::slice::from_raw_parts(ptr, n * 2);
    with_game(|g| { g.sp.warps.clear(); g.sp.objects = (0..n).map(|i| (a[i * 2], if a[i * 2 + 1].is_nan() { i64::MIN } else { a[i * 2 + 1] as i64 })).collect(); });
}
/// `_getSmartStartPosSettings(pos)` into `out` (6 numbers: game mode, mini, speed, dual, gravity flipped, mirrored).
#[no_mangle]
pub unsafe extern "C" fn wd_game_smart(pos_x: f64, out: *mut f64) {
    let s = with_game(|g| g.smart_start_settings(pos_x));
    let o = std::slice::from_raw_parts_mut(out, 6);
    o.copy_from_slice(&[s.game_mode as f64, s.mini as u8 as f64, s.speed as f64, s.dual as u8 as f64, s.gravity_flipped as u8 as f64, s.mirrored as u8 as f64]);
}
/// `_quantizeDelta` (tests)
#[no_mangle]
pub extern "C" fn wd_game_quantize(delta_ms: f64) -> f64 { with_game(|g| g.quantize_delta_pub(delta_ms)) }
#[no_mangle]
pub extern "C" fn wd_game_set_delta_buffer(v: f64) { with_game(|g| g.delta_buffer = v); }
/// Options that change gameplay (gd_settings): noclip, noclip accuracy, speed hack, respawn delay, smart start pos. 0 speed / NaN = unset.
#[no_mangle]
pub extern "C" fn wd_game_set_options(noclip: u32, noclip_acc: u32, speed_hack: f64, respawn_delay: f64, smart: u32) {
    with_game(|g| { g.g.no_clip = noclip != 0; g.g.no_clip_accuracy = noclip_acc != 0; g.speed_hack = speed_hack; g.respawn_delay = respawn_delay; g.sp.smart = smart != 0; });
}
/// `_restartLevel` from the current start position (the switcher's choice)
#[no_mangle]
pub extern "C" fn wd_game_restart_current() -> usize {
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    gm.log.clear();
    let sp = gm.start_pos_index;
    gm.restart_level(sp);
    let mut o = OUT.lock().unwrap();
    *o = gm.log.join("|").into_bytes();
    o.len()
}
/// Current start position index (-1 = level start) and the count
#[no_mangle]
pub extern "C" fn wd_game_start_pos_index() -> f64 { with_game(|g| g.start_pos_index.map_or(-1.0, |i| i as f64)) }
/// `window.speedHack`
#[no_mangle]
pub extern "C" fn wd_game_set_speedhack(v: f64) { with_game(|g| g.speed_hack = v); }
/// `window.respawnDelay`
#[no_mangle]
pub extern "C" fn wd_game_set_respawn_delay(v: f64) { with_game(|g| g.respawn_delay = v); }
/// `changeStartPos(direction)`; returns the new index (-1 = none) and logs like a restart.
#[no_mangle]
pub extern "C" fn wd_game_change_start_pos(direction: i32) -> f64 {
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    gm.log.clear();
    gm.change_start_pos(direction as i64);
    gm.start_pos_index.map_or(-1.0, |i| i as f64)
}
#[no_mangle]
pub extern "C" fn wd_game_set_start_pos_index(i: f64) { with_game(|g| g.start_pos_index = if i >= 0.0 { Some(i as usize) } else { None }); }
/// Macro bot: `startRecording`. `level` is stored in the macro's meta.
#[no_mangle]
pub unsafe extern "C" fn wd_macro_record(level: *const u8, len: usize) {
    let l = String::from_utf8_lossy(std::slice::from_raw_parts(level, len)).into_owned();
    with_game(|g| g.macro_start_recording(&[("level".to_string(), jsonread::Json::Str(l))]));
}
/// `stopRecording()`: the macro as JSON text (length; read with `wd_result_ptr`)
#[no_mangle]
pub extern "C" fn wd_macro_stop_recording() -> usize { let s = with_game(|g| g.macro_stop_recording()); out_str(s) }
/// `exportObject()` as JSON text
#[no_mangle]
pub extern "C" fn wd_macro_export() -> usize { let s = with_game(|g| g.macro_export()); out_str(s) }
/// `startPlayback(json)`; 0 when it does not parse
#[no_mangle]
pub unsafe extern "C" fn wd_macro_play(ptr: *const u8, len: usize) -> u32 {
    let d = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    with_game(|g| g.macro_start_playback(&d)) as u32
}
#[no_mangle]
pub extern "C" fn wd_macro_stop_playback() { with_game(|g| g.macro_stop_playback()); }
/// 0 idle, 1 recording, 2 playing
#[no_mangle]
pub extern "C" fn wd_macro_state() -> u32 { with_game(|g| if g.mb.playing { 2 } else if g.mb.recording { 1 } else { 0 }) }
#[no_mangle]
pub extern "C" fn wd_macro_frames() -> usize { with_game(|g| g.mb.frames.len()) }
#[no_mangle]
pub extern "C" fn wd_game_start_pos_count() -> usize { with_game(|g| g.start_positions.len()) }
/// `_getSongOffsetForWorldX`
#[no_mangle]
pub extern "C" fn wd_game_song_offset(x: f64) -> f64 { with_game(|g| g.song_offset_for_world_x(x)) }
/// `_getStartPosMusicOffset`
#[no_mangle]
pub extern "C" fn wd_game_start_pos_music_offset() -> f64 { with_game(|g| g.start_pos_music_offset()) }
/// `_getCurrentMusicSyncOffset`
#[no_mangle]
pub extern "C" fn wd_game_music_sync_offset() -> f64 { with_game(|g| g.current_music_sync_offset()) }
#[no_mangle]
pub extern "C" fn wd_trig_ff(x: f64) { with_game(|g| g.fast_forward_triggers(x)); }

/// Whole-frame update (`Game::update`); returns 1 when the level restarted.
#[no_mangle]
pub extern "C" fn wd_game_update(delta_ms: f64, held: u32) -> usize {
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    gm.log.clear();
    let restarted = gm.update(delta_ms, held != 0);
    let mut o = OUT.lock().unwrap();
    *o = gm.log.join("|").into_bytes();
    ((restarted as usize) << 30) | o.len()
}
/// Scene values: 0 camera x, 1 camera y, 2 end camera override, 3 level won, 4 end portal y, 5 end x.
#[no_mangle]
pub extern "C" fn wd_game_scene(which: u32) -> f64 {
    let guard = GAME.lock().unwrap();
    let gm = guard.as_ref().unwrap();
    match which { 0 => gm.camera_x, 1 => gm.camera_y, 2 => gm.end_camera_override as u8 as f64, 3 => gm.level_won as u8 as f64, 4 => gm.end_portal_game_y, 5 => gm.end_x_pos, _ => f64::NAN }
}
#[no_mangle]
pub extern "C" fn wd_game_set_end(x: f64) { with_game(|g| g.end_x_pos = x); }

/// Practice mode: 0 = enter/leave practice mode (returns the new state), 1 = save checkpoint, 2 = delete last, 3 = respawn from checkpoint (returns log length).
#[no_mangle]
pub extern "C" fn wd_game_practice(which: u32) -> usize {
    let mut guard = GAME.lock().unwrap();
    let gm = guard.as_mut().unwrap();
    gm.log.clear();
    let r = match which {
        0 => { gm.practice_mode = !gm.practice_mode; if !gm.practice_mode { gm.checkpoints.clear(); } gm.practice_mode as usize }
        1 => gm.save_checkpoint() as usize,
        2 => gm.delete_last_checkpoint() as usize,
        _ => { gm.respawn_from_checkpoint(); let mut o = OUT.lock().unwrap(); *o = gm.log.join("|").into_bytes(); o.len() }
    };
    r
}

// ---- visual spawn test hooks ----
static ATLASES: Mutex<Option<visual::Atlases>> = Mutex::new(None);

/// Registers a Phaser JSON atlas by name; returns the number of frames.
#[no_mangle]
pub unsafe extern "C" fn wd_atlas_add(name_ptr: *const u8, name_len: usize, json_ptr: *const u8, json_len: usize) -> usize {
    let name = String::from_utf8_lossy(std::slice::from_raw_parts(name_ptr, name_len)).into_owned();
    let json = String::from_utf8_lossy(std::slice::from_raw_parts(json_ptr, json_len)).into_owned();
    let mut g = ATLASES.lock().unwrap();
    g.get_or_insert_with(Default::default).add(&name, &json).unwrap_or(0)
}

static SPRITE_RNG: Mutex<Option<u32>> = Mutex::new(None);
/// Seeds the generator `wd_visual_spawn` draws random numbers from (linear congruential, same as the test harness).
#[no_mangle]
pub extern "C" fn wd_visual_seed(seed: u32) { *SPRITE_RNG.lock().unwrap() = Some(seed); }

/// Spawns the visuals of object `index` of the level at ptr; the sprites of all objects spawned so far in this level are kept.
#[no_mangle]
pub unsafe extern "C" fn wd_visual_level(ptr: *const u8, len: usize) -> usize {
    let input = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    let level = level::parse_level(&input);
    let objects = level::normalize_teleport_portals(level.objects.clone());
    let guard = ATLASES.lock().unwrap();
    let atlases = guard.as_ref().expect("atlases");
    let mut rng = || { let mut g = SPRITE_RNG.lock().unwrap(); let s = g.get_or_insert(12345); *s = s.wrapping_mul(1664525).wrapping_add(1013904223); *s as f64 / 4294967296.0 };
    let mut b = visual::Builder::new(atlases, &mut rng);
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for o in &objects {
        let is_trigger = objects::is_trigger_branch(o.id.unwrap_or(0));
        if is_trigger { b.next_object_id += 1; ranges.push((0, 0)); continue; }
        let r = b.spawn(o);
        ranges.push((r.start, r.end));
    }
    let json = format!("{{\"ranges\":[{}],\"sprites\":{}}}", ranges.iter().map(|r| format!("[{},{}]", r.0, r.1)).collect::<Vec<_>>().join(","), visual::sprites_json(&b.sprites));
    out_str(json)
}

// ---- colour manager test hooks ----
static CM: Mutex<Option<colors::ColorManager>> = Mutex::new(None);
#[no_mangle]
pub extern "C" fn wd_cm_new(main: f64, secondary: f64) { *CM.lock().unwrap() = Some(colors::ColorManager::new(main, secondary)); }
/// One operation on the colour manager; `ptr` holds up to 16 f64 arguments.
/// 0 reset | 1 initial colour ch r g b | 2 initial copy ch source hsvFlag h s v sAdd vAdd | 3 initial opacity ch v |
/// 4 trigger ch r g b duration copySource hsvFlag h s v sAdd vAdd opacity(NaN none) | 5 step ms | 6 pulse ch r g b fadeIn hold fadeOut hsvFlag h s v sAdd vAdd copySource | 7 clear pulses
#[no_mangle]
pub unsafe extern "C" fn wd_cm_op(op: u32, ptr: *const f64) {
    let a = std::slice::from_raw_parts(ptr, 16);
    let hsv = |i: usize| if a[i] != 0.0 { Some(colors::Hsv { h: a[i + 1], s: a[i + 2], v: a[i + 3], s_add: a[i + 4] != 0.0, v_add: a[i + 5] != 0.0 }) } else { None };
    let rgb = |i: usize| colors::Rgb { r: a[i], g: a[i + 1], b: a[i + 2] };
    let mut g = CM.lock().unwrap();
    let cm = g.as_mut().unwrap();
    match op {
        0 => cm.reset(),
        1 => cm.set_initial_color(a[0] as i64, rgb(1)),
        2 => cm.set_initial_copy(a[0] as i64, colors::Copy { source: a[1], hsv: hsv(2) }),
        3 => cm.set_initial_opacity(a[0] as i64, a[1]),
        4 => cm.trigger_color(a[0] as i64, rgb(1), a[4], if a[5] > 0.0 { Some(colors::Copy { source: a[5], hsv: hsv(6) }) } else { None }, if a[12].is_nan() { None } else { Some(a[12]) }),
        5 => cm.step(a[0]),
        6 => cm.start_pulse(a[0], rgb(1), hsv(7), a[13], a[4], a[5], a[6]),
        _ => cm.clear_pulses(),
    }
}
/// Writes r, g, b, opacity, hex of channel `index` to `out`.
#[no_mangle]
pub unsafe extern "C" fn wd_cm_query(index: f64, out: *mut f64) {
    let g = CM.lock().unwrap();
    let cm = g.as_ref().unwrap();
    let o = std::slice::from_raw_parts_mut(out, 5);
    let c = cm.get_color(index as i64);
    o[0] = c.r; o[1] = c.g; o[2] = c.b; o[3] = cm.get_opacity(index as i64); o[4] = cm.get_hex(index as i64);
}

/// Tints of every sprite of a level under the colour manager `wd_cm_*` has been driven into: one f64 per sprite (NaN = untouched, else the tint).
#[no_mangle]
pub unsafe extern "C" fn wd_visual_tints(ptr: *const u8, len: usize) -> usize {
    let input = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    let level = level::parse_level(&input);
    let objects = level::normalize_teleport_portals(level.objects.clone());
    let guard = ATLASES.lock().unwrap();
    let atlases = guard.as_ref().expect("atlases");
    let mut rng = || { let mut g = SPRITE_RNG.lock().unwrap(); let s = g.get_or_insert(12345); *s = s.wrapping_mul(1664525).wrapping_add(1013904223); *s as f64 / 4294967296.0 };
    let mut b = visual::Builder::new(atlases, &mut rng);
    for o in &objects {
        if objects::is_trigger_branch(o.id.unwrap_or(0)) { b.next_object_id += 1; continue; }
        b.spawn(o);
    }
    let cm = CM.lock().unwrap();
    let cm = cm.as_ref().expect("colour manager");
    let vals: Vec<String> = b.sprites.iter().map(|s| match s.num("_eeColorChannel") {
        Some(ch) => match s.channel_tint(ch as i64, cm.get_hex(ch as i64), cm) { Some(t) => num(t), None => "null".into() },
        None => "null".into(),
    }).collect();
    out_str(format!("[{}]", vals.join(",")))
}

/// `_setUpSettings` of a level string: colours, copies, opacity, blending, background and ground, as JSON.
#[no_mangle]
pub unsafe extern "C" fn wd_level_settings(ptr: *const u8, len: usize) -> usize {
    let input = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    let level = level::parse_level(&input);
    out_str(settings::LevelSettings::parse(&level.settings).to_json())
}

/// Colour of channel `index` in the game's colour manager: r, g, b, opacity, hex.
#[no_mangle]
pub unsafe extern "C" fn wd_trig_cm(index: f64, out: *mut f64) {
    with_game(|g| {
        let o = std::slice::from_raw_parts_mut(out, 5);
        let c = g.cm.get_color(index as i64);
        o[0] = c.r; o[1] = c.g; o[2] = c.b; o[3] = g.cm.get_opacity(index as i64); o[4] = g.cm.get_hex(index as i64);
    });
}

// ---- ground test hooks ----
static GROUND: Mutex<Option<ground::Ground>> = Mutex::new(None);
#[no_mangle]
pub extern "C" fn wd_ground_new(screen_width: f64, tile_w: f64) { *GROUND.lock().unwrap() = Some(ground::Ground::new(screen_width, tile_w)); }
/// 0 setFlyMode(enabled, centreY, height, visualOnly, cameraHeight) | 1 step(dt) | 2 shift(amount) | 3 resetTiles(cameraX) | 4 resetState
#[no_mangle]
pub unsafe extern "C" fn wd_ground_op(op: u32, ptr: *const f64) {
    let a = std::slice::from_raw_parts(ptr, 8);
    let mut g = GROUND.lock().unwrap();
    let gr = g.as_mut().unwrap();
    match op {
        0 => gr.set_fly_mode(a[0] != 0.0, a[1], a[2], a[3] != 0.0, a[4], a[5]),
        1 => gr.step_animation(a[0]),
        2 => gr.shift(a[0]),
        3 => gr.reset_tiles(a[0]),
        _ => gr.reset_state(),
    }
}
/// `updateGroundTiles`; returns JSON of the frame plus the mutable state.
#[no_mangle]
pub unsafe extern "C" fn wd_ground_update(ptr: *const f64) -> usize {
    let a = std::slice::from_raw_parts(ptr, 8);
    let mut g = GROUND.lock().unwrap();
    let gr = g.as_mut().unwrap();
    let f = gr.update(a[0], a[1], a[2], a[3], a[4] != 0.0);
    let n = |v: f64| json::num(v);
    let list = |v: &[f64]| v.iter().map(|x| n(*x)).collect::<Vec<_>>().join(",");
    out_str(format!(
        "{{\"tileX\":[{}],\"worldX\":[{}],\"groundY\":{},\"ceilingY\":{},\"ceilVis\":{},\"lineX\":{},\"g2\":{},\"c2\":{},\"max\":{},\"shift\":{},\"target\":{},\"active\":{},\"anim\":{},\"floorY\":{},\"camT\":{}}}",
        list(&f.tile_x), list(&gr.world_x), n(f.ground_y), n(f.ceiling_y), f.ceiling_visible, n(f.line_x), f.ground2_visible, f.ceiling2_visible,
        n(gr.max_world_x), n(gr.ground_shift), n(gr.ground_target), gr.fly_ground_active, gr.animating, n(gr.floor_y()), gr.fly_camera_target.map(n).unwrap_or("null".into())
    ))
}

// ---- background test hooks ----
static BG: Mutex<Option<background::Background>> = Mutex::new(None);
/// op 0: new | 1: apply_texture(a0) | 2: update(a0 cameraX, a1 cameraY, a2 mirror) -> writes tileX, tileY, shift to a3..a5 (as out)
#[no_mangle]
pub unsafe extern "C" fn wd_bg_op(op: u32, ptr: *mut f64) {
    let a = std::slice::from_raw_parts_mut(ptr, 8);
    let mut g = BG.lock().unwrap();
    match op {
        0 => *g = Some(background::Background::new()),
        1 => g.as_mut().unwrap().apply_texture(a[0]),
        _ => { let bg = g.as_mut().unwrap(); let s = bg.update(a[0], a[1], a[2]); a[3] = bg.tile_x; a[4] = bg.tile_y; a[5] = s; }
    }
}
/// Art-id helpers on a string: 0 backgroundTextureIndex | 1 backgroundDisplayId | 2 groundTextureId (`len` < 0: undefined).
#[no_mangle]
pub unsafe extern "C" fn wd_art_id(which: u32, ptr: *const u8, len: isize) -> usize {
    let s = if len < 0 { None } else { Some(String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len as usize)).into_owned()) };
    let r = match which {
        0 => background::background_texture_index(s.as_deref()).to_string(),
        1 => background::background_display_id(s.as_deref()),
        _ => background::ground_texture_id(s.as_deref()),
    };
    out_str(r)
}

#[no_mangle]
pub extern "C" fn wd_trig_time(now_ms: f64) { with_game(|g| g.trig.now_ms = now_ms); }
#[no_mangle]
pub extern "C" fn wd_trig_shake(now_ms: f64) { with_game(|g| g.apply_shake(now_ms)); }
/// Test hook: the scene's `_physicsTick` (the shake runs on it).
#[no_mangle]
pub extern "C" fn wd_trig_tick(tick: f64) { with_game(|g| g.e.physics_tick = tick); }

/// Spawns the level's sprites (when atlases are loaded) and hands them to the game.
/// `window.enableLDM`, set by the host before a level is built
static LDM: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const LDM_OBJECTS: [i64; 37] = [503, 505, 504, 1888, 1887, 1012, 1013, 1011, 1886, 1762, 1763, 1761, 1269, 1760, 1270, 1271, 1274, 1272, 1273, 1764, 1765, 1766, 1767, 1769, 1292, 1758, 1291, 1293, 1759, 1051, 1058, 1060, 1752, 1050, 1052, 1059, 1061];
#[no_mangle]
pub extern "C" fn wd_set_ldm(v: u32) { LDM.store(v != 0, std::sync::atomic::Ordering::Relaxed); if let Some(g) = GAME.lock().unwrap().as_mut() { g.ldm = v != 0; } }
fn attach_level_sprites(level: &level::Level, g: &mut game::Game, official: u32) { attach_level_sprites_ed(level, g, official, false) }

fn attach_level_sprites_ed(level: &level::Level, g: &mut game::Game, official: u32, editor: bool) {
    let guard = ATLASES.lock().unwrap();
    let Some(atlases) = guard.as_ref() else { return };
    let objects = level::filter_coins(level::normalize_teleport_portals(level.objects.clone()), official);
    let mut rng = || { let mut s = SPRITE_RNG.lock().unwrap(); let v = s.get_or_insert(12345); *v = v.wrapping_mul(1664525).wrapping_add(1013904223); *v as f64 / 4294967296.0 };
    let mut b = visual::Builder::new(atlases, &mut rng);
    b.pad_emitters = true;
    let ldm = LDM.load(std::sync::atomic::Ordering::Relaxed);
    b.ldm = ldm; g.ldm = ldm;
    b.channel_blending = g.settings.channel_blending.clone();
    b.legacy_color_level = g.settings.legacy_color_level;
    b.secret_done = g.coins.secret_done.clone();
    b.user_done = g.coins.user_done.clone();
    let mut invisible: Vec<(f64, Vec<usize>)> = Vec::new();
    let mut obj_ranges: Vec<(u32, u32)> = Vec::new();
    for o in &objects {
        // level.js `ldm_object`: Low Detail Mode leaves these decorations out
        if ldm && LDM_OBJECTS.contains(&(o.id.unwrap_or(0))) { let n = b.sprites.len() as u32; obj_ranges.push((n, n)); continue; }
        let is_trigger = objects::is_trigger_branch(o.id.unwrap_or(0));
        if is_trigger {
            let linked = b.next_object_id;
            b.next_object_id += 1;
            let start = b.sprites.len();
            if editor { editor_trigger_sprite(atlases, &mut b.sprites, o); }
            // the text object (id 914) is a "trigger" that draws a bitmap text
            else if let Some(def) = visual::def_of(o.id.unwrap_or(0)) { if def.get_bool("textObject") == Some(true) { b.spawn_text(o, def, linked); } }
            obj_ranges.push((start as u32, b.sprites.len() as u32));
            continue;
        }
        let before = b.emitters.len();
        let r = b.spawn(o);
        obj_ranges.push((r.start as u32, r.end as u32));
        // the colliders of this object come after its sprites: the next pad sprite sees the last of them
        if collider::spawn_colliders(o).is_some() { // objects without a collider still push a placeholder entry
            b.prev_pad_emitter = if b.emitters.len() > before { b.emitters.last().copied() } else { None };
        }
        let inv: Vec<usize> = (r.start..r.end).filter(|&i| b.sprites[i].props.contains_key("_eeApproachAlpha")).collect();
        if !inv.is_empty() { invisible.push((o.x * 2.0, inv)); }
    }
    let (sprites, groups, channels, emitters, lists) = (std::mem::take(&mut b.sprites), std::mem::take(&mut b.group_sprites), std::mem::take(&mut b.color_channel_sprites), std::mem::take(&mut b.emitters), std::mem::take(&mut b.lists));
    let mut anim_names: std::collections::BTreeMap<usize, Vec<Option<(String, String)>>> = Default::default();
    for &si in &lists.animated {
        if let Some(visual::Val::List(names)) = sprites[si].props.get("_animFrames") {
            anim_names.insert(si, names.iter().map(|n| atlases.find(n).map(|(a, _)| (a.to_string(), n.clone()))).collect());
        }
    }
    let beasts = std::mem::take(&mut b.beasts);
    drop(b);
    g.beasts = beast::Beasts::default();
    for (bi, bst) in beasts.iter().enumerate() { for (_, si) in &bst.parts { g.beasts.of_sprite.insert(*si, bi); } }
    g.beasts.list = beasts;
    g.attach_sprites(sprites, groups, invisible, channels, emitters, lists, anim_names);
    g.fx.obj_ranges = obj_ranges;
    // both looks of every secret/user coin (a finished level swaps them)
    let mut coin_names = Vec::new();
    for &si in &g.fx.lists.coins.clone() {
        let s = &g.fx.sprites[si];
        let user = s.num("_userCoinId").is_some();
        if !user && s.num("_secretCoinId").is_none() { continue; }
        let (a, b) = if user { (coins::USER_A, coins::USER_B) } else { (coins::SECRET_A, coins::SECRET_B) };
        let res = |l: [&str; 4]| -> Vec<Option<(String, String)>> { l.iter().map(|n| atlases.find(n).map(|(at, _)| (at.to_string(), n.to_string()))).collect() };
        coin_names.push((si, res(a), res(b)));
    }
    g.attach_coin_frames(coin_names);
}

/// JSON of every sprite's mutable state: `[x, y, rotation, scaleX, scaleY, alpha, visible, active]`.
#[no_mangle]
pub extern "C" fn wd_fx_sprites() -> usize {
    out_str(with_game(|g| {
        let rows: Vec<String> = g.fx.sprites.iter().enumerate().map(|(i, s)| format!("[{},{},{},{},{},{},{},{},{},{},{}]", json::num(s.x), json::num(s.y), json::num(s.rotation), json::num(s.scale_x), json::num(s.scale_y), json::num(s.alpha), s.visible, s.active, s.tint.map_or("null".to_string(), json::num), s.blend, g.fx.sprite_frame[i])).collect();
        format!("[{}]", rows.join(","))
    }))
}
/// `_groupSprites`: `{group: [sprite index...]}`
#[no_mangle]
pub extern "C" fn wd_fx_groups() -> usize {
    out_str(with_game(|g| {
        let rows: Vec<String> = g.fx.group_sprites.iter().map(|(k, v)| format!("\"{}\":[{}]", k, v.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(","))).collect();
        format!("{{{}}}", rows.join(","))
    }))
}
#[no_mangle]
pub extern "C" fn wd_fx_enter(camera_x: f64, player_x: f64) { with_game(|g| { g.fx.player_x = player_x; g.apply_color_channels(); g.apply_enter_effects(camera_x); }); }
/// `updateVisibility` (called before the pulse step of a frame)
#[no_mangle]
pub extern "C" fn wd_fx_visibility(camera_x: f64) { with_game(|g| g.update_visibility(camera_x)); }
/// Invisible-block fade entries `[x, sprites, alpha]` (tests)
#[no_mangle]
pub extern "C" fn wd_fx_invisible() -> usize {
    out_str(with_game(|g| format!("[{}]", g.fx.invisible.iter().map(|e| format!("[{},{},{}]", json::num(e.x), e.sprites.len(), json::num(e.alpha))).collect::<Vec<_>>().join(","))))
}
/// One sprite with all props (tests)
#[no_mangle]
pub extern "C" fn wd_fx_sprite(i: usize) -> usize {
    out_str(with_game(|g| visual::sprites_json(&g.fx.sprites[i..i + 1])))
}

static OFFICIAL: Mutex<u32> = Mutex::new(2);
/// Whether the level counts as official (`_isOfficialLevel`): decides which coins it keeps (0 = no, 1 = yes, anything else = keep every coin).
#[no_mangle]
pub extern "C" fn wd_set_official(flag: u32) { *OFFICIAL.lock().unwrap() = flag; }

#[no_mangle]
pub extern "C" fn wd_game_after_load() { with_game(|g| g.after_load()); }
/// `_getGroupCenter` of a group (tests): writes cx, cy
#[no_mangle]
pub unsafe extern "C" fn wd_fx_center(gid: i64, out: *mut f64) {
    let (cx, cy) = with_game(|g| g.group_center_pub(gid));
    let o = std::slice::from_raw_parts_mut(out, 2);
    o[0] = cx; o[1] = cy;
}

// ---- renderer hooks ----
/// Tells the game about the textures (see `Game::render_setup`).
#[no_mangle]
pub extern "C" fn wd_render_setup(ground_tile_w: f64, has_ground2: u32, bg_height: f64) { with_game(|g| g.render_setup(ground_tile_w, has_ground2 != 0, bg_height)); }
/// Builds the frame buffer; returns its length in f64s. Read it at `wd_render_ptr()`.
#[no_mangle]
pub extern "C" fn wd_render_build() -> usize { with_game(|g| g.build_render()) }
#[no_mangle]
pub extern "C" fn wd_render_ptr() -> *const f64 { with_game(|g| g.fx.render_buf.as_ptr()) }
/// JSON `[[atlas, frame], ...]` of the frame table (indexes in the sprite records).
#[no_mangle]
pub extern "C" fn wd_render_frames() -> usize {
    out_str(with_game(|g| format!("[{}]", g.fx.frames.iter().map(|(a, f)| format!("[{},{}]", json::str_lit(a), json::str_lit(f))).collect::<Vec<_>>().join(","))))
}
/// JSON with the level's background/ground ids and start colours.
#[no_mangle]
pub extern "C" fn wd_render_info() -> usize {
    out_str(with_game(|g| format!("{{\"background\":{},\"ground\":{},\"main\":{},\"secondary\":{},\"endX\":{},\"screenWidth\":{},\"musicOffset\":{}}}",
        g.settings.background_id.as_ref().map_or("null".to_string(), |s| json::str_lit(s)), g.settings.ground_id.as_ref().map_or("null".to_string(), |s| json::str_lit(s)),
        327424, 64511, json::num(g.end_x_pos), json::num(g.screen_width), json::num(crate::js::to_number(g.settings.map.get("kA13").map_or("0", |s| s.as_str())).max(0.0)))))
}

/// The music's metering value (0..1.2) the audio-reactive sprites follow.
#[no_mangle]
pub extern "C" fn wd_set_audio_meter(v: f64) { with_game(|g| g.fx.audio_meter = v); }

/// The sprite animation block of `update` (tests): texture swaps, saws, orb guides, audio scale.
#[no_mangle]
pub extern "C" fn wd_fx_animate(delta_ms: f64, camera_x: f64, gravity_flipped: u32, meter: f64) {
    with_game(|g| { g.camera_x = camera_x; g.p.gravity_flipped = gravity_flipped != 0; g.fx.audio_meter = meter; g.animate_sprites(delta_ms); });
}

static METER: Mutex<Option<audio::Meter>> = Mutex::new(None);
/// Meter test hook: op 0 = reset, 1 = update(a0 seconds, a1 peak, a2 volume) -> value
#[no_mangle]
pub unsafe extern "C" fn wd_meter(op: u32, ptr: *mut f64) {
    let a = std::slice::from_raw_parts_mut(ptr, 4);
    let mut g = METER.lock().unwrap();
    if op == 0 || g.is_none() { *g = Some(audio::Meter::new()); }
    if op == 1 { let mt = g.as_mut().unwrap(); mt.update(a[0], a[1], a[2]); a[3] = mt.value; }
}
/// Feeds the game's meter with the analyser's loudest sample and the music volume; call once per alive frame before `wd_game_update`.
#[no_mangle]
pub extern "C" fn wd_set_audio_peak(peak: f64, volume: f64) { with_game(|g| { g.fx.audio_peak = (peak, volume); }); }

/// Let the game derive the audio meter from the analyser peaks (`wd_set_audio_peak`) like the original does.
#[no_mangle]
pub extern "C" fn wd_audio_live(on: u32) { with_game(|g| g.fx.meter_live = on != 0); }

// ---- menus / UI ----
static UI: Mutex<Option<ui::Ui>> = Mutex::new(None);
fn with_ui<R>(f: impl FnOnce(&mut ui::Ui) -> R) -> R { let mut g = UI.lock().unwrap(); f(g.as_mut().unwrap()) }
/// Creates the UI. `sizes` JSON: {"atlas/frame": [w, h]} for textures that are not in the atlases.
#[no_mangle]
pub extern "C" fn wd_ui_new(screen_width: f64) {
    let mut u = ui::Ui::new(screen_width);
    if let Some(a) = ATLASES.lock().unwrap().as_ref() {
        for (name, sheet) in &a.sheets { for (f, d) in sheet { u.sizes.insert(format!("{}/{}", name, f), (d.real_w, d.real_h)); } }
    }
    *UI.lock().unwrap() = Some(u);
}
#[no_mangle]
pub unsafe extern "C" fn wd_ui_size(ptr: *const u8, len: usize, w: f64, h: f64) {
    let key = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    with_ui(|u| { u.sizes.insert(key, (w, h)); });
}
#[no_mangle]
pub extern "C" fn wd_ui_main_menu() { with_ui(|u| u.build_main_menu()); }
#[no_mangle]
pub extern "C" fn wd_ui_pointer(kind: u32, x: f64, y: f64) { with_ui(|u| match kind { 0 => u.pointer_down(x, y), 1 => u.pointer_move(x, y), _ => u.pointer_up(x, y) }); }
#[no_mangle]
pub extern "C" fn wd_ui_step(dt_ms: f64) { with_ui(|u| u.step(dt_ms)); }
/// Pops the next UI event as an index (0 none) — see `wd_ui_event_name`.
#[no_mangle]
pub extern "C" fn wd_ui_event() -> usize {
    with_ui(|u| if u.events.is_empty() { 0 } else { let a = u.events.remove(0); out_str(match a { ui::Action::OpenUrl(s) => format!("url:{}", s), other => format!("{:?}", other) }) })
}
/// Saved options the settings screen shows: music volume, sfx volume, menu-music checkbox.
#[no_mangle]
pub extern "C" fn wd_ui_set_options(music: f64, sfx: f64, menu_music: u32) { with_ui(|u| { u.music_vol = music; u.sfx_vol = sfx; u.menu_music = menu_music != 0; }); }
#[no_mangle]
pub unsafe extern "C" fn wd_ui_opt_set(ptr: *const u8, len: usize, v: f64) { let k = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned(); with_ui(|u| { u.opts.insert(k, v); }); }
/// all saved options as a JSON object
#[no_mangle]
pub extern "C" fn wd_ui_opts_json() -> usize { out_str(with_ui(|u| { let mut k: Vec<_> = u.opts.iter().collect(); k.sort_by(|a, b| a.0.cmp(b.0)); format!("{{{}}}", k.iter().map(|(k, v)| format!("{}:{}", json::str_lit(k), v)).collect::<Vec<_>>().join(",")) })) }
#[no_mangle]
pub extern "C" fn wd_ui_wheel(x: f64, y: f64, dy: f64) { with_ui(|u| u.wheel(x, y, dy)); }
#[no_mangle]
pub extern "C" fn wd_ui_open_options() { with_ui(|u| u.open_options()); }
#[no_mangle]
pub extern "C" fn wd_ui_key(code: u32) { with_ui(|u| u.key_input(code)); }
#[no_mangle]
pub extern "C" fn wd_ui_open_settings() { with_ui(|u| u.open_settings()); }
#[no_mangle]
pub extern "C" fn wd_ui_option(which: u32) -> f64 { with_ui(|u| match which { 0 => u.music_vol, 1 => u.sfx_vol, _ => u.menu_music as u32 as f64 }) }
#[no_mangle]
pub extern "C" fn wd_ui_start_game() { with_ui(|u| u.start_game_dismiss()); }
/// Builds the draw buffer; returns the node count. Layout: 16 numbers per node (see `Ui::build`). Read at `wd_ui_ptr`, texts via `wd_ui_texts`.
#[no_mangle]
pub extern "C" fn wd_ui_build(frames_known: usize) -> usize { let _ = frames_known; with_ui(|u| { let mut f = UI_FRAMES.lock().unwrap(); u.build(&mut f) }) }
static UI_FRAMES: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
#[no_mangle]
pub extern "C" fn wd_ui_ptr() -> *const f64 { with_ui(|u| u.buf.as_ptr()) }
#[no_mangle]
pub extern "C" fn wd_ui_texts() -> usize { out_str(with_ui(|u| format!("[{}]", u.strings.iter().map(|s| json::str_lit(s)).collect::<Vec<_>>().join(",")))) }
/// `[[atlas, frame], ...]` table that image nodes index into
#[no_mangle]
pub extern "C" fn wd_ui_frames_len() -> usize { UI_FRAMES.lock().unwrap().len() }
#[no_mangle]
pub extern "C" fn wd_ui_frames() -> usize { out_str(format!("[{}]", UI_FRAMES.lock().unwrap().iter().map(|(a, f)| format!("[{},{}]", json::str_lit(a), json::str_lit(f))).collect::<Vec<_>>().join(","))) }
#[no_mangle]
pub extern "C" fn wd_ui_menu_active() -> u32 { with_ui(|u| u.menu_active as u32) }

/// Menu mode on/off (the level scrolls behind the menu); turning it off starts nothing by itself.
/// Press-play slide-in (instead of an immediate restart).
#[no_mangle]
pub extern "C" fn wd_game_begin_slide() { with_game(|g| { g.fx.menu_mode = false; g.begin_slide(); }); }
/// 0 = no slide, 1 = sliding, 2 = just finished (reading resets it)
#[no_mangle]
pub extern "C" fn wd_game_slide_state() -> u32 { with_game(|g| if g.fx.slide.is_some() { 1 } else if g.fx.slide_done { g.fx.slide_done = false; 2 } else { 0 }) }
#[no_mangle]
pub extern "C" fn wd_menu_mode(on: u32) { with_game(|g| { g.fx.menu_mode = on != 0; if on != 0 { g.fx.menu_camera_x = -g.g.center_x; g.camera_x = g.fx.menu_camera_x; g.camera_y = 0.0; if let Some(gr) = g.fx.ground.as_mut() { gr.reset_state(); } // the scene restart of the original: no raised camera, no ship floor and ceiling
             g.fx.bg.prev_camera_x = g.camera_x; } }); }

/// Level list for the level select: JSON `[[internal, display, id, song?], ...]`; `page` selects the shown one.
#[no_mangle]
pub unsafe extern "C" fn wd_ui_levels(ptr: *const u8, len: usize, page: usize) {
    let json_s = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    let mut levels = Vec::new();
    if let Ok(j) = jsonread::parse(&json_s) {
        if let Some(a) = j.arr() { for e in a { if let Some(r) = e.arr() {
            let g = |i: usize| r.get(i).and_then(|v| v.str()).unwrap_or("").to_string();
            levels.push(ui::LevelInfo { name: g(0), display: g(1), id: g(2), song: r.get(3).and_then(|v| v.str()).map(String::from) });
        } } }
    }
    with_ui(|u| { u.levels = levels; u.page = page; });
}
#[no_mangle]
pub unsafe extern "C" fn wd_ui_font(name_ptr: *const u8, name_len: usize, ptr: *const u8, len: usize) {
    let name = String::from_utf8_lossy(std::slice::from_raw_parts(name_ptr, name_len)).into_owned();
    let data = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    with_ui(|u| { u.fonts.insert(name, ui::Font::parse(&data)); });
}
/// Saved progress of a level: normal %, practice %, coin bits (1, 2, 4)
#[no_mangle]
pub unsafe extern "C" fn wd_ui_progress(ptr: *const u8, len: usize, normal: f64, practice: f64, coins: u32) {
    let id = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    with_ui(|u| { u.progress.insert(id, ui::Progress { normal, practice, coins: [coins & 1 != 0, coins & 2 != 0, coins & 4 != 0] }); });
}
#[no_mangle]
pub extern "C" fn wd_ui_ground(tile_w: f64, ref_h: f64, has2: u32, total_coins: f64) { with_ui(|u| { u.ground_tile = (tile_w, ref_h); u.has_ground2 = has2 != 0; u.total_coins = total_coins; }); }
#[no_mangle]
pub extern "C" fn wd_ui_open_level_select() { with_ui(|u| u.open_level_select()); }
#[no_mangle]
pub extern "C" fn wd_ui_close_level_select() { with_ui(|u| u.close_level_select()); }
#[no_mangle]
pub extern "C" fn wd_ui_switch(dir: i32) { with_ui(|u| u.switch_level(dir as i64)); }
#[no_mangle]
pub extern "C" fn wd_ui_page() -> usize { with_ui(|u| u.page) }

#[no_mangle]
pub extern "C" fn wd_ui_hud() { with_ui(|u| u.build_hud()); }
#[no_mangle]
pub extern "C" fn wd_ui_apply_hud() { with_ui(|u| u.apply_hud_options()); }
#[no_mangle]
pub extern "C" fn wd_ui_hud_update(percent: f64) { with_ui(|u| u.set_percent(percent)); }
#[no_mangle]
pub extern "C" fn wd_ui_attempt(n: u32, camera_x: f64) { with_ui(|u| u.set_attempt(n, camera_x)); }
#[no_mangle]
pub extern "C" fn wd_ui_practice(on: u32) { with_ui(|u| { u.practice = on != 0; u.set_practice_ui(on != 0); }); }
#[no_mangle]
pub unsafe extern "C" fn wd_ui_pause(open: u32, ptr: *const u8, len: usize, normal: f64, practice: f64) {
    let name = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    with_ui(|u| { if open != 0 { u.level_name = name; u.best_normal = normal; u.best_practice = practice; u.open_pause(); } else { u.close_pause(); } });
}

#[no_mangle]
pub extern "C" fn wd_ui_main_menu_again() { with_ui(|u| u.reset_to_main_menu()); }
/// Progress values: 0 last death percent, 1 best percent of this run, 2 level won
#[no_mangle]
pub extern "C" fn wd_game_percent(which: u32) -> f64 { with_game(|g| match which { 0 => g.last_percent, 1 => g.best_percent, _ => g.level_won as u8 as f64 }) }

#[no_mangle]
pub extern "C" fn wd_ui_banner(practice: u32) { with_ui(|u| u.show_complete_banner(practice != 0)); }
#[no_mangle]
pub unsafe extern "C" fn wd_ui_end_layer(attempts: u32, jumps: u32, seconds: f64, practice: u32, ptr: *const u8, len: usize) {
    let msg = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    with_ui(|u| { u.end_clock = 0.0; u.open_end_layer(attempts, jumps, seconds, practice != 0, &msg); });
}
/// Coins of the level for the end screen: main level (1) or not, how many, bits collected this run, bits saved before
#[no_mangle]
pub extern "C" fn wd_ui_end_coins(main: u32, count: u32, run_mask: u32, saved_mask: u32) { with_ui(|u| u.set_end_coins(main != 0, count, run_mask, saved_mask)); }
#[no_mangle]
pub extern "C" fn wd_ui_hud_pause(show: u32) { with_ui(|u| u.hud_pause_fade(show != 0)); }
#[no_mangle]
pub extern "C" fn wd_game_coin_count(main: u32) -> u32 { with_game(|g| g.coin_count(main != 0)) }
#[no_mangle]
pub extern "C" fn wd_ui_close_end() { with_ui(|u| u.close_end_layer()); }


// ---- creator menu / My Levels host API ----
#[no_mangle]
pub unsafe extern "C" fn wd_em_levels(ptr: *const u8, len: usize) { let j = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned(); with_ui(|u| u.em_set_levels(&j)); }
#[no_mangle]
pub extern "C" fn wd_em_open_my_levels() { with_ui(|u| u.em_open_my_levels()); }
#[no_mangle]
pub unsafe extern "C" fn wd_em_open_new(ptr: *const u8, len: usize) { let j = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned(); with_ui(|u| u.em_open_new(&j)); }
/// Closes the creator screens (level view, My Levels, creator menu), as when a created level starts.
#[no_mangle]
pub extern "C" fn wd_em_close() { with_ui(|u| { u.kill_tag("cm_"); u.kill_tag("my_"); u.kill_tag("lv_"); u.em.view = None; u.em.input = 0; }); }
#[no_mangle]
pub extern "C" fn wd_em_after_delete() { with_ui(|u| u.em_after_delete()); }
#[no_mangle]
pub extern "C" fn wd_em_key(ch: u32) { with_ui(|u| u.em_key(ch)); }
/// createdId of the level the view shows (and of the n-th row for `EmView`)
#[no_mangle]
pub extern "C" fn wd_em_view_id() -> usize { out_str(with_ui(|u| u.em.view.map(|i| u.em_created_id(i)).unwrap_or_default())) }
#[no_mangle]
pub extern "C" fn wd_em_field(which: u32) -> usize { out_str(with_ui(|u| u.em_field(which))) }
#[no_mangle]
pub extern "C" fn wd_em_open_creator() { with_ui(|u| u.em_open_creator()); }
#[no_mangle]
pub extern "C" fn wd_ui_ready() -> u32 { UI.lock().map(|g| g.is_some() as u32).unwrap_or(0) }

/// The icon the editor shows for a trigger (`_spawnTriggerEditorVisual`: first of the candidate frames that exists).
fn editor_trigger_sprite(atlases: &visual::Atlases, sprites: &mut Vec<visual::Sprite>, o: &level::LevelObject) {
    let id = o.id.unwrap_or(0);
    let Some(d) = visual::def_of(id) else { return };
    let mut cands: Vec<String> = Vec::new();
    if let Some(f) = d.get_str("frame") { cands.push(f.to_string()); }
    if let Some(f) = d.get("randomFrames").and_then(jsonread::Json::arr).and_then(|a| a.first()).and_then(|v| v.str()) { cands.push(f.to_string()); }
    if let Some(f) = d.get_str("editorFrame") { cands.push(f.to_string()); }
    if let Some(f) = d.get_str("icon") { cands.push(f.to_string()); }
    if id > 0 { cands.extend([format!("trigger_{}_001.png", id), format!("edit_trigger_{}_001.png", id), format!("edit_e{}_001.png", id), format!("object_{}_001.png", id)]); }
    for f in cands {
        if f.is_empty() { continue; }
        if let Some((a, _)) = atlases.find(&f) {
            let mut s = visual::Sprite::new(a, &f, o.x * 2.0, 460.0 - o.y * 2.0);
            s.alpha = 0.95;
            s.rotation = o.rot.to_radians();
            s.flip_x = o.flip_x; s.flip_y = o.flip_y;
            s.set_num("_eeLayer", 2.0); s.set_num("_eeZDepth", 995.0);
            s.set_num("_eeWorldX", o.x * 2.0); s.set_num("_eeBaseY", 460.0 - o.y * 2.0);
            sprites.push(s);
            return;
        }
    }
}

pub(crate) fn atlas_for_frame(frame: &str) -> Option<String> { ATLASES.lock().ok()?.as_ref()?.find(frame).map(|(a, _)| a.to_string()) }

/// Rebuilds the editor's view game from the editor level (keeps the ground/background set up by the host).
/// Bumped whenever the editor view is rebuilt: its table of sprite images (`wd_render_frames`) is a new one.
static ED_FRAMES_VER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
#[no_mangle]
pub extern "C" fn wd_ed_frames_ver() -> u32 { ED_FRAMES_VER.load(std::sync::atomic::Ordering::Relaxed) }

pub(crate) fn editor_rebuild(level: &level::Level) {
    ED_FRAMES_VER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let old = GAME.lock().unwrap().take();
    let mut g = game::Game::from_level_official(level, 2);
    attach_level_sprites_ed(level, &mut g, 2, true);
    if let Some(o) = old { g.fx.ground = o.fx.ground; g.fx.bg = o.fx.bg; g.fx.has_ground2 = o.fx.has_ground2; g.g.center_x = o.g.center_x; }
    g.after_load();
    g.fx.editor_mode = true;
    *GAME.lock().unwrap() = Some(g);
}

// ---- level editor host API ----
#[no_mangle]
pub unsafe extern "C" fn wd_ed_open(ptr: *const u8, len: usize, id_ptr: *const u8, id_len: usize) {
    let s = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    let id = String::from_utf8_lossy(std::slice::from_raw_parts(id_ptr, id_len)).into_owned();
    with_ui(|u| u.ed_open(&s, &id));
}
#[no_mangle]
pub extern "C" fn wd_ed_close() { with_ui(|u| u.ed_close()); }
#[no_mangle]
pub extern "C" fn wd_ed_frame(dt: f64, keys: u32) { with_ui(|u| u.ed_frame(dt, keys)); }
/// Playtest state for the host: 0 event counter (changes on start / stop / pause / resume), 1 active, 2 paused, 3 start position x (for the music offset).
#[no_mangle]
pub extern "C" fn wd_ed_pt(which: u32) -> f64 { with_ui(|u| match which { 0 => u.ed.pt_event as f64, 1 => u.ed.pt as u8 as f64, 2 => u.ed.pt_paused as u8 as f64, _ => u.ed.pt_music }) }
/// The names of the official songs, `["Stereo Madness", ...]` (`window.allLevels[i][1]`), and the song id of the level being edited.
#[no_mangle]
pub unsafe extern "C" fn wd_ed_set_songs(ptr: *const u8, len: usize, song_id: f64) {
    let s = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned();
    let names: Vec<String> = jsonread::parse(&s).ok().and_then(|j| j.arr().map(|a| a.iter().filter_map(|v| v.str().map(String::from)).collect())).unwrap_or_default();
    with_ui(|u| { u.ed.songs = names; u.ed.song_id = song_id as i64; });
}
/// The song picked in the level settings: id (NaN when none) and its name.
#[no_mangle]
pub extern "C" fn wd_ed_pending_song() -> f64 { with_ui(|u| u.ed.pending_song.as_ref().map_or(f64::NAN, |p| p.0 as f64)) }
#[no_mangle]
pub extern "C" fn wd_ed_pending_song_name() -> usize { out_str(with_ui(|u| u.ed.pending_song.as_ref().map(|p| p.1.clone()).unwrap_or_default())) }
#[no_mangle]
pub extern "C" fn wd_ed_clear_pending_song() { with_ui(|u| u.ed.pending_song = None); }
/// Pixels of the colour picker artwork: 0 hue ring (252 x 252), 1 disc background, 2 shadow, 3 overlay (185 x 185), RGBA, sent by the host.
#[no_mangle]
pub unsafe extern "C" fn wd_ed_cp_tex(which: u32, ptr: *const u8, len: usize) { let v = std::slice::from_raw_parts(ptr, len).to_vec(); with_ui(|u| u.ed_cp_set_tex(which as usize, v)); }
/// The colour the picker shows (r, g, b packed as 0xRRGGBB), -1 when it is closed.
#[no_mangle]
pub extern "C" fn wd_ed_cp_color() -> f64 { with_ui(|u| if u.ed.cp.open { ((u.ed.cp.color.0 << 16) | (u.ed.cp.color.1 << 8) | u.ed.cp.color.2) as f64 } else { -1.0 }) }
/// Menu keyboard shortcuts: 1 when the key was used by the menu (`code` is a `KeyboardEvent.code`).
#[no_mangle]
pub unsafe extern "C" fn wd_ui_menu_key(ptr: *const u8, len: usize) -> u32 { let c = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned(); with_ui(|u| u.menu_key(&c)) as u32 }
/// Whether `wd_ui_menu_key` would use the key, without using it (the resume after a pause waits 75 ms first).
#[no_mangle]
pub unsafe extern "C" fn wd_ui_menu_key_probe(ptr: *const u8, len: usize) -> u32 { let c = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned(); with_ui(|u| u.menu_key_probe(&c)) as u32 }
/// Editor keybinds: 0..2 tabs, 3..6 move the selection (up, left, down, right); `big` moves 60 instead of 1.
#[no_mangle]
pub extern "C" fn wd_ed_key_action(which: u32, big: u32) { with_ui(|u| u.ed_key_action(which, big != 0)); }
/// `window.showHitboxes` / `showHitboxTrail` / `hitboxesOnDeath`
#[no_mangle]
pub extern "C" fn wd_game_set_hitbox_opts(show: u32, trail: u32, on_death: u32) { with_game(|g| { g.show_hitboxes = show != 0; g.hitbox_trail_on = trail != 0; g.hitboxes_on_death = on_death != 0; if trail == 0 { g.clear_hitbox_trails(); } }); }
static HB_BUF: Mutex<Vec<f64>> = Mutex::new(Vec::new());
/// The hitbox records of this frame (see hitbox.rs); returns their length in numbers, read at `wd_hitbox_ptr`.
#[no_mangle]
pub extern "C" fn wd_hitbox_build() -> usize { let v = with_game(|g| g.hitbox_shapes()); let n = v.len(); *HB_BUF.lock().unwrap() = v; n }
#[no_mangle]
pub extern "C" fn wd_hitbox_ptr() -> *const f64 { HB_BUF.lock().unwrap().as_ptr() }
/// `window.showEditorGlow`: the glow sprites of the editor view
#[no_mangle]
pub extern "C" fn wd_ed_set_glow(on: u32) { with_ui(|u| u.ed.glow = on != 0); }
#[no_mangle]
pub extern "C" fn wd_ed_glow() -> u32 { with_ui(|u| u.ed.glow as u32) }
/// Test hook: 0 starts, 1 pauses / resumes, 2 stops the playtest.
#[no_mangle]
pub extern "C" fn wd_ed_playtest(op: u32) { with_ui(|u| match op { 0 => u.ed_pt_start(), 1 => u.ed_pt_toggle_pause(), 3 => u.ed_pm_open(), 4 => u.ed_ls_open(), 5 => u.ed_ls_action("EdSoOpen", 0).then_some(()).unwrap_or(()), 6 => { u.ed_ls_open(); u.ed_ls_action("EdLsPick", 2); } 7 => { u.ed_ls_open(); u.ed_ls_action("EdLsPick", 0); } 8 => u.ed_cp_open(1000), 10 | 11 | 12 | 13 | 14 => { let src = match op { 10 => "1,31,2,300,3,90", 11 => "1,914,2,300,3,90,31,SGVsbG8=", 12 => "1,901,2,300,3,90,51,5,28,12.5,10,1.5", 14 => "1,899,2,300,3,90,23,3,7,255,8,100,9,0,10,2.5", _ => "1,1268,2,300,3,90,51,7,63,0.25" }; if let Some(o) = editor::EdObj::parse(src) { u.ed.level.objects.push(o); u.ed.sel = vec![u.ed.level.objects.len() - 1]; u.ed.dirty = true; u.ed_open_object_options(); } } 9 => { if !u.ed.level.objects.is_empty() { u.ed.sel = vec![0, 1.min(u.ed.level.objects.len() - 1)]; u.ed_gp_open(); } } _ => u.ed_pt_stop(false) }); }
/// Trail segments and death crosses: flat rows of 6 (kind, x0, y0, x1, y1, colour); returns the number of rows (read through `wd_ed_marks_ptr`).
static ED_MARKS: Mutex<Vec<f64>> = Mutex::new(Vec::new());
#[no_mangle]
pub extern "C" fn wd_ed_marks() -> usize { let v: Vec<f64> = with_ui(|u| u.ed.marks.iter().flatten().copied().collect()); let n = v.len() / 6; *ED_MARKS.lock().unwrap() = v; n }
#[no_mangle]
pub extern "C" fn wd_ed_marks_ptr() -> *const f64 { ED_MARKS.lock().unwrap().as_ptr() }
#[no_mangle]
pub extern "C" fn wd_ed_level_string() -> usize { out_str(with_ui(|u| u.ed_level_string())) }


// ---- player visuals (web/player_vis.js reads this; nothing here changes the simulation) ----
static PV_BUF: Mutex<Vec<f64>> = Mutex::new(Vec::new());
/// Visual state of the primary (`which` 0) or secondary (1) player as flat numbers:
/// 0 y, 1 world x, 2 rotation, 3 dead, 4 mini, 5 ship, 6 wave, 7 ball, 8 ufo, 9 spider, 10 robot, 11 gravity flipped, 12 mirrored,
/// 13 on ground, 14 on ceiling, 15 dashing, 16 dash y velocity, 17 y velocity, 18 jumping, 19 up key down,
/// 20 slope angle (NaN none), 21 player speed, 22 spider teleport timer, 23 spider flash timer, 24 spider flash duration, 25 dual active, 26 swing.
#[no_mangle]
pub extern "C" fn wd_pv_state(which: u32) -> *const f64 {
    with_game(|g| {
        let p = if which == 0 { &g.p } else { &g.p2 };
        let b = |v: bool| if v { 1.0 } else { 0.0 };
        let v = vec![
            p.y, g.e.player_world_x, p.rotation, b(p.is_dead), b(p.is_mini), b(p.is_flying), b(p.is_wave), b(p.is_ball), b(p.is_ufo), b(p.is_spider), b(p.is_robot),
            b(p.gravity_flipped), b(p.mirrored), b(p.on_ground), b(p.on_ceiling), b(p.is_dashing), p.dash_y_velocity, p.y_velocity, b(p.is_jumping), b(p.up_key_down),
            p.on_slope_angle.unwrap_or(f64::NAN), g.g.player_speed, p.spider_teleport_anim_timer, p.spider_flash_timer, p.spider_flash_duration, b(g.e.is_dual), b(p.is_swing),
        ];
        *PV_BUF.lock().unwrap() = v;
    });
    PV_BUF.lock().unwrap().as_ptr()
}

/// Drains the visual events (`land:`, `grav:`, `fx:`, `tp:` lines, `|` separated) into the result buffer; returns the length.
#[no_mangle]
pub extern "C" fn wd_pv_events() -> usize {
    with_game(|g| { let s = g.vlog.join("|"); g.vlog.clear(); out_str(s) })
}

// ---- garage (icon kit) host calls ----
#[no_mangle]
pub unsafe extern "C" fn wd_gar_set(kp: *const u8, kl: usize, vp: *const u8, vl: usize) {
    let k = String::from_utf8_lossy(std::slice::from_raw_parts(kp, kl)).into_owned();
    let v = String::from_utf8_lossy(std::slice::from_raw_parts(vp, vl)).into_owned();
    with_ui(|u| u.gar_set(&k, &v));
}
#[no_mangle]
pub unsafe extern "C" fn wd_gar_anim(kind: u32, p: *const u8, l: usize) { let t = String::from_utf8_lossy(std::slice::from_raw_parts(p, l)).into_owned(); with_ui(|u| u.gar_set_anim(kind as usize, &t)); }
#[no_mangle]
pub extern "C" fn wd_gar_open() { with_ui(|u| u.gar_open()); }
#[no_mangle]
pub extern "C" fn wd_gar_close() { with_ui(|u| { u.gar_action("GarageClose", 0); }); }
#[no_mangle]
pub extern "C" fn wd_gar_is_open() -> u32 { with_ui(|u| u.gar.open as u32) }
/// JSON list of the icon bases the grid is waiting for
#[no_mangle]
pub extern "C" fn wd_gar_need() -> usize { out_str(with_ui(|u| { let need = std::mem::take(&mut u.gar.need); format!("[{}]", need.iter().map(|b| json::str_lit(b)).collect::<Vec<_>>().join(",")) })) }
#[no_mangle]
pub unsafe extern "C" fn wd_gar_ready(p: *const u8, l: usize, ok: u32) {
    let b = String::from_utf8_lossy(std::slice::from_raw_parts(p, l)).into_owned();
    with_ui(|u| { if ok != 0 { u.gar.ready.insert(b); } else { u.gar.failed.insert(b); } });
}
#[no_mangle]
pub extern "C" fn wd_gar_rebuild() { with_ui(|u| u.gar_rebuild()); }
/// `key=value` lines of the choices made since the last call
#[no_mangle]
pub extern "C" fn wd_gar_changes() -> usize { out_str(with_ui(|u| { let s = u.gar.changes.join("\n"); u.gar.changes.clear(); s })) }
/// Switches the garage tab (tests / host)
#[no_mangle]
pub extern "C" fn wd_gar_tab(n: u32) { with_ui(|u| { u.gar_action("GarageTab", n as usize); }); }


/// Turns the scene's mirror-portal handling on (the physics test harnesses leave it off).
#[no_mangle]
pub extern "C" fn wd_mirror_enable(on: u32) { with_game(|g| g.mirror.enabled = on != 0); }
/// Tests: force the mirror state (animating, target, from, p, mirrored)
#[no_mangle]
pub extern "C" fn wd_mirror_set(animating: u32, target: u32, from: f64, p: f64, mirrored: u32) {
    with_game(|g| { g.mirror.animating = animating != 0; g.mirror.target = target != 0; g.mirror.from = from; g.mirror.p = p; g.p.mirrored = mirrored != 0; });
}
#[no_mangle]
pub extern "C" fn wd_mirror_progress() -> f64 { with_game(|g| g.mirror_progress()) }
/// `[x, scaleX, rotation, alpha, visible]` of a level sprite after the mirror effect (tests)
#[no_mangle]
pub extern "C" fn wd_mirror_view(si: usize) -> usize { with_game(|g| { let v = g.mirror_view(si); out_str(format!("[{},{},{},{},{}]", v.0, v.1, v.2, v.3, v.4 as u8)) }) }
/// Tests: the scene's camera (the trigger test harness drives the triggers directly)
#[no_mangle]
pub extern "C" fn wd_mirror_cam(x: f64) { with_game(|g| g.camera_x = x); }
#[no_mangle]
pub extern "C" fn wd_mirror_toggle(on: u32) -> u32 { with_game(|g| g.toggle_mirror(on != 0) as u32) }
#[no_mangle]
pub extern "C" fn wd_mirror_step(dt: f64) { with_game(|g| g.step_mirror(dt)); }
/// `[animating, target, from, p, time, mirrored, mirrored2]`
#[no_mangle]
pub extern "C" fn wd_mirror_state() -> usize { with_game(|g| out_str(format!("[{},{},{},{},{},{},{}]", g.mirror.animating as u8, g.mirror.target as u8, g.mirror.from, g.mirror.p, g.mirror.time, g.p.mirrored as u8, g.p2.mirrored as u8))) }

// ---- coins ----
static COINS_INIT: Mutex<Option<coins::CoinsInit>> = Mutex::new(None);
fn id_csv(ptr: *const u8, len: usize) -> Vec<i64> {
    let s = unsafe { String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned() };
    s.split(',').filter_map(|v| v.trim().parse::<i64>().ok()).collect()
}
/// Turns saved-coin handling on for the next level load: the level key (`currentlevel[2]`), what kind of level it is, the saved
/// progress as comma separated linked object ids, and the running totals. Never called = the physics harness behaviour.
#[no_mangle]
pub unsafe extern "C" fn wd_coins_config(official: u32, stored_user: u32, editor: u32, key: *const u8, key_len: usize, secret: *const u8, secret_len: usize, user: *const u8, user_len: usize, total_secret: f64, total_user: f64) {
    *COINS_INIT.lock().unwrap() = Some(coins::CoinsInit {
        official: official != 0, stored_user: stored_user != 0, editor: editor != 0,
        key: String::from_utf8_lossy(std::slice::from_raw_parts(key, key_len)).into_owned(),
        secret_done: id_csv(secret, secret_len), user_done: id_csv(user, user_len),
        total_secret: total_secret as u64, total_user: total_user as u64,
    });
}
/// localStorage writes waiting for the host: JSON `[[key, value], ...]` (cleared)
#[no_mangle]
pub extern "C" fn wd_coins_outbox() -> usize {
    out_str(with_game(|g| {
        let o = std::mem::take(&mut g.coins.outbox);
        format!("[{}]", o.iter().map(|(k, v)| format!("[{},{}]", json::str_lit(k), json::str_lit(v))).collect::<Vec<_>>().join(","))
    }))
}
/// `_levelComplete` outside practice mode: save the coins collected this run
#[no_mangle]
pub extern "C" fn wd_coins_commit() { with_game(|g| g.commit_coins()); }
/// Tests: `{"secretRun":[..],"userRun":[..],"secretDone":[..],"userDone":[..]}`
#[no_mangle]
pub extern "C" fn wd_coins_state() -> usize {
    out_str(with_game(|g| { let c = &g.coins; format!("{{\"secretRun\":{:?},\"userRun\":{:?},\"secretDone\":{:?},\"userDone\":{:?}}}", c.secret_run, c.user_run, c.secret_done, c.user_done) }))
}
/// Tests: a coin collider is touched (the coin branch of `checkCollisions`, outside practice mode); 1 when it was collected
#[no_mangle]
pub extern "C" fn wd_coins_touch(uid: usize) -> u32 {
    with_game(|g| {
        if g.cols[uid].activated { return 0; }
        let col = g.cols[uid].clone();
        if !g.coins.gate(&col) { return 0; }
        g.cols[uid].activated = true;
        1
    })
}
/// Tests: `resetObjects` (coins and everything else)
#[no_mangle]
pub extern "C" fn wd_game_reset_objects() { with_game(|g| g.reset_objects()); }

// ---- BeastAnim ----
/// Hands the host's `<name>_AnimDesc.json` to the beast code (before the level is loaded).
#[no_mangle]
pub unsafe extern "C" fn wd_beast_desc_add(name: *const u8, name_len: usize, json: *const u8, json_len: usize) -> u32 {
    let n = String::from_utf8_lossy(std::slice::from_raw_parts(name, name_len)).into_owned();
    let j = String::from_utf8_lossy(std::slice::from_raw_parts(json, json_len)).into_owned();
    beast::add_desc(&n, &j) as u32
}
/// Tests: a free-standing beast state (id) kept in slot `slot`
static BEAST_TEST: Mutex<Vec<Option<beast::BState>>> = Mutex::new(Vec::new());
#[no_mangle]
pub extern "C" fn wd_beast_test_new(slot: usize, object_id: f64) -> u32 {
    let mut v = BEAST_TEST.lock().unwrap();
    while v.len() <= slot { v.push(None); }
    v[slot] = beast::BState::create(object_id as i64);
    v[slot].is_some() as u32
}
/// op 0 = advance(dt), 1 = command(animIdx, delay override or NaN), 2 = defaultBase; returns the state JSON
#[no_mangle]
pub extern "C" fn wd_beast_test_op(slot: usize, op: u32, a: f64, b: f64) -> usize {
    let mut v = BEAST_TEST.lock().unwrap();
    let s = v[slot].as_mut().unwrap();
    match op { 0 => s.advance(a), 1 => { s.command(a as i64, if b.is_nan() { None } else { Some(b) }); } 2 => s.default_base(), _ => {} }
    let j = s.json();
    drop(v);
    out_str(j)
}
/// Tests: `commandMany` over the listed slots
#[no_mangle]
pub unsafe extern "C" fn wd_beast_test_many(slots: *const f64, n: usize, anim_idx: f64) {
    let ids: Vec<usize> = std::slice::from_raw_parts(slots, n).iter().map(|v| *v as usize).collect();
    let mut v = BEAST_TEST.lock().unwrap();
    let mut taken: Vec<beast::BState> = ids.iter().map(|&i| v[i].take().unwrap()).collect();
    { let mut refs: Vec<&mut beast::BState> = taken.iter_mut().collect(); beast::command_many(&mut refs, anim_idx as i64); }
    for (k, &i) in ids.iter().enumerate() { v[i] = Some(taken[k].clone()); }
}
/// JSON of the level's beasts: `[[state, partCount, glowCount, hiddenCount], ...]`
#[no_mangle]
pub extern "C" fn wd_beast_info() -> usize {
    out_str(with_game(|g| format!("[{}]", g.beasts.list.iter().map(|b| format!("[{},{},{},{}]", b.state.json(), b.parts.len(), b.glows.len(), b.hidden.len())).collect::<Vec<_>>().join(","))))
}
/// Tests: the beast update step of a frame (`BeastAnim.update`), `delta_ms` of real time
#[no_mangle]
pub extern "C" fn wd_beast_update(delta_ms: f64) { with_game(|g| g.beast_update(delta_ms)); }

/// No-clip statistics shown by the HUD: 0 = accuracy (percent), 1 = deaths
#[no_mangle]
pub extern "C" fn wd_noclip_stat(which: u32) -> f64 { with_game(|g| if which == 0 { g.p.nc_accuracy } else { g.p.nc_deaths }) }

// ---- online levels (web/online.js) ----
#[no_mangle]
pub unsafe extern "C" fn wd_on_results(ptr: *const u8, len: usize, total: f64, offset: f64, count: f64) { let j = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned(); with_ui(|u| u.on_set_results(&j, total as i64, offset as i64, count as i64)); }
#[no_mangle]
pub unsafe extern "C" fn wd_on_error(ptr: *const u8, len: usize) { let j = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned(); with_ui(|u| u.on_set_error(&j)); }
#[no_mangle]
pub unsafe extern "C" fn wd_on_special(ptr: *const u8, len: usize) { let j = String::from_utf8_lossy(std::slice::from_raw_parts(ptr, len)).into_owned(); with_ui(|u| u.on_set_special(&j)); }
#[no_mangle]
pub extern "C" fn wd_on_query() -> usize { out_str(with_ui(|u| u.on_query())) }
#[no_mangle]
pub extern "C" fn wd_on_page() -> usize { with_ui(|u| u.on_page()) }
#[no_mangle]
pub extern "C" fn wd_on_kind() -> u32 { with_ui(|u| u.on_kind() as u32) }
/// The play menu's back action (the host calls it after the delete button was confirmed).
#[no_mangle]
pub extern "C" fn wd_on_menu_back() { with_ui(|u| u.emit(ui::Action::Pop("OnMenuBack", 0))); }
#[no_mangle]
pub extern "C" fn wd_on_close() { with_ui(|u| { u.kill_tag("on_"); u.on.special = None; u.on.input = false; }); }
#[no_mangle]
pub extern "C" fn wd_on_play_busy(on: u32) { with_ui(|u| u.on_play_busy(on != 0)); }
#[no_mangle]
pub extern "C" fn wd_on_reopen() -> i32 { with_ui(|u| u.on_reopen() as i32) }
#[no_mangle]
pub extern "C" fn wd_on_diff() -> usize { out_str(with_ui(|u| u.on_diff_param())) }
#[no_mangle]
pub unsafe extern "C" fn wd_on_song(np: *const u8, nl: usize, ap: *const u8, al: usize) {
    let n = String::from_utf8_lossy(std::slice::from_raw_parts(np, nl)).into_owned();
    let a = String::from_utf8_lossy(std::slice::from_raw_parts(ap, al)).into_owned();
    with_ui(|u| u.on_set_song(&n, &a));
}
#[no_mangle]
pub extern "C" fn wd_on_demon() -> u32 { with_ui(|u| u.on_demon()) }
/// the current audio meter value and the smoothed value objects scale with (debug)
#[no_mangle]
pub extern "C" fn wd_audio_dbg(which: u32) -> f64 { with_game(|g| match which { 0 => g.fx.audio_meter, 1 => g.fx.pulse_meter.unwrap_or(f64::NAN), 2 => g.fx.lists.audio_scale.len() as f64, 3 => { g.fx.lists.audio_scale.iter().filter(|&&i| g.fx.sprites[i].active).map(|&i| g.fx.sprites[i].scale_x).fold(0.0, f64::max) } 4 => { let a: Vec<f64> = g.fx.lists.audio_pulse.iter().filter(|&&i| g.fx.sprites[i].active).map(|&i| g.fx.sprites[i].scale_x).collect(); (a.len() as f64) * 1000.0 + a.iter().sum::<f64>() / a.len().max(1) as f64 } 5 => { let a: Vec<f64> = g.fx.lists.audio_pulse.iter().filter_map(|&i| g.fx.sprites[i].num("_eePulseMult")).collect(); (a.len() as f64) * 1000.0 + a.iter().sum::<f64>() / a.len().max(1) as f64 } 6 => g.fx.sprites.iter().filter(|s| s.flag("_eePulsed")).count() as f64 * 1000.0 + g.trig.active_pulses.len() as f64, 7 => { let (lo, hi) = (g.camera_x - 200.0, g.camera_x + g.screen_width + 200.0); let a: Vec<f64> = g.fx.lists.audio_pulse.iter().filter(|&&i| g.fx.sprites[i].num("_eeWorldX").map_or(false, |x| x >= lo && x <= hi)).filter_map(|&i| g.fx.sprites[i].num("_eePulseMult")).collect(); a.len() as f64 * 1000.0 + a.iter().sum::<f64>() / a.len().max(1) as f64 } 8 => { let (lo, hi) = (g.camera_x - 200.0, g.camera_x + g.screen_width + 200.0); let a: Vec<f64> = g.fx.lists.audio_scale.iter().filter(|&&i| { let sp = &g.fx.sprites[i]; sp.active && sp.visible && sp.num("_eeWorldX").map_or(true, |x| x >= lo && x <= hi) }).map(|&i| g.fx.sprites[i].scale_x).collect(); a.len() as f64 * 1000.0 + a.iter().cloned().fold(0.0, f64::max) } 9 => g.fx.lists.animated.len() as f64, 10 => g.fx.lists.saws.len() as f64, 11 => g.fx.sprites.len() as f64, 12 => g.fx.lists.orbs.len() as f64, _ => f64::NAN }) }

//! The macro bot (`class MacroBot` in game-scene.js): records a snapshot of the player state after every physics step and plays such
//! snapshots back. Frames are kept as JSON values so that macros made by the original load unchanged and the file format is identical
//! (`{ meta, frames: [{ frame, playerWorldX, cameraX, cameraY, speed, dual, player: { state, rotation, mode }, dualPlayer }] }`).
use crate::game::Game;
use crate::jsonread::{parse, Json};
use crate::physics::Player;

#[derive(Clone, Debug)]
pub struct MacroBot {
    pub recording: bool,
    pub playing: bool,
    pub cursor: usize,
    pub frames: Vec<Json>,
    /// index into `frames` of `currentFrameState`
    pub current: Option<usize>,
    pub meta: Vec<(String, Json)>,
}

impl Default for MacroBot { fn default() -> Self { let mut m = MacroBot { recording: false, playing: false, cursor: 0, frames: Vec::new(), current: None, meta: Vec::new() }; m.reset_all(); m } }

impl MacroBot {
    /// `resetAll`
    pub fn reset_all(&mut self) {
        self.recording = false; self.playing = false; self.cursor = 0; self.frames.clear(); self.current = None;
        self.meta = vec![("author".into(), Json::Str("Web Dashers".into())), ("level".into(), Json::Str(String::new())), ("version".into(), Json::Num(2.0))];
    }
}

/// `PlayerState` keys in the order `reset()` assigns them, and the Rust field each one lives in (None: always false in the original).
const STATE_KEYS: &[(&str, Option<&str>)] = &[
    ("_gdFallStartY", Some("gd_fall_start_y")), ("_gdLandFlag", Some("gd_land_flag")), ("_gdPrevFallVel", Some("gd_prev_fall_vel")), ("_gdWasFalling", Some("gd_was_falling")),
    ("_gdPrevHeld", Some("gd_prev_held")), ("_gdPrevCeiling", Some("gd_prev_ceiling")),
    ("y", Some("y")), ("lastY", Some("last_y")), ("lastGroundPosY", Some("last_ground_pos_y")), ("yVelocity", Some("y_velocity")), ("onGround", Some("on_ground")),
    ("canJump", Some("can_jump")), ("isJumping", Some("is_jumping")), ("gravityFlipped", Some("gravity_flipped")), ("gravityScale", Some("gravity_scale")), ("isFlying", Some("is_flying")), ("isBall", Some("is_ball")),
    ("isWave", Some("is_wave")), ("isUfo", Some("is_ufo")), ("isSpider", Some("is_spider")), ("isBird", None), ("isDart", None), ("isRobot", Some("is_robot")), ("isSwing", Some("is_swing")),
    ("isJetpack", None), ("isMini", Some("is_mini")), ("wasBoosted", Some("was_boosted")), ("gdAccel", Some("gd_accel")), ("robotAccel", Some("robot_accel")),
    ("robotTouchedPad", Some("robot_touched_pad")), ("_slopeBounceActive", Some("slope_bounce_active")), ("pendingVelocity", Some("pending_velocity")), ("collideTop", Some("collide_top")),
    ("collideBottom", Some("collide_bottom")), ("onCeiling", Some("on_ceiling")), ("upKeyDown", Some("up_key_down")), ("upKeyPressed", Some("up_key_pressed")), ("queuedHold", Some("queued_hold")),
    ("_orbActivationConsumedForPress", Some("orb_activation_consumed_for_press")), ("isDead", Some("is_dead")), ("mirrored", Some("mirrored")), ("isDashing", Some("is_dashing")),
    ("dashYVelocity", Some("dash_y_velocity")), ("isDual", None), ("ignorePortals", Some("ignore_portals")), ("_robotHold", Some("robot_hold")), ("_robotHoldTimer", Some("robot_hold_timer")),
    ("_spiderTeleportNoclipDeathPending", Some("spider_teleport_noclip_death_pending")), ("ballShouldRotate", Some("ball_should_rotate")), ("ballRotateOpposite", Some("ball_rotate_opposite")),
    ("ballNormalRotate", Some("ball_normal_rotate")), ("ballHitPad", Some("ball_hit_pad")), ("ballRotation2", Some("ball_rotation2")),
];

fn fields() -> &'static Vec<(String, String)> {
    static F: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    F.get_or_init(|| { let mut v = Vec::new(); Player::names("", &mut v); v.into_iter().map(|s| { let (n, t) = s.split_once(':').unwrap(); (n.to_string(), t.to_string()) }).collect() })
}

/// JS key -> Rust field for the PlayerState keys outside `reset()` that the original sets on the state later.
fn dynamic_field(js_key: &str) -> Option<String> {
    const SPECIAL: [(&str, &str); 15] = [("_pressCount", "press_count"), ("forceTicks", "force_ticks"), ("safeHeadTicks", "safe_head_ticks"), ("flipHeadTicks", "flip_head_ticks"), ("jBlockTicks", "j_block_ticks"), ("lastGroundY", "last_ground_y"), ("wasUpKeyDown", "was_up_key_down"), ("touchingRing", "touching_ring"), ("_lastBluePadTick", "last_blue_pad_tick"), ("_spiderTeleportPending", "spider_teleport_pending"), ("_spiderTeleportAnimTimer", "spider_teleport_anim_timer"),
        ("_spiderFlashTimer", "spider_flash_timer"), ("_spiderFlashDuration", "spider_flash_duration"), ("diedThisFrame", "died_this_frame"), ("diedLastFrame", "died_last_frame")];
    SPECIAL.iter().find(|(k, _)| *k == js_key).map(|(_, f)| f.to_string())
}

fn get_field(p: &Player, name: &str) -> Option<(f64, String)> {
    let f = fields();
    let i = f.iter().position(|(n, _)| n == name)?;
    let mut v = Vec::new(); p.pack(&mut v);
    Some((v[i], f[i].1.clone()))
}

fn set_field(p: &mut Player, name: &str, value: f64) {
    let f = fields();
    let Some(i) = f.iter().position(|(n, _)| n == name) else { return };
    let mut v = Vec::new(); p.pack(&mut v);
    v[i] = value;
    p.unpack(&mut v.into_iter());
}

fn mode_id(p: &Player) -> &'static str {
    if p.is_swing { "swing" } else if p.is_flying { "ship" } else if p.is_ball { "ball" } else if p.is_ufo { "ufo" } else if p.is_wave { "wave" } else if p.is_robot { "robot" } else if p.is_spider { "spider" } else { "cube" }
}

/// `_cloneState(state)` for `PlayerState`: the primitive fields in the order the original keeps them.
fn clone_state(p: &Player) -> Json {
    let mut out: Vec<(String, Json)> = Vec::new();
    for (key, field) in STATE_KEYS.iter() {
        let Some(field) = field else { out.push((key.to_string(), Json::Bool(false))); continue };
        let Some((v, ty)) = get_field(p, field) else { continue };
        if v.is_nan() && matches!(*key, "_gdPrevFallVel" | "_gdPrevHeld") { continue; }
        let j = match ty.as_str() { "bool" | "Option<bool>" => Json::Bool(v != 0.0), "Option<f64>" => if v.is_nan() { Json::Null } else { Json::Num(v) }, _ => Json::Num(v) };
        out.push((key.to_string(), j));
    }
    // keys the original adds to the state object later (only present once set)
    for (key, field) in [("lastGroundY", "last_ground_y"), ("wasUpKeyDown", "was_up_key_down"), ("touchingRing", "touching_ring"), ("diedThisFrame", "died_this_frame"), ("diedLastFrame", "died_last_frame"), ("_lastBluePadTick", "last_blue_pad_tick"), ("_pressCount", "press_count"), ("forceTicks", "force_ticks"), ("safeHeadTicks", "safe_head_ticks"), ("flipHeadTicks", "flip_head_ticks"), ("jBlockTicks", "j_block_ticks")] {
        let Some((v, ty)) = get_field(p, field) else { continue };
        if ty.starts_with("Option") || field == "last_ground_y" { if v.is_nan() { continue; } }
        out.push((key.to_string(), match ty.as_str() { "bool" => Json::Bool(v != 0.0), _ => Json::Num(v) }));
    }
    Json::Obj(out)
}

fn snapshot(p: &Player) -> Json {
    let rot = if p.rotation.is_finite() { p.rotation } else { 0.0 };
    Json::Obj(vec![("state".into(), clone_state(p)), ("rotation".into(), Json::Num(rot)), ("mode".into(), Json::Str(mode_id(p).into()))])
}

pub fn to_json(j: &Json, out: &mut String) {
    match j {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Num(n) => out.push_str(&crate::json::num(*n)),
        Json::Str(s) => out.push_str(&crate::json::str_lit(s)),
        Json::Arr(a) => { out.push('['); for (i, v) in a.iter().enumerate() { if i > 0 { out.push(','); } to_json(v, out); } out.push(']'); }
        Json::Obj(o) => { out.push('{'); for (i, (k, v)) in o.iter().enumerate() { if i > 0 { out.push(','); } out.push_str(&crate::json::str_lit(k)); out.push(':'); to_json(v, out); } out.push('}'); }
    }
}

fn frame_no(f: &Json) -> f64 { f.get_num("frame").unwrap_or(0.0) }

impl Game {
    /// `startRecording(meta)`
    pub fn macro_start_recording(&mut self, meta: &[(String, Json)]) {
        self.mb.reset_all();
        self.mb.recording = true;
        for (k, v) in meta { match self.mb.meta.iter_mut().find(|(mk, _)| mk == k) { Some(e) => e.1 = v.clone(), None => self.mb.meta.push((k.clone(), v.clone())) } }
        match self.mb.meta.iter_mut().find(|(k, _)| k == "version") { Some(e) => e.1 = Json::Num(2.0), None => self.mb.meta.push(("version".into(), Json::Num(2.0))) }
        self.e.macro_playing = false;
    }

    /// `stopRecording()`: the exported macro
    pub fn macro_stop_recording(&mut self) -> String { self.mb.recording = false; self.macro_export() }

    pub fn macro_clear_recording(&mut self) { self.mb.frames.clear(); self.mb.cursor = 0; self.mb.current = None; }
    pub fn macro_clear_playback(&mut self) { self.mb.cursor = 0; self.mb.current = None; }

    /// `_captureFrame(currentFrame)`
    fn macro_capture(&self, frame: f64) -> Json {
        let mut v = vec![
            ("frame".to_string(), Json::Num(frame)), ("playerWorldX".into(), Json::Num(self.e.player_world_x)), ("cameraX".into(), Json::Num(self.camera_x)), ("cameraY".into(), Json::Num(self.camera_y)),
            ("speed".into(), Json::Num(self.g.player_speed)), ("dual".into(), Json::Bool(self.e.is_dual)), ("player".into(), snapshot(&self.p)),
            ("dualPlayer".into(), if self.e.is_dual { snapshot(&self.p2) } else { Json::Null }),
        ];
        v.shrink_to_fit();
        Json::Obj(v)
    }

    /// `recordFrame(currentFrame)`
    pub fn macro_record_frame(&mut self) {
        if !self.mb.recording { return; }
        let cur = self.physics_frame;
        let f = self.macro_capture(cur);
        match self.mb.frames.last_mut() { Some(last) if frame_no(last) == cur => *last = f, _ => self.mb.frames.push(f) }
    }

    /// `rollbackRecording(currentFrame)`
    pub fn macro_rollback_recording(&mut self, frame: f64) {
        self.mb.frames.retain(|f| frame_no(f) <= frame);
        self.mb.cursor = 0; self.mb.current = None;
    }

    /// `rollbackPlayback(currentFrame)`
    pub fn macro_rollback_playback(&mut self, frame: f64) {
        if self.mb.frames.is_empty() { return; }
        self.mb.cursor = 0; self.mb.current = None;
        while self.mb.cursor < self.mb.frames.len() && frame_no(&self.mb.frames[self.mb.cursor]) <= frame { self.mb.cursor += 1; }
        let index = self.mb.cursor.saturating_sub(1);
        let f = self.mb.frames[index].clone();
        self.macro_apply_frame(&f);
    }

    /// `startPlayback(macroData)`; false when the JSON does not parse
    pub fn macro_start_playback(&mut self, data: &str) -> bool {
        let Ok(m) = parse(data) else { return false };
        self.mb.reset_all();
        self.mb.playing = true;
        self.e.macro_playing = true;
        if let Json::Obj(entries) = &m { for (k, v) in entries { if k == "frames" { continue; } match self.mb.meta.iter_mut().find(|(mk, _)| mk == k) { Some(e) => e.1 = v.clone(), None => self.mb.meta.push((k.clone(), v.clone())) } } }
        // (`{...this.meta, ...macro}` copies every key of the macro, `frames` included; the exported object rebuilds `frames` itself)
        self.mb.frames = m.get("frames").and_then(|f| f.arr()).map(|a| a.to_vec()).unwrap_or_default();
        self.mb.frames.sort_by(|a, b| frame_no(a).partial_cmp(&frame_no(b)).unwrap_or(std::cmp::Ordering::Equal));
        self.mb.cursor = 0; self.mb.current = None;
        true
    }

    pub fn macro_stop_playback(&mut self) { self.mb.playing = false; self.e.macro_playing = false; self.mb.cursor = 0; self.mb.current = None; }

    /// `step(currentFrame)`, called once per physics step while playing
    pub fn macro_step(&mut self) {
        let cur = self.physics_frame;
        if !self.mb.playing || self.mb.frames.is_empty() { return; }
        while self.mb.cursor < self.mb.frames.len() && frame_no(&self.mb.frames[self.mb.cursor]) <= cur { self.mb.current = Some(self.mb.cursor); self.mb.cursor += 1; }
        if self.mb.cursor >= self.mb.frames.len() { self.macro_stop_playback(); return; }
        if let Some(i) = self.mb.current { let f = self.mb.frames[i].clone(); self.macro_apply_frame(&f); }
    }

    /// `applyCurrentFrame()`
    pub fn macro_apply_current(&mut self) {
        if !self.mb.playing { return; }
        if let Some(i) = self.mb.current { let f = self.mb.frames[i].clone(); self.macro_apply_frame(&f); }
    }

    fn macro_apply_snapshot(&mut self, snap: &Json, second: bool) {
        let mode = snap.get_str("mode").unwrap_or("cube").to_string();
        let current = mode_id(if second { &self.p2 } else { &self.p });
        if current != mode { self.sim_for_pub(second, |s| s.set_player_gamemode(&mode, true)); }
        let p = if second { &mut self.p2 } else { &mut self.p };
        if let Some(entries) = snap.get("state").and_then(|s| s.entries()) {
            for (key, val) in entries {
                let field = STATE_KEYS.iter().find(|(k, _)| k == key).map(|(_, f)| f.map(|s| s.to_string())).or_else(|| dynamic_field(key).map(Some));
                let Some(Some(field)) = field else { continue };
                let Some((_, ty)) = get_field(p, &field) else { continue };
                let v = match val { Json::Num(n) => *n, Json::Bool(b) => *b as u8 as f64, Json::Null if ty.starts_with("Option") => f64::NAN, _ => continue };
                set_field(p, &field, v);
            }
        }
        if let Some(r) = snap.get_num("rotation") { if r.is_finite() { p.rotation = r; } }
    }

    /// `_applyFrame(frameState)`
    fn macro_apply_frame(&mut self, f: &Json) {
        if let Some(x) = f.get_num("playerWorldX") { if x.is_finite() { self.e.player_world_x = x; } }
        if let Some(x) = f.get_num("cameraX") { if x.is_finite() { self.camera_x = x; } }
        if let Some(x) = f.get_num("cameraY") { if x.is_finite() { self.camera_y = x; } }
        if let Some(x) = f.get_num("speed") { if x.is_finite() { self.g.player_speed = x; } }
        if let Some(s) = f.get("player") { if matches!(s, Json::Obj(_)) { self.macro_apply_snapshot(s, false); } }
        self.e.is_dual = f.get_bool("dual").unwrap_or(false);
        if self.e.is_dual { if let Some(s) = f.get("dualPlayer") { if matches!(s, Json::Obj(_)) { self.macro_apply_snapshot(s, true); } } }
    }

    /// `exportObject()` as JSON text
    pub fn macro_export(&self) -> String {
        let mut out = String::from("{\"meta\":");
        to_json(&Json::Obj(self.mb.meta.clone()), &mut out);
        out.push_str(",\"frames\":");
        to_json(&Json::Arr(self.mb.frames.clone()), &mut out);
        out.push('}');
        out
    }

    pub fn macro_set_name(&mut self, name: &str) {
        match self.mb.meta.iter_mut().find(|(k, _)| k == "name") { Some(e) => e.1 = Json::Str(name.into()), None => self.mb.meta.push(("name".into(), Json::Str(name.into()))) }
    }
}

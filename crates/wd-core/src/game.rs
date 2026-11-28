//! The per-frame simulation loop (game-scene.js `update`, physics part) for one player:
//! input -> quantised substeps -> jump update -> move -> collisions -> rotation.
//! Dual mode, move triggers, checkpoints, macros and the camera are not part of this yet.
use crate::jsmath as m;
use crate::pcol::Col;
use crate::pdual::{dual_shared_signature, SyncOpts};
use crate::physics::{Env, Globals, Player, Sim, D};

const U: f64 = 1.0 / 240.0;
const BALL_G: f64 = 60.0; // `g` in config.js (= `a`)

/// The level header options read at an attempt start (`kA2` mode, `kA4` speed, `kA3` mini, `kA11` upside down, `kA28` mirrored, `kA8` dual).
#[derive(Clone, Copy, Debug, Default)]
pub struct StartOptions { pub game_mode: i64, pub speed_key: i64, pub mini: bool, pub gravity_flipped: bool, pub mirrored: bool, pub dual: bool }

/// A start position object (ids 31 / 34), in `_startPositions` form.
#[derive(Clone, Copy, Debug)]
pub struct StartPos { pub x: f64, pub y: f64, pub game_mode: Option<i64>, pub mini: Option<i64>, pub speed: Option<i64>, pub dual: Option<i64>, pub mirrored: Option<i64>, pub gravity_flipped: bool }

/// The end-of-level camera move (a 1.2 s tween from where the camera is to the end portal).
#[derive(Clone, Copy, Debug)]
pub struct EndCam { pub from_x: f64, pub to_x: f64, pub from_y: f64, pub to_y: f64, pub elapsed_ms: f64 }

pub struct Game {
    pub cm: crate::colors::ColorManager,
    pub channel_blending: std::collections::BTreeMap<i64, bool>,
    pub channel_blending_initial: std::collections::BTreeMap<i64, bool>,
    pub ldm: bool,
    pub settings: crate::settings::LevelSettings,
    pub practice_mode: bool,
    pub checkpoints: Vec<Checkpoint>,
    pub screen_width: f64,
    pub end_x_pos: f64,
    pub end_portal_game_y: f64,
    pub camera_x: f64,
    pub camera_y: f64,
    pub follow_camera_y: Option<f64>,
    pub rotate_gameplay_enabled: bool,
    pub sideways_proxies: std::collections::BTreeMap<usize, Col>,
    pub level_won: bool,
    pub end_camera_override: bool,
    pub end_cam: Option<EndCam>,
    pub death_timer: f64,
    pub death_sound_played: bool,
    pub new_best_shown: bool,
    pub had_new_best: bool,
    pub last_percent: f64,
    pub best_percent: f64,
    pub total_deaths: f64,
    pub respawn_delay: f64,
    pub start_pos_index: Option<usize>,
    pub sp: crate::startpos::StartPosState,
    pub mb: crate::macrobot::MacroBot,
    /// `window.speedHack` (0 / NaN count as 1, like `|| 1`)
    pub speed_hack: f64,
    pub start: StartOptions,
    pub start_positions: Vec<StartPos>,
    pub show_hitboxes: bool, pub hitbox_trail_on: bool, pub hitboxes_on_death: bool,
    pub hitbox_trail: [Vec<crate::hitbox::TrailPoint>; 2],
    pub p: Player,
    pub p2: Player,
    /// visual-only events since the host last drained them (see `wd_pv_events`)
    pub vlog: Vec<String>,
    pub g: Globals,
    pub e: Env,
    pub cols: Vec<Col>,
    // `_collisionSections`: a JS array, so only indices below 2^32 - 1 count; the length is the highest index + 1
    pub(crate) sections: std::collections::BTreeMap<u64, Vec<usize>>,
    pub(crate) sections_len: u64,
    /// `col.disabled` (set by toggle triggers), parallel to `cols`
    pub disabled: Vec<bool>,
    pub trig: crate::triggers::Triggers,
    pub fx: crate::fx::Fx,
    pub mirror: crate::mirror::Mirror,
    pub coins: crate::coins::CoinState,
    pub beasts: crate::beast::Beasts,
    pub delta_buffer: f64,
    pub space_was_down: bool,
    pub physics_frame: f64,
    pub log: Vec<String>,
}

/// `PlayerState.reset()` + `PlayerObject.reset()` values at the start of an attempt.
pub fn fresh_player(start_x: f64) -> Player {
    let mut p = Player::default();
    p.y = 30.0;
    p.last_y = 30.0;
    p.last_ground_pos_y = 30.0;
    p.on_ground = true;
    p.can_jump = true;
    p.ball_normal_rotate = 0.86;
    p.ball_rotation2 = 0.86;
    p.nc_accuracy = 100.0;
    p.gd_last_slope_steepness = 0.0;
    p.gd_last_slope_up = Some(false);
    p.gd_slope_rotation = 0.0;
    p.prev_slope_angle = f64::NAN;
    p.last_collision_world_x = Some(start_x);
    p.last_collision_world_y = Some(30.0);
    p
}

impl Game {
    /// Start of an attempt from the level settings (`kA4` speed key, `kA2` game mode), as in `_restartLevel`.
    pub fn start_attempt(&mut self, speed_key: i64, game_mode: i64, start_x: f64) {
        self.p = fresh_player(start_x);
        self.e.player_world_x = start_x;
        self.begin_modes(speed_key, game_mode);
    }

    /// The shared tail of an attempt start: level ground state, start speed, start game mode.
    fn begin_modes(&mut self, speed_key: i64, game_mode: i64) {
        self.e.fly_ground_active = false;
        self.e.fly_visual_only = false;
        self.e.fly_floor_y = None;
        self.e.fly_ceiling_y = None;
        self.e.fly_camera_target = None; // `resetGroundState`: the camera stops following the fly area
        self.reset_ground_camera();
        self.e.portal_free_mode = false;
        self.e.floor_y = 0.0;
        self.e.ceiling_y = None;
        use crate::physics::speed_portal as sp;
        self.g.player_speed = match speed_key { 1 => sp::HALF, 2 => sp::TWO_TIMES, 3 => sp::THREE_TIMES, 4 => sp::FOUR_TIMES, _ => sp::ONE_TIMES };
        self.delta_buffer = 0.0;
        self.space_was_down = false;
        let mut sim = Sim::new(&mut self.p, &mut self.g, &mut self.e);
        match game_mode {
            1 => sim.enter_ship_mode(None, false, false),
            2 => sim.enter_ball_mode(Some(30.0), false),
            3 => sim.enter_ufo_mode(None, false, false),
            4 => sim.enter_wave_mode(None, false),
            5 => sim.enter_robot_mode(false),
            6 => sim.enter_spider_mode(None, false, false),
            7 => sim.enter_swing_mode(None, false, false),
            _ => {}
        }
        self.log.extend(sim.log);
    }

    /// `PlayerState.reset()` + `PlayerObject.reset()`: only the fields those two reset; everything else keeps its value from the last attempt.
    fn reset_player_for_restart(&mut self) {
        crate::pdual::reset_state(&mut self.p);
        crate::pdual::reset_object(&mut self.p, self.e.player_world_x);
    }

    /// `resetObjects`: everything the player activated is available again.
    pub fn reset_objects(&mut self) {
        for c in &mut self.cols {
            c.activated = false;
            c.act_main = false;
            c.act_dual = false;
            c.dash_hold_main = 0.0;
            c.dash_hold_dual = 0.0;
        }
        self.reset_coins();
    }

    /// `_applyLevelStartOptions`
    pub fn apply_level_start_options(&mut self) {
        self.p.is_mini = self.start.mini;
        self.p.gravity_flipped = self.start.gravity_flipped;
        self.p.mirrored = self.start.mirrored;
        self.mirror_cancel();
        if self.start.dual { self.set_dual(true); }
    }

    /// `level.fastForwardTriggers(targetX)` for the trigger kinds that are ported.
    pub fn fast_forward_triggers(&mut self, target_x: f64) {
        for ti in 0..self.trig.l.colors.len() {
            let t = self.trig.l.colors[ti].clone();
            if t.touch { continue; }
            if t.x <= target_x {
                self.cm.trigger_color(t.index, t.color, 0.0, t.copy, t.opacity);
                self.set_channel_blending(t.index, t.blending.unwrap_or(false));
            } else { break; }
        }
        for ti in 0..self.trig.l.toggles.len() {
            let t = &self.trig.l.toggles[ti];
            if t.touch || t.b.spawn_triggered { continue; }
            if t.x <= target_x { self.execute_toggle_pub(ti); } else { break; }
        }
        self.check_animation(target_x);
        self.check_pickup(target_x);
        self.check_count(target_x);
        self.check_collision_triggers(target_x);
        self.check_instant_count(target_x);
        self.check_on_death(target_x);
        self.check_touch_triggers(target_x);
    }

    /// The trigger checks `loadLevel` makes right after spawning the objects.
    pub fn after_load(&mut self) {
        self.check_toggle(0.0); self.check_pickup(0.0); self.check_count(0.0); self.check_collision_triggers(0.0);
        self.check_instant_count(0.0); self.check_on_death(0.0); self.check_touch_triggers(0.0);
    }

    /// `_restartLevel` (not practice mode): level and trigger reset, then the player starts from the level options or a start position.
    pub fn restart_level(&mut self, start_pos: Option<usize>) {
        if self.level_won && self.practice_mode { self.practice_mode = false; self.checkpoints.clear(); }
        if self.practice_mode && !self.checkpoints.is_empty() { self.respawn_from_checkpoint(); return; }
        self.checkpoints.clear();
        self.reset_gameplay_state();
        self.reset_objects();
        self.reset_color();
        self.reset_rotate(); self.reset_alpha(); self.reset_shake(); self.reset_player_vis(); self.reset_animation(); self.reset_enter_effect(); self.reset_pulse(); self.reset_spawn(); self.reset_move(); self.reset_stop(); self.reset_follow_y(); self.reset_follow();
        self.reset_channel_blending();
        self.reset_toggle(); self.reset_on_death(); self.reset_touch(); self.reset_pickup(); self.reset_count(); self.reset_collision(); self.reset_instant_count(); self.reset_generic(false);
        self.check_toggle(0.0); self.check_pickup(0.0); self.check_count(0.0); self.check_collision_triggers(0.0); self.check_instant_count(0.0);
        self.check_on_death(0.0); self.check_touch_triggers(0.0);
        self.cm.reset();
        self.physics_frame = 0.0;
        self.e.player_world_x = 0.0;
        self.reset_player_for_restart();
        self.e.is_dual = false;
        crate::pdual::reset_state(&mut self.p2);
        crate::pdual::reset_object(&mut self.p2, self.e.player_world_x);
        self.p.nc_total_frames = 0.0; self.p.nc_death_frames = 0.0; self.p.nc_deaths = 0.0;
        use crate::physics::speed_portal as sp;
        let speeds = [sp::ONE_TIMES, sp::HALF, sp::TWO_TIMES, sp::THREE_TIMES, sp::FOUR_TIMES];
        let mut pos = start_pos.and_then(|i| self.start_positions.get(i).copied());
        if self.sp.smart {
            if let Some(p) = pos.as_mut() {
                let s = self.smart_start_settings(p.x);
                p.game_mode = Some(s.game_mode); p.mini = Some(s.mini as i64); p.speed = Some(s.speed); p.dual = Some(s.dual as i64); p.gravity_flipped = s.gravity_flipped; p.mirrored = Some(s.mirrored as i64);
            }
        }
        if let Some(pos) = pos {
            let y = if pos.y.is_finite() { pos.y } else { 30.0 };
            self.e.player_world_x = pos.x;
            self.p.y = y; self.p.last_y = y; self.p.last_ground_pos_y = y;
            self.begin_ground();
            self.enter_start_mode(pos.game_mode.unwrap_or(-1), Some(y));
            self.p.gravity_flipped = pos.gravity_flipped;
            self.p.is_mini = pos.mini.map_or(false, |v| v != 0);
            self.g.player_speed = pos.speed.and_then(|s| if s >= 0 { speeds.get(s as usize).copied() } else { None }).unwrap_or(f64::NAN);
            self.p.mirrored = pos.mirrored.map_or(false, |v| v != 0);
            self.mirror_cancel();
            if pos.dual.map_or(false, |v| v != 0) { self.set_dual(true); }
            self.fast_forward_triggers(pos.x);
            self.p.last_collision_world_x = if self.e.player_world_x.is_finite() { Some(self.e.player_world_x) } else { None };
            self.p.last_collision_world_y = Some(y);
        } else {
            if self.start.speed_key >= 0 { if let Some(v) = speeds.get(self.start.speed_key as usize) { self.g.player_speed = *v; } }
            self.begin_ground();
            self.enter_start_mode(self.start.game_mode, None);
            self.apply_level_start_options();
        }
        if self.mb.recording { self.macro_clear_recording(); }
        if self.mb.playing { self.macro_clear_playback(); }
        self.update_camera_y(0.0, true);
        // `_restartLevel` clears the ground (`resetGroundState`) after entering the start mode, whichever way the restart came
        self.render_restart();
        self.apply_fly_log();
    }

    /// `_startEditorPlaytest` from the level state reset on: `_resetEditorPlaytestLevelState`, the player reset, then
    /// `_applyEditorPlaytestStartPosition` for the furthest right start position, or `_applyEditorPlaytestStartMode` (the level's options)
    /// when there is none. Unlike `_restartLevel` the speed is applied before the mode, a missing value counts as 0, the mirror is never
    /// switched on and the player starts on the ground.
    pub fn editor_playtest_start(&mut self) {
        self.checkpoints.clear();
        self.practice_mode = false;
        self.reset_gameplay_state();
        self.reset_objects();
        self.reset_color();
        self.reset_rotate(); self.reset_alpha(); self.reset_shake(); self.reset_player_vis(); self.reset_animation(); self.reset_enter_effect(); self.reset_pulse(); self.reset_spawn(); self.reset_move(); self.reset_stop(); self.reset_follow_y(); self.reset_follow();
        self.reset_channel_blending();
        self.reset_toggle(); self.reset_on_death(); self.reset_touch(); self.reset_pickup(); self.reset_count(); self.reset_collision(); self.reset_instant_count(); self.reset_generic(false);
        self.check_toggle(0.0); self.check_pickup(0.0); self.check_count(0.0); self.check_collision_triggers(0.0); self.check_instant_count(0.0);
        self.check_on_death(0.0); self.check_touch_triggers(0.0);
        self.cm.reset();
        self.begin_ground();
        self.physics_frame = 0.0;
        self.e.player_world_x = 0.0;
        self.reset_player_for_restart();
        self.e.is_dual = false;
        crate::pdual::reset_state(&mut self.p2);
        crate::pdual::reset_object(&mut self.p2, self.e.player_world_x);
        self.p.nc_total_frames = 0.0; self.p.nc_death_frames = 0.0; self.p.nc_deaths = 0.0;
        use crate::physics::speed_portal as sp;
        let speeds = [sp::ONE_TIMES, sp::HALF, sp::TWO_TIMES, sp::THREE_TIMES, sp::FOUR_TIMES];
        self.p.y = 30.0; self.p.on_ground = true; self.p.can_jump = true;
        // `_getLatestEditorStartPosition`: sorted by x (then by object), the last one
        let pos = self.start_positions.last().copied();
        if let Some(pos) = pos {
            let y = if pos.y.is_finite() { pos.y } else { 30.0 };
            let x = if pos.x.is_finite() { pos.x } else { 0.0 };
            self.e.player_world_x = x;
            self.p.y = y; self.p.last_y = y; self.p.last_ground_pos_y = y;
            self.p.on_ground = true; self.p.can_jump = true;
            let speed = pos.speed.unwrap_or(0);
            self.g.player_speed = if speed >= 0 { speeds.get(speed as usize).copied().unwrap_or(sp::ONE_TIMES) } else { sp::ONE_TIMES };
            self.enter_start_mode(pos.game_mode.unwrap_or(0), Some(y));
            self.p.is_mini = pos.mini.unwrap_or(0) == 1;
            self.p.gravity_flipped = pos.gravity_flipped;
            self.p.mirrored = false; self.p2.mirrored = false;
            if pos.dual.unwrap_or(0) == 1 { self.set_dual(true); self.p2.mirrored = false; }
            self.fast_forward_triggers(x);
            self.p.last_collision_world_x = if self.e.player_world_x.is_finite() { Some(self.e.player_world_x) } else { None };
            self.p.last_collision_world_y = Some(y);
            self.camera_x = self.e.player_world_x - self.g.center_x + crate::triggers::or0(self.trig.gen.camera_offset.0) + crate::triggers::or0(self.trig.gen.gameplay_offset.0);
        } else {
            if self.start.speed_key >= 0 { if let Some(v) = speeds.get(self.start.speed_key as usize) { self.g.player_speed = *v; } }
            self.enter_start_mode(self.start.game_mode, Some(30.0));
            self.p.is_mini = self.start.mini;
            self.p.gravity_flipped = self.start.gravity_flipped;
            self.p.mirrored = false; self.p2.mirrored = false;
            if self.start.dual { self.set_dual(true); self.p2.mirrored = false; }
        }
        self.update_camera_y(0.0, true);
    }

    /// The level-layer part of an attempt start (`resetGroundState`).
    fn begin_ground(&mut self) {
        self.e.fly_ground_active = false; self.e.fly_visual_only = false; self.e.fly_floor_y = None; self.e.fly_ceiling_y = None; self.e.fly_camera_target = None; self.reset_ground_camera();
        self.e.floor_y = 0.0; self.e.ceiling_y = None;
        self.e.portal_free_mode = false;
        self.delta_buffer = 0.0; self.space_was_down = false;
    }

    fn enter_start_mode(&mut self, game_mode: i64, y: Option<f64>) {
        let mut sim = Sim::new(&mut self.p, &mut self.g, &mut self.e);
        match game_mode {
            1 => sim.enter_ship_mode(None, false, false),
            2 => sim.enter_ball_mode(Some(y.unwrap_or(30.0)), false),
            3 => sim.enter_ufo_mode(None, false, false),
            4 => sim.enter_wave_mode(None, false),
            5 => sim.enter_robot_mode(false),
            6 => sim.enter_spider_mode(None, false, false),
            7 => sim.enter_swing_mode(None, false, false),
            _ => {}
        }
        self.log.extend(sim.log);
    }

    /// `_restartLevel` without a start position or practice mode: level state, triggers and the player go back to the start.
    pub fn restart(&mut self, speed_key: i64, game_mode: i64, start_x: f64) {
        self.reset_objects();
        self.reset_rotate(); self.reset_spawn(); self.reset_move(); self.reset_stop(); self.reset_follow_y(); self.reset_follow();
        self.reset_toggle(); self.reset_on_death(); self.reset_touch(); self.reset_pickup(); self.reset_count(); self.reset_collision(); self.reset_instant_count(); self.reset_generic(false);
        self.check_toggle(0.0); self.check_pickup(0.0); self.check_count(0.0); self.check_collision_triggers(0.0); self.check_instant_count(0.0);
        self.check_on_death(0.0); self.check_touch_triggers(0.0);
        self.physics_frame = 0.0;
        self.e.player_world_x = start_x;
        self.reset_player_for_restart();
        self.e.is_dual = false;
        crate::pdual::reset_state(&mut self.p2);
        crate::pdual::reset_object(&mut self.p2, self.e.player_world_x);
        self.p.nc_total_frames = 0.0; self.p.nc_death_frames = 0.0; self.p.nc_deaths = 0.0;
        self.begin_modes(speed_key, game_mode);
    }

    pub fn new(cols: Vec<Col>) -> Game {
        let mut sections: std::collections::BTreeMap<u64, Vec<usize>> = std::collections::BTreeMap::new();
        let mut sections_len = 0u64;
        for (i, c) in cols.iter().enumerate() {
            let raw = (c.x / 400.0).floor();
            let idx = if raw.is_nan() { f64::NAN } else { raw.max(0.0) };
            if !idx.is_finite() || idx >= 4294967295.0 { continue; }
            let idx = idx as u64;
            sections.entry(idx).or_default().push(i);
            sections_len = sections_len.max(idx + 1);
        }
        let mut e = Env::default();
        e.manage_layer = true;
        e.has_level_events = true;
        let mut trig = crate::triggers::Triggers::default();
        trig.phase_limit = u32::MAX;
        trig.time_warp = 1.0;
        trig.meta = cols.iter().map(|c| crate::triggers::ColMeta {
            groups: Vec::new(), init_x: c.x, init_y: c.y, init_rot: if c.rotation_degrees == 0.0 || c.rotation_degrees.is_nan() { 0.0 } else { c.rotation_degrees },
            toggled: Default::default(), sec: crate::triggers::section_index(c.x), init_wh: None,
        }).collect();
        let disabled = vec![false; cols.len()];
        Game { cm: crate::colors::ColorManager::new(327424.0, 64511.0), channel_blending: Default::default(), channel_blending_initial: Default::default(), ldm: false, settings: Default::default(), practice_mode: false, checkpoints: Vec::new(), screen_width: 1138.0, end_x_pos: 0.0, end_portal_game_y: 240.0, camera_x: -419.0, camera_y: 0.0, follow_camera_y: None, rotate_gameplay_enabled: true, sideways_proxies: Default::default(), level_won: false, end_camera_override: false, end_cam: None, death_timer: 0.0, death_sound_played: false, new_best_shown: false, had_new_best: false, last_percent: 0.0, best_percent: 0.0, total_deaths: 0.0, respawn_delay: 1.0, start_pos_index: None, mirror: Default::default(), coins: Default::default(), beasts: Default::default(), sp: Default::default(), mb: Default::default(), speed_hack: 1.0, start: StartOptions::default(), start_positions: Vec::new(), show_hitboxes: false, hitbox_trail_on: false, hitboxes_on_death: false, hitbox_trail: [Vec::new(), Vec::new()], p: Player::default(), p2: Player::default(), g: Globals::default(), e, cols, sections, sections_len, disabled, trig, fx: Default::default(), delta_buffer: 0.0, space_was_down: false, physics_frame: 0.0, log: Vec::new(), vlog: Vec::new() }
    }

    /// Everything the level provides to the simulation: colliders (with their groups) and triggers.
    pub fn from_level(level: &crate::level::Level) -> Game { Game::from_level_official(level, 2) }

    /// `official`: 1 = official level, 0 = other level, anything else = keep every coin (tests)
    pub fn from_level_official(level: &crate::level::Level, official: u32) -> Game {
        let objects = crate::level::filter_coins(crate::level::normalize_teleport_portals(level.objects.clone()), official);
        let mut cols: Vec<Col> = Vec::new();
        let mut groups: Vec<Vec<i64>> = Vec::new();
        let mut toggle_blocks: std::collections::BTreeMap<usize, crate::collider::ToggleBlock> = Default::default();
        for o in &objects {
            if let Some(list) = crate::collider::spawn_colliders(o) {
                for c in list.iter().filter(|c| !c.placeholder) {
                    if let Some(tb) = c.toggle { toggle_blocks.insert(cols.len(), tb); }
                    cols.push(Col::from_collider(c, cols.len() as f64));
                    groups.push(if c.toggle.is_some() { Vec::new() } else { crate::triggers::object_groups(&o.groups) });
                }
            }
        }
        // which colliders are coins, and the linked object id the saved progress refers to (the object's index in the loaded list)
        let mut coin_link: std::collections::BTreeMap<usize, (bool, i64)> = Default::default();
        {
            let mut idx = 0usize;
            for (k, o) in objects.iter().enumerate() {
                if let Some(list) = crate::collider::spawn_colliders(o) {
                    for c in list.iter().filter(|c| !c.placeholder) {
                        if c.coin_id == Some(142) { coin_link.insert(idx, (false, k as i64)); }
                        if c.coin_id == Some(1329) { coin_link.insert(idx, (true, k as i64)); }
                        idx += 1;
                    }
                }
            }
        }
        let mut g = Game::new(cols);
        g.trig.gen.toggle_blocks = toggle_blocks;
        g.coins.link = coin_link;
        for (i, gr) in groups.into_iter().enumerate() {
            for &gid in &gr { g.trig.group_cols.entry(gid).or_default().push(i); }
            g.trig.meta[i].groups = gr;
        }
        g.trig.l = crate::triggers::build_lists(&objects);
        { let mut gs = std::mem::take(&mut g.trig.gen); crate::generic::build_generic_extras(&mut g.trig.l, &mut gs); g.trig.gen = gs; }
        g.sp_load(&objects);
        g.settings = crate::settings::LevelSettings::parse(&level.settings);
        for (ch, c) in &g.settings.initial_colors { g.cm.set_initial_color(*ch, *c); }
        for (ch, c) in &g.settings.initial_copies { g.cm.set_initial_copy(*ch, *c); }
        for (ch, v) in &g.settings.initial_opacity { g.cm.set_initial_opacity(*ch, *v); }
        g.cm.reset();
        g.channel_blending = g.settings.channel_blending.clone();
        g.channel_blending_initial = g.channel_blending.clone();
        let mut last_x = 0.0f64;
        for o in &objects {
            if crate::objects::is_trigger_branch(o.id.unwrap_or(0)) { continue; }
            let wx = o.x * 2.0;
            if wx > last_x { last_x = wx; }
        }
        g.end_x_pos = (g.screen_width + 1200.0).max(last_x + 680.0);
        let settings: Vec<&str> = level.settings.split(',').collect();
        let get = |k: &str| -> i64 {
            let mut v = "0";
            let mut i = 0;
            while i + 1 < settings.len() { if settings[i] == k { v = settings[i + 1]; } i += 2; }
            crate::js::parse_int(if v.is_empty() { "0" } else { v }).unwrap_or(0)
        };
        g.start = StartOptions { game_mode: get("kA2"), speed_key: get("kA4"), mini: get("kA3") == 1, gravity_flipped: get("kA11") == 1, mirrored: get("kA28") == 1, dual: get("kA8") == 1 };
        for o in &objects {
            if matches!(o.id, Some(31) | Some(34)) {
                g.start_positions.push(StartPos { x: 2.0 * o.x, y: 2.0 * o.y, game_mode: o.game_mode, mini: o.mini_mode, speed: o.speed, dual: o.dual_mode, mirrored: o.mirrored, gravity_flipped: o.flip_gravity });
            }
        }
        g.start_positions.sort_by(|a, b| (a.x - b.x).partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal));
        g
    }

    /// `getNearbySectionObjects(worldX)`: indices into `cols`, in section order.
    pub fn nearby(&self, world_x: f64) -> Vec<usize> {
        if self.sections_len == 0 { return Vec::new(); }
        let mid_raw = (world_x / 400.0).floor();
        let mid = if mid_raw.is_nan() { f64::NAN } else { mid_raw.max(0.0) };
        // `Math.max(0, Math.floor(x))`: NaN stays NaN and the loop below then runs zero times
        let lo = (mid - 1.0).max(0.0);
        let hi = ((self.sections_len - 1) as f64).min(mid + 1.0);
        let mut out = Vec::new();
        if lo.is_nan() || hi.is_nan() { return out; }
        let mut i = lo;
        while i <= hi {
            if let Some(s) = self.sections.get(&(i as u64)) { out.extend(s.iter().copied().filter(|&c| !self.disabled[c])); }
            i += 1.0;
        }
        out
    }

    fn player(&self, second: bool) -> &Player { if second { &self.p2 } else { &self.p } }

    /// A `Sim` for the primary (`second = false`) or secondary player, with the other one reachable for dual-mode code.
    pub(crate) fn sim_for_pub<R>(&mut self, second: bool, f: impl FnOnce(&mut Sim) -> R) -> R { self.sim_for(second, f) }
    fn sim_for<R>(&mut self, second: bool, f: impl FnOnce(&mut Sim) -> R) -> R {
        self.e.second_player = second;
        if self.coins.enabled { self.e.practice_mode = self.practice_mode; }
        let from = self.log.len();
        let mut orb_hits: Vec<(f64, f64)> = Vec::new();
        let r = {
            let (p, other) = if second { (&mut self.p2, &mut self.p) } else { (&mut self.p, &mut self.p2) };
            let mut sim = Sim::new(p, &mut self.g, &mut self.e);
            sim.other = Some(other);
            if self.coins.enabled { sim.coins = Some(&mut self.coins); }
            let r = f(&mut sim);
            self.log.extend(sim.log);
            for l in &sim.vlog { if let Some(rest) = l.strip_prefix("orb:") { if let Some((x, y)) = rest.split_once(',') { if let (Ok(x), Ok(y)) = (x.parse::<f64>(), y.parse::<f64>()) { orb_hits.push((x, y)); } } } }
            self.vlog.extend(sim.vlog);
            r
        };
        for (x, y) in orb_hits { self.mark_orb_sprites(x, y); }
        self.consume_mirror_log(from);
        self.e.second_player = false;
        r
    }

    /// `_isTouchingAvailableOrbForInput`'s scan (without the "press already consumed" check, which `Sim` applies).
    fn scan_orb_touch(&self, second: bool) -> bool {
        let pwx = self.e.player_world_x;
        let pl = self.player(second);
        let size = if pl.is_mini { 18.0 } else { 30.0 };
        for o in &self.near_cols(pwx) {
            if o.ty != 6.0 { continue; }
            let activated = if second { o.act_dual } else { o.act_main };
            if !o.multi_activate && activated { continue; }
            if let Some(r) = o.hitbox_radius {
                let (dx, dy) = (pwx - o.x, pl.y - o.y);
                let hit = r + size;
                if dx * dx + dy * dy <= hit * hit { return true; }
                continue;
            }
            let rad = (if o.rotation_degrees == 0.0 || o.rotation_degrees.is_nan() { 0.0 } else { o.rotation_degrees }) * std::f64::consts::PI / 180.0;
            let (cos, sin) = (m::cos(rad), m::sin(rad));
            let (half_w, half_h) = (o.w / 2.0, o.h / 2.0);
            let rhw = (half_w * cos).abs() + (half_h * sin).abs();
            let rhh = (half_w * sin).abs() + (half_h * cos).abs();
            let (left, right, top, bottom) = (o.x - rhw, o.x + rhw, o.y - rhh, o.y + rhh);
            if !(pwx + size <= left) && !(pwx - size >= right) && !(pl.y + size <= top) && !(pl.y - size >= bottom) { return true; }
        }
        false
    }

    fn update_jump(&mut self, second: bool, dt: f64) {
        self.e.orb_touch = self.scan_orb_touch(second);
        let near = self.near_cols(self.e.player_world_x);
        self.sim_for(second, |sim| sim.update_jump(dt, &near));
    }

    pub fn sync_floor_ceiling(&mut self) {
        if self.trig.gen.gameplay_sideways {
            self.e.floor_y = -1e9;
            self.e.ceiling_y = None;
        } else {
            self.e.floor_y = if self.e.fly_ground_active { if self.e.fly_visual_only { 0.0 } else { self.e.fly_floor_y.unwrap_or(f64::NAN) } } else { 0.0 };
            self.e.ceiling_y = self.e.fly_ceiling_y;
        }
    }

    fn near_cols(&self, world_x: f64) -> Vec<Col> {
        if self.trig.gen.gameplay_sideways { self.nearby(self.trig.gen.sideways_world_x).into_iter().map(|i| self.sideways_proxy(i)).collect() }
        else { self.nearby(world_x).into_iter().map(|i| self.cols[i].clone()).collect() }
    }

    fn swap_sideways_frame(&mut self) {
        std::mem::swap(&mut self.e.player_world_x, &mut self.p.y);
        self.e.in_sideways_step = !self.e.in_sideways_step;
    }

    fn sideways_proxy(&self, i: usize) -> Col {
        let col = &self.cols[i];
        let mut p = col.clone();
        if let Some(own) = self.sideways_proxies.get(&i) {
            p.dash_hold_main = own.dash_hold_main; p.dash_hold_dual = own.dash_hold_dual; p.ee_toggle_press = own.ee_toggle_press;
            p.secret_completed = own.secret_completed; p.user_completed = own.user_completed;
        }
        p.x = col.y;
        p.y = col.x;
        let rot = col.rotation_degrees;
        if col.ty != crate::pcol::type_code("slope") && (rot % 180.0).abs() == 90.0 {
            p.w = col.w; p.h = col.h; p.rotation_degrees = 0.0;
        } else {
            p.w = col.h; p.h = col.w; p.rotation_degrees = -rot;
        }
        p.hypo_ax = col.hypo_ay; p.hypo_ay = col.hypo_ax;
        p.hypo_bx = col.hypo_by; p.hypo_by = col.hypo_bx;
        p.right_ax = col.right_ay; p.right_ay = col.right_ax;
        p
    }

    fn check_collisions_sideways(&mut self, second: bool, offset: f64) {
        let idxs = self.nearby(self.trig.gen.sideways_world_x);
        let mut near: Vec<Col> = idxs.iter().map(|&i| self.sideways_proxy(i)).collect();
        let final_idx = self.sim_for(second, |sim| {
            sim.cands = Vec::new();
            sim.near_idx = idxs.clone();
            sim.replace_idx = None;
            sim.replace_cols = Default::default();
            sim.check_collisions(offset, &mut near);
            std::mem::take(&mut sim.near_idx)
        });
        self.sync_floor_ceiling();
        for (k, &i) in final_idx.iter().enumerate() {
            if let Some(c) = near.get(k) {
                let src = &mut self.cols[i];
                src.act_main = c.act_main; src.act_dual = c.act_dual; src.activated = c.activated;
                self.sideways_proxies.insert(i, c.clone());
            }
        }
    }

    fn check_collisions(&mut self, second: bool, offset: f64) {
        if self.trig.gen.gameplay_sideways { self.check_collisions_sideways(second, offset); return; }
        let piece_width = offset + self.g.center_x;
        let idxs = self.nearby(piece_width);
        let mut near: Vec<Col> = idxs.iter().map(|&i| self.cols[i].clone()).collect();
        // dual-mode portal: `_getInitialDualPortalMode` needs the colliders around the previous position too
        let mut cands: Vec<Col> = Vec::new();
        let mut replace_idx: Option<Vec<usize>> = None;
        let mut replace_cols: std::collections::BTreeMap<usize, Col> = Default::default();
        if !second && !self.e.is_dual && near.iter().any(|c| c.ty == 24.0) {
            let world_x = self.e.player_world_x;
            let prev_raw = self.p.last_collision_world_x.unwrap_or(0.0);
            let prev_x = if prev_raw.is_finite() { prev_raw } else { world_x };
            let list = if prev_x != world_x { let l = self.nearby(prev_x); replace_idx = Some(l.clone()); l } else { self.nearby(world_x) };
            for &i in &list { cands.push(self.cols[i].clone()); }
            if let Some(r) = &replace_idx { for &i in r { replace_cols.insert(i, self.cols[i].clone()); } }
        }
        let (final_idx, stash) = self.sim_for(second, |sim| {
            sim.cands = cands;
            sim.near_idx = idxs.clone();
            sim.replace_idx = replace_idx;
            sim.replace_cols = replace_cols;
            sim.check_collisions(offset, &mut near);
            (std::mem::take(&mut sim.near_idx), std::mem::take(&mut sim.stash))
        });
        for (i, c) in stash { self.cols[i] = c; }
        for (k, &i) in final_idx.iter().enumerate() { self.cols[i] = near[k].clone(); }
    }

    /// The scene's `_enableDualMode()` / `_disableDualMode()` called from outside a collision check.
    pub fn set_dual(&mut self, on: bool) {
        let mut cands: Vec<Col> = Vec::new();
        if on && !self.e.is_dual {
            let world_x = self.e.player_world_x;
            let prev_raw = self.p.last_collision_world_x.unwrap_or(0.0);
            let prev_x = if prev_raw.is_finite() { prev_raw } else { world_x };
            let list = if prev_x != world_x { self.nearby(prev_x) } else { self.nearby(world_x) };
            for &i in &list { cands.push(self.cols[i].clone()); }
        }
        self.sim_for(false, |s| { if on { s.cands = cands; s.enable_dual_mode(None); } else { s.disable_dual_mode(); } });
    }

    /// `_pushButton`
    pub fn push_button(&mut self) {
        if self.p.is_dead { return; }
        self.p.press_count = Some(self.p.press_count.unwrap_or(0.0) + 1.0);
        self.p2.press_count = Some(self.p2.press_count.unwrap_or(0.0) + 1.0);
        self.p.up_key_down = true;
        self.p.up_key_pressed = true;
        self.p.queued_hold = true;
        self.p.orb_activation_consumed_for_press = false;
        if self.e.is_dual && !self.p2.is_dead {
            self.p2.up_key_down = true;
            self.p2.up_key_pressed = true;
            self.p2.queued_hold = true;
            self.p2.orb_activation_consumed_for_press = false;
        }
        let grav_before = self.p.gravity_flipped;
        let mut jumped = false;
        if !self.p.is_flying && !self.p.is_wave && !self.p.is_ufo && !self.p.is_swing && self.p.can_jump {
            self.update_jump(false, 0.0);
            jumped = true;
        } else if self.p.is_ufo {
            self.e.orb_touch = self.scan_orb_touch(false);
            let prioritize = { let sim = Sim::new(&mut self.p, &mut self.g, &mut self.e); sim.should_prioritize_ufo_orb_input_pub() };
            if !prioritize { self.update_jump(false, 0.0); jumped = true; }
        }
        let mut synced = false;
        if self.e.is_dual && self.p.gravity_flipped != grav_before {
            let o = SyncOpts { skip_ball_input: self.p.is_ball && jumped, skip_spider_input: self.p.is_spider && jumped, force: false };
            synced = self.sim_for(false, |s| s.sync_globals_from_primary(o));
        }
        if self.e.is_dual && !self.p2.is_dead {
            if synced && self.p2.is_ball && self.p2.up_key_pressed { self.p2.up_key_pressed = false; self.p2.queued_hold = false; }
            let before = self.p2.gravity_flipped;
            let ball_in = self.p2.is_ball && self.p2.up_key_pressed;
            let spider_in = self.p2.is_spider && self.p2.up_key_pressed;
            if !self.p2.is_flying && !self.p2.is_wave && !self.p2.is_ufo && !self.p2.is_swing && self.p2.can_jump {
                self.update_jump(true, 0.0);
            } else if self.p2.is_ufo {
                self.e.orb_touch = self.scan_orb_touch(true);
                self.e.second_player = true;
                let prioritize = { let sim = Sim::new(&mut self.p2, &mut self.g, &mut self.e); sim.should_prioritize_ufo_orb_input_pub() };
                self.e.second_player = false;
                if !prioritize { self.update_jump(true, 0.0); }
            }
            if self.p2.gravity_flipped != before {
                let o = SyncOpts { skip_ball_input: ball_in, skip_spider_input: spider_in, force: false };
                self.sim_for(true, |s| s.sync_globals_from_secondary(o));
            }
        }
    }

    /// `_releaseButton`
    pub fn release_button(&mut self) {
        for p in [&mut self.p, &mut self.p2] {
            p.robot_touched_pad = true;
            p.up_key_down = false;
            p.up_key_pressed = false;
            p.queued_hold = false;
            p.orb_activation_consumed_for_press = false;
        }
    }

    /// `_quantizeDelta`: returns the quantised delta in frames (60 Hz units).
    pub(crate) fn slide_quantize(&mut self, delta_ms: f64) -> f64 { self.quantize_delta(delta_ms) }
    pub fn quantize_delta_pub(&mut self, delta_ms: f64) -> f64 { self.quantize_delta(delta_ms) }
    fn quantize_delta(&mut self, delta_ms: f64) -> f64 {
        let speed = (if self.speed_hack == 0.0 || self.speed_hack.is_nan() { 1.0 } else { self.speed_hack }) * (if self.trig.time_warp == 0.0 || self.trig.time_warp.is_nan() { 1.0 } else { self.trig.time_warp });
        let buf = (delta_ms * speed) / 1000.0 + self.delta_buffer;
        let mut n = crate::physics::js_round(buf / U);
        if n < 0.0 { n = 0.0; }
        if n > 60.0 { n = 60.0; }
        let q = n * U;
        self.delta_buffer = buf - q;
        q * 60.0
    }

    fn update_rotation(&mut self, second: bool, horizontal: f64, vertical: f64) {
        self.sim_for(second, |sim| {
            if sim.p.is_ball {
                let on_surface = sim.p.on_ground || sim.p.on_ceiling;
                sim.update_ball_roll(horizontal, on_surface);
            } else if sim.p.on_ground {
                sim.update_ground_rotation(vertical);
            } else if sim.p.is_dashing {
                sim.update_dash_rotation(U);
            } else if sim.p.rotate_action_active {
                sim.update_rotate_action(U);
            }
        });
    }

    /// One call of the physics part of `update(time, deltaTime)`.
    pub fn step_frame(&mut self, delta_ms: f64, jump_held: bool) { self.step_frame_with_delta(delta_ms, jump_held); }

    /// `step_frame` that also returns the quantised delta (the camera uses it).
    pub fn step_frame_with_delta(&mut self, delta_ms: f64, jump_held: bool) -> f64 {
        let quantized = self.quantize_delta(delta_ms);
        let mut sub_steps = if quantized > 0.0 { 1f64.max(crate::physics::js_round(quantized * 4.0)) } else { 0.0 };
        if sub_steps > 60.0 { sub_steps = 60.0; }
        let sub_delta = if sub_steps > 0.0 { quantized / sub_steps } else { 0.0 };
        let vertical = sub_delta * D;
        let trigger_direction = if self.trig.gen.direction < 0.0 { -1.0 } else { 1.0 };
        let horizontal = sub_delta * self.g.player_speed * D * trigger_direction;
        let initial_y = self.p.y;
        let initial_y2 = self.p2.y;
        let mut i = 0.0;
        while i < sub_steps {
            let sideways_step = self.trig.gen.gameplay_sideways && !self.e.is_dual;
            if self.trig.gen.gameplay_sideways || self.e.floor_y == -1e9 { self.sync_floor_ceiling(); }
            if sideways_step {
                self.trig.gen.sideways_world_x = self.e.player_world_x;
                self.swap_sideways_frame();
            }
            self.p.last_y = self.p.y;
            self.physics_frame += 1.0;
            self.e.physics_tick = (if self.e.physics_tick == 0.0 || self.e.physics_tick.is_nan() { 0.0 } else { self.e.physics_tick }) + 1.0;
            // _applyJumpInput
            if jump_held && !self.space_was_down { self.push_button(); } else if !jump_held && self.space_was_down { self.release_button(); }
            self.space_was_down = jump_held;
            if self.mb.playing { self.macro_step(); }
            let input = self.p.clone();
            let both_balls_before = self.e.is_dual && self.p.is_ball && self.p2.is_ball;
            let grav_before = self.p.gravity_flipped;
            let shared_before = dual_shared_signature(&self.p);
            self.update_jump(false, vertical);
            self.p.y += self.p.y_velocity * vertical;
            self.check_collisions(false, self.e.player_world_x - self.g.center_x);
            let grav_changed = self.e.is_dual && self.p.gravity_flipped != grav_before;
            let wave_gravity_sync = grav_changed && self.p.is_wave && self.p2.is_wave;
            let mut synced = false;
            if self.e.is_dual && !wave_gravity_sync && dual_shared_signature(&self.p) != shared_before {
                let o = SyncOpts { skip_ball_input: grav_changed && self.p.is_ball && input.up_key_pressed, skip_spider_input: grav_changed && self.p.is_spider && input.up_key_pressed, force: false };
                synced = self.sim_for(false, |s| s.sync_globals_from_primary(o));
            }
            if self.e.is_dual && self.p.is_dead && !self.p2.is_dead { self.sim_for(false, |s| s.kill_secondary()); }
            self.e.player_world_x += horizontal;
            if sideways_step { self.swap_sideways_frame(); }
            if self.e.is_dual && !self.p2.is_dead {
                {
                    let p2 = &mut self.p2;
                    p2.up_key_down = input.up_key_down;
                    p2.up_key_pressed = input.up_key_pressed;
                    p2.queued_hold = input.queued_hold;
                    p2.orb_activation_consumed_for_press = input.up_key_down && (p2.orb_activation_consumed_for_press || input.orb_activation_consumed_for_press);
                    if synced && p2.is_ball && p2.up_key_pressed { p2.up_key_pressed = false; p2.queued_hold = false; }
                    p2.last_y = p2.y;
                }
                let shared_before2 = dual_shared_signature(&self.p2);
                let ball_in = self.p2.is_ball && self.p2.up_key_pressed;
                let spider_in = self.p2.is_spider && self.p2.up_key_pressed;
                self.update_jump(true, vertical);
                if !self.p2.up_key_pressed { self.p.up_key_pressed = false; }
                if !self.p2.queued_hold { self.p.queued_hold = false; }
                if self.p2.orb_activation_consumed_for_press { self.p.orb_activation_consumed_for_press = true; }
                self.p2.y += self.p2.y_velocity * vertical;
                if wave_gravity_sync { self.sim_for(false, |s| s.sync_globals_from_primary(SyncOpts::default())); }
                self.check_collisions(true, self.e.player_world_x - self.g.center_x - horizontal);
                if self.e.is_dual && !self.p2.is_dead && dual_shared_signature(&self.p2) != shared_before2 {
                    let o = SyncOpts { skip_ball_input: ball_in, skip_spider_input: spider_in, force: false };
                    self.sim_for(true, |s| s.sync_globals_from_secondary(o));
                }
                if !both_balls_before && self.sim_for(false, |s| s.balls_overlapping()) { self.e.dual_ball_spawn_lock = true; }
                self.sim_for(false, |s| s.resolve_dual_ball_overlap());
                if self.e.is_dual && self.p2.is_dead && !self.p.is_dead { self.sim_for(true, |s| s.kill_secondary_target_primary()); }
                if self.e.is_dual && self.p.is_dead && !self.p2.is_dead { self.sim_for(false, |s| s.kill_secondary()); }
                if self.e.is_dual { self.sim_for(false, |s| s.ensure_dual_fly_bounds()); }
            }
            if !self.p.is_flying && !self.p.is_wave && !self.p.is_ufo && !self.p.is_swing { self.update_rotation(false, horizontal, vertical); }
            if self.e.is_dual && !self.p2.is_dead && !self.p2.is_flying && !self.p2.is_wave && !self.p2.is_ufo && !self.p2.is_swing { self.update_rotation(true, horizontal, vertical); }
            self.push_hitbox_trail();
            if self.mb.playing { self.macro_apply_current(); } else if self.mb.recording { self.macro_record_frame(); }
            if !self.p.is_dead {
                // V2.1 runs the level triggers once per substep, stepped by the substep's time
                let pos = crate::triggers::Pos { player_x: self.e.player_world_x, player_y: self.p.y, camera_x: self.camera_x, camera_y: self.camera_y };
                let dual_alive = self.e.is_dual && !self.p2.is_dead;
                let tin = crate::triggers::Input { p1_hold: self.p.up_key_down, p1_press: self.p.up_key_pressed, p2_hold: dual_alive && self.p2.up_key_down, p2_press: dual_alive && self.p2.up_key_pressed };
                self.trigger_frame(pos, if dual_alive { Some(self.p2.y) } else { None }, sub_delta / 60.0, tin);
            }
            i += 1.0;
        }
        self.p.last_y = initial_y;
        if self.e.is_dual { self.p2.last_y = initial_y2; }
        self.p.ignore_portals = false;
        self.p2.ignore_portals = false;
        quantized
    }
}

impl<'a> Sim<'a> {
    pub fn should_prioritize_ufo_orb_input_pub(&self) -> bool { self.p.is_ufo && self.p.up_key_pressed && self.orb_touching(true) }

    pub fn update_dash_rotation(&mut self, dt: f64) {
        let spin = std::f64::consts::PI * 4.0 * self.flip_mod();
        self.p.rotation += spin * dt;
    }

    pub fn update_rotate_action(&mut self, dt: f64) {
        if !self.p.rotate_action_active { return; }
        self.p.rotate_action_time += dt;
        if self.p.rotate_action_time >= self.p.rotate_action_duration { self.p.rotate_action_active = false; }
        let t = (self.p.rotate_action_time / self.p.rotate_action_duration).min(1.0);
        self.p.rotation = self.p.rotate_action_start + self.p.rotate_action_total * t;
    }

    pub fn update_ball_roll(&mut self, dt: f64, on_surface: bool) {
        if !self.p.ball_should_rotate { return; }
        let mut roll_dir = if self.p.ball_normal_rotate == 0.0 || self.p.ball_normal_rotate.is_nan() { 1.0 } else { self.p.ball_normal_rotate };
        if self.p.ball_rotate_opposite { roll_dir = -roll_dir; }
        let ground = 0.48;
        let mut air = if self.p.ball_rotate_opposite { 0.27 } else { 0.35 };
        if self.p.is_dashing {
            let v = self.p.dash_y_velocity;
            let dash_speed = (if v == 0.0 || v.is_nan() { let y = self.p.y_velocity; if y == 0.0 || y.is_nan() { 0.0 } else { y } } else { v }).abs();
            let scale = 0.15f64.max(1f64.min(dash_speed / 24.0));
            air = 0.32 + scale * 0.32;
        } else if self.p.ball_hit_pad {
            air *= 1.25;
        }
        let speed_factor = if on_surface { ground } else { air };
        let mut mult = 1.0;
        if self.p.ball_rotation2 > 0.0 {
            mult = 2.1;
            self.p.ball_rotation2 -= 1.0;
        }
        let mini_scale = if self.p.is_mini { 1.0 / 0.8 } else { 1.0 };
        self.p.rotation += dt / (BALL_G / 2.0) * roll_dir * speed_factor * mult * mini_scale;
    }
}

/// `game-scene.js` `update()` for one running level: death / win handling, physics, camera and triggers. Rendering, audio and UI are not part of it.
impl Game {
    /// `_updateCameraY(delta, snap)`
    pub fn update_camera_y(&mut self, delta: f64, snap: bool) {
        let zoom = if self.trig.gen.zoom == 0.0 || self.trig.gen.zoom.is_nan() { 1.0 } else { self.trig.gen.zoom };
        let min_y = if zoom != 1.0 { 640.0 / 2.0 * (1.0 / zoom - 1.0) } else { 0.0 };
        let mut piece = self.camera_y;
        let mut target = piece;
        let p = &self.p;
        if let Some(t) = self.e.fly_camera_target {
            target = t;
        } else if self.trig.gen.free_mode_camera && (p.is_flying || p.is_ufo || p.is_wave || p.is_ball || p.is_swing || p.is_spider) {
            let padding = self.trig.gen.free_cam_padding;
            let margin = 2.0 * (160.0 - ((160.0 - 32.0) * padding + 30.0));
            let center = piece - 180.0 + 320.0;
            let y = p.y;
            if y > center + margin { target = y - 320.0 - margin + 180.0; }
            else if y < center - margin { target = y - 320.0 + margin + 180.0; }
            let easing = self.trig.gen.free_cam_easing;
            if !snap && delta != 0.0 {
                piece = if easing > 1.0 { piece + (target - piece) / (easing / delta) } else { target };
                self.camera_y = crate::triggers::js_max(min_y, piece);
                return;
            }
        } else {
            let y = self.p.y;
            let (up, down) = (140.0, 80.0);
            let center = piece - 180.0 + 320.0;
            if self.p.gravity_flipped {
                if y > center + down { target = y - 320.0 - down + 180.0; } else if y < center - up { target = y - 320.0 + up + 180.0; }
            } else if y > center + up { target = y - 320.0 - up + 180.0; } else if y < center - down { target = y - 320.0 + down + 180.0; }
        }
        if target < min_y { target = min_y; }
        if snap { self.camera_y = target; }
        else if delta != 0.0 {
            piece += (target - piece) / (10.0 / delta);
            if piece < min_y { piece = min_y; }
            self.camera_y = piece;
        }
    }

    pub fn camera_y_block(&mut self, q: f64) {
        if let Some(fy) = self.follow_camera_y { if self.trig.gen.static_camera.is_some() { self.camera_y = fy; } }
        self.camera_y -= self.trig.gen.applied_camera_offset_y;
        self.camera_y -= self.trig.gen.applied_gameplay_offset_y;
        self.update_camera_y(q, false);
        self.trig.gen.applied_camera_offset_y = crate::triggers::or0(self.trig.gen.camera_offset.1);
        self.camera_y += self.trig.gen.applied_camera_offset_y;
        self.trig.gen.applied_gameplay_offset_y = crate::triggers::or0(self.trig.gen.gameplay_offset.1);
        self.camera_y += self.trig.gen.applied_gameplay_offset_y;
        self.apply_static_camera(q);
        self.apply_camera_edges();
    }

    fn camera_anchor(&self, gid: i64) -> Option<(f64, f64)> {
        if !(gid > 0) { return None; }
        if let Some(&(x, y)) = self.trig.l.camera_guides.get(&gid) { return Some((x, y)); }
        let (cx, cy) = self.group_center_pub(gid);
        if cx == 0.0 && cy == 0.0 { return None; }
        Some((cx, 460.0 - cy))
    }

    pub fn apply_camera_edges(&mut self) {
        let edges = match &self.trig.gen.camera_edges { Some(e) => e.clone(), None => return };
        let zoom = if self.trig.gen.zoom == 0.0 || self.trig.gen.zoom.is_nan() { 1.0 } else { self.trig.gen.zoom };
        let half_w = self.screen_width / 2.0 / zoom;
        let half_h = 640.0 / 2.0 / zoom;
        let pos = |k: i64| edges.get(&k).and_then(|&g| self.camera_anchor(g));
        let (left, right, top, bottom) = (pos(1), pos(2), pos(3), pos(4));
        if let Some(r) = right { self.camera_x = crate::triggers::js_min(self.camera_x, r.0 - self.screen_width / 2.0 - half_w); }
        if let Some(l) = left { self.camera_x = crate::triggers::js_max(self.camera_x, l.0 - self.screen_width / 2.0 + half_w); }
        if let Some(t) = top { self.camera_y = crate::triggers::js_min(self.camera_y, t.1 + 180.0 - 320.0 - half_h); }
        if let Some(b) = bottom { self.camera_y = crate::triggers::js_max(self.camera_y, b.1 + 180.0 - 320.0 + half_h); }
    }

    pub fn apply_static_camera(&mut self, delta: f64) {
        let active = matches!(&self.trig.gen.static_camera, Some(c) if c.x.is_some() || c.y.is_some());
        if !active { self.follow_camera_y = None; return; }
        self.follow_camera_y = Some(self.camera_y);
        let seconds = delta / 60.0;
        for is_x in [true, false] {
            let st = { let c = self.trig.gen.static_camera.as_ref().unwrap(); if is_x { c.x.clone() } else { c.y.clone() } };
            let mut st = match st { Some(s) => s, None => continue };
            let follow = if is_x { self.camera_x } else { self.camera_y };
            if !st.last.map_or(false, |v| v.is_finite()) { st.last = Some(follow); }
            if !st.exiting {
                let anchor = self.camera_anchor(st.group);
                let (gx, gy) = match anchor {
                    Some(a) => a,
                    None => {
                        let c = self.trig.gen.static_camera.as_mut().unwrap();
                        if is_x { c.x = None; } else { c.y = None; }
                        continue;
                    }
                };
                st.target = if is_x { gx - self.screen_width / 2.0 } else { gy - 320.0 + 180.0 };
                if !st.from.map_or(false, |v| v.is_finite()) { st.from = Some(follow); }
            }
            st.elapsed += seconds;
            let (dur, et, er) = { let t = &self.trig.l.generics[st.trig]; (t.duration, t.easing_type, t.easing_rate) };
            let s = if dur > 0.0 && !st.instant { crate::triggers::easing(et as f64, er, crate::triggers::js_min(st.elapsed / dur, 1.0)) } else { 1.0 };
            let last = st.last.unwrap();
            let value;
            let mut clear = false;
            if st.exiting {
                value = last + (follow - last) * s;
                if s >= 1.0 { clear = true; }
            } else {
                let from = st.from.unwrap();
                value = from + (st.target - from) * s;
            }
            st.last = Some(value);
            let c = self.trig.gen.static_camera.as_mut().unwrap();
            let slot = if is_x { &mut c.x } else { &mut c.y };
            *slot = if clear { None } else { Some(st) };
            if is_x { self.camera_x = value; } else { self.camera_y = value; }
        }
    }

    /// Sets up the scene state at the start of a level (`_resetGameplayState`).
    pub fn reset_gameplay_state(&mut self) {
        self.clear_hitbox_trails();
        self.trig.gen.sideways_world_x = 0.0;
        self.e.in_sideways_step = false;
        self.camera_x = -self.g.center_x;
        self.camera_y = 0.0;
        self.e.player_world_x = 0.0;
        self.delta_buffer = 0.0;
        self.death_timer = 0.0;
        self.death_sound_played = false;
        self.new_best_shown = false;
        self.had_new_best = false;
        self.level_won = false;
        self.end_camera_override = false;
        self.end_cam = None;
        self.space_was_down = false;
        self.physics_frame = 0.0;
    }

    fn end_cam_progress(&self) -> f64 {
        let t = (self.end_cam.map_or(0.0, |c| c.elapsed_ms) / 1200.0).min(1.0);
        let power = 1.8;
        if t < 0.5 { m::pow(t * 2.0, power) / 2.0 } else { 1.0 - m::pow((1.0 - t) * 2.0, power) / 2.0 }
    }

    /// One frame (`delta_ms` real time). Returns true when the level was restarted this frame.
    /// One frame of the scene: simulation and triggers, then the sprite world (`render_step`).
    pub fn update(&mut self, delta_ms: f64, jump_held: bool) -> bool {
        if self.fx.menu_mode { self.menu_step(delta_ms); return false; }
        if self.fx.editor_mode { return false; }
        if self.fx.slide.is_some() { self.slide_step(delta_ms); return false; }
        let restarted = self.update_inner(delta_ms, jump_held);
        if restarted { self.render_restart(); }
        self.render_step(delta_ms);
        restarted
    }

    fn update_inner(&mut self, delta_ms: f64, jump_held: bool) -> bool {
        // `level.updateEndPortalY`
        let fly_like = self.p.is_flying || self.p.is_wave || self.p.is_ufo || self.p.is_swing || self.p.is_spider;
        let y = 140.0 + self.camera_y;
        self.end_portal_game_y = if fly_like { y } else { 240f64.max(y) };
        if !self.level_won && !self.p.is_dead && self.end_x_pos > 0.0 && self.e.player_world_x >= self.end_x_pos - 600.0 {
            self.level_won = true;
        }
        if let Some(c) = &mut self.end_cam { c.elapsed_ms += delta_ms; }
        if self.level_won {
            self.delta_buffer = 0.0;
            if let Some(c) = self.end_cam {
                let p = self.end_cam_progress();
                self.camera_x = c.from_x + (c.to_x - c.from_x) * p;
                self.camera_y = c.from_y + (c.to_y - c.from_y) * p;
            }
            return false;
        }
        if self.p.is_dead {
            if !self.death_sound_played {
                self.trigger_on_death();
                self.death_sound_played = true;
                self.total_deaths += 1.0;
            }
            if !self.new_best_shown {
                self.new_best_shown = true;
                let end = if self.end_x_pos == 0.0 || self.end_x_pos.is_nan() { 6000.0 } else { self.end_x_pos };
                self.last_percent = 99f64.min(0f64.max((self.e.player_world_x / end * 100.0).floor()));
                if self.last_percent > self.best_percent { self.best_percent = self.last_percent; self.had_new_best = true; }
            }
            self.apply_shake(0.0);
            self.death_timer += delta_ms;
            let limit = (if self.had_new_best { 1400.0 } else { 1000.0 }) * self.respawn_delay;
            if self.death_timer > limit {
                let sp = self.start_pos_index;
                self.restart_level(sp);
                return true;
            }
            return false;
        }
        self.animate_sprites(delta_ms);
        let quantized_before = self.delta_buffer;
        let _ = quantized_before;
        let q = self.step_frame_with_delta(delta_ms, jump_held);
        self.coin_pickups();
        if !self.end_camera_override {
            let offset = self.e.player_world_x - self.g.center_x + crate::triggers::or0(self.trig.gen.camera_offset.0) + crate::triggers::or0(self.trig.gen.gameplay_offset.0);
            if self.end_x_pos > 0.0 {
                let max_cam = self.end_x_pos - self.screen_width;
                if offset >= max_cam - 200.0 {
                    self.end_camera_override = true;
                    self.camera_x = offset;
                    let end_y = -140.0 + self.end_portal_game_y;
                    self.end_cam = Some(EndCam { from_x: self.camera_x, to_x: max_cam, from_y: self.camera_y, to_y: end_y, elapsed_ms: 0.0 });
                } else { self.camera_x = offset; }
            } else { self.camera_x = offset; }
        }
        if self.end_camera_override {
            if let Some(c) = self.end_cam {
                let p = self.end_cam_progress();
                self.camera_x = c.from_x + (c.to_x - c.from_x) * p;
                self.camera_y = c.from_y + (c.to_y - c.from_y) * p;
            }
        }
        if !self.end_camera_override { self.camera_y_block(q); }
        self.update_visibility(self.camera_x);
        if self.p.is_flying || self.p.is_swing { self.sim_for(false, |s| s.update_ship_rotation(q)); }
        if self.e.is_dual && (self.p2.is_flying || self.p2.is_swing) && !self.p2.is_dead { self.sim_for(true, |s| s.update_ship_rotation(q)); }
        if self.mirror.enabled { self.step_mirror(delta_ms / 1000.0); }
        false
    }
}

/// Practice mode: one saved checkpoint (`PracticeMode.saveCheckpoint`), the parts that matter for the simulation.
#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub x: f64, pub y: f64, pub y_velocity: f64, pub gravity_flipped: bool, pub is_mini: bool,
    pub is_ball: bool, pub is_ufo: bool, pub is_wave: bool, pub is_spider: bool, pub is_robot: bool, pub is_swing: bool, pub is_flying: bool,
    pub is_jumping: bool, pub on_ground: bool, pub can_jump: bool, pub was_boosted: bool, pub mirrored: bool,
    pub is_dashing: bool, pub dash_y_velocity: f64, pub ball_should_rotate: bool, pub ball_rotate_opposite: bool, pub ball_normal_rotate: f64, pub ball_hit_pad: bool,
    pub robot_hold: bool, pub robot_hold_timer: f64, pub rotate_action_active: bool,
    pub camera_x: f64, pub camera_y: f64,
    pub fly_floor_y: Option<f64>, pub fly_ceiling_y: Option<f64>, pub fly_ground_active: bool, pub fly_visual_only: bool, pub fly_camera_target: Option<f64>,
    pub speed: f64, pub physics_frame: f64,
    pub ground: Option<crate::ground::Ground>,
    pub dual: bool, pub dual_mode: &'static str, pub dual_mini: bool, pub dual_y: f64, pub dual_y_velocity: f64, pub dual_gravity_flipped: bool,
    pub dual_on_ground: bool, pub dual_on_ceiling: bool, pub dual_can_jump: bool, pub dual_is_jumping: bool,
    pub swing_gd_accel: bool, pub swing_on_ceiling: bool, pub portal_free_mode: bool,
    pub time_warp: (f64, usize, Vec<i64>), pub dual_bounds_center_y: Option<f64>,
    pub dual_swing_gd_accel: bool, pub dual_swing_was_boosted: bool,
    pub gameplay_sideways: bool, pub direction: f64,
}

impl Game {
    /// `PracticeMode.saveCheckpoint`; false when not in practice mode or dead.
    pub fn save_checkpoint(&mut self) -> bool {
        if !self.practice_mode || self.p.is_dead { return false; }
        let p = &self.p;
        let p2 = &self.p2;
        let dual_mode = if p2.is_swing { "swing" } else if p2.is_flying { "ship" } else if p2.is_ball { "ball" } else if p2.is_ufo { "ufo" } else if p2.is_wave { "wave" } else if p2.is_robot { "robot" } else if p2.is_spider { "spider" } else { "cube" };
        self.checkpoints.push(Checkpoint {
            x: self.e.player_world_x, y: p.y, y_velocity: p.y_velocity, gravity_flipped: p.gravity_flipped, is_mini: p.is_mini,
            is_ball: p.is_ball, is_ufo: p.is_ufo, is_wave: p.is_wave, is_spider: p.is_spider, is_robot: p.is_robot, is_swing: p.is_swing, is_flying: p.is_flying,
            is_jumping: p.is_jumping, on_ground: p.on_ground, can_jump: p.can_jump, was_boosted: p.was_boosted, mirrored: p.mirrored,
            is_dashing: p.is_dashing, dash_y_velocity: p.dash_y_velocity, ball_should_rotate: p.ball_should_rotate, ball_rotate_opposite: p.ball_rotate_opposite,
            ball_normal_rotate: p.ball_normal_rotate, ball_hit_pad: p.ball_hit_pad, robot_hold: p.robot_hold, robot_hold_timer: p.robot_hold_timer,
            rotate_action_active: p.rotate_action_active, camera_x: self.camera_x, camera_y: self.camera_y,
            fly_floor_y: self.e.fly_floor_y, fly_ceiling_y: self.e.fly_ceiling_y, fly_ground_active: self.e.fly_ground_active, fly_visual_only: self.e.fly_visual_only,
            fly_camera_target: self.e.fly_camera_target, speed: self.g.player_speed, physics_frame: self.physics_frame, ground: self.fx.ground.clone(),
            dual: self.e.is_dual, dual_mode, dual_mini: p2.is_mini, dual_y: p2.y, dual_y_velocity: p2.y_velocity, dual_gravity_flipped: p2.gravity_flipped,
            dual_on_ground: p2.on_ground, dual_on_ceiling: p2.on_ceiling, dual_can_jump: p2.can_jump, dual_is_jumping: p2.is_jumping,
            swing_gd_accel: p.gd_accel, swing_on_ceiling: p.on_ceiling, portal_free_mode: self.e.portal_free_mode,
            time_warp: self.save_time_warp(), dual_bounds_center_y: self.e.dual_bounds_center_y,
            dual_swing_gd_accel: p2.gd_accel, dual_swing_was_boosted: p2.was_boosted, gameplay_sideways: self.trig.gen.gameplay_sideways, direction: if self.trig.gen.direction == -1.0 { -1.0 } else { 1.0 }
        });
        true
    }

    pub fn delete_last_checkpoint(&mut self) -> bool { self.checkpoints.pop().is_some() }

    /// `_respawnFromCheckpoint`; falls back to a normal restart without checkpoints.
    pub fn respawn_from_checkpoint(&mut self) {
        self.clear_hitbox_trails();
        let Some(cp) = self.checkpoints.last().cloned() else { self.restart_level(self.start_pos_index); return; };
        // (`_restartLevel` already checked for checkpoints; reaching here means one exists)
        self.death_timer = 0.0; self.death_sound_played = false; self.new_best_shown = false;
        self.p.is_dead = false;
        self.e.player_world_x = cp.x;
        self.camera_x = cp.camera_x; self.camera_y = cp.camera_y;
        {
            let p = &mut self.p;
            p.y = cp.y; p.y_velocity = cp.y_velocity; p.gravity_flipped = cp.gravity_flipped; p.is_mini = cp.is_mini;
            p.is_swing = cp.is_swing; p.is_flying = cp.is_flying; p.is_jumping = cp.is_jumping; p.on_ground = cp.on_ground; p.can_jump = cp.can_jump;
            p.was_boosted = cp.was_boosted; p.mirrored = cp.mirrored; self.mirror.animating = false; p.is_dashing = cp.is_dashing; p.dash_y_velocity = cp.dash_y_velocity;
            p.ball_should_rotate = cp.ball_should_rotate; p.ball_rotate_opposite = cp.ball_rotate_opposite;
            p.ball_normal_rotate = if cp.ball_normal_rotate == 0.0 || cp.ball_normal_rotate.is_nan() { 1.0 } else { cp.ball_normal_rotate };
            p.ball_hit_pad = cp.ball_hit_pad; p.robot_hold = cp.robot_hold; p.robot_hold_timer = cp.robot_hold_timer;
        }
        crate::pdual::reset_object(&mut self.p, self.e.player_world_x);
        self.p.ignore_teleport_until_clear = true;
        self.p.is_flying = false; self.p.is_ball = false; self.p.is_wave = false; self.p.is_ufo = false; self.p.is_spider = false; self.p.is_robot = false; self.p.is_swing = false;
        {
            let mut sim = Sim::new(&mut self.p, &mut self.g, &mut self.e);
            if cp.is_swing { sim.enter_swing_mode(None, true, false); }
            else if cp.is_flying { sim.enter_ship_mode(None, true, false); }
            else if cp.is_ball { sim.enter_ball_mode(None, false); }
            else if cp.is_ufo { sim.enter_ufo_mode(None, true, false); }
            else if cp.is_wave { sim.enter_wave_mode(None, false); }
            else if cp.is_robot { sim.enter_robot_mode(false); }
            else if cp.is_spider { sim.enter_spider_mode(None, false, false); }
            self.log.extend(sim.log);
        }
        self.p.is_flying = cp.is_flying; self.p.is_ball = cp.is_ball; self.p.is_wave = cp.is_wave; self.p.is_ufo = cp.is_ufo; self.p.is_spider = cp.is_spider; self.p.is_robot = cp.is_robot;
        self.p.is_swing = cp.is_swing;
        if cp.is_swing {
            self.p.gd_accel = cp.swing_gd_accel; self.p.on_ceiling = cp.swing_on_ceiling; self.p.on_ground = cp.on_ground; self.p.can_jump = cp.can_jump;
            self.p.is_jumping = cp.is_jumping; self.p.was_boosted = cp.was_boosted;
        }
        self.p.robot_hold = cp.robot_hold; self.p.robot_hold_timer = cp.robot_hold_timer;
        self.p.rotation = 0.0; // the checkpoint's `rotation` is a PlayerState field that doesn't exist: `Number(undefined)` -> 0
        self.p.rotate_action_active = cp.rotate_action_active;
        self.p.ignore_portals = true; self.p2.ignore_portals = true;
        self.reset_objects();
        self.e.portal_free_mode = cp.portal_free_mode;
        self.e.fly_floor_y = cp.fly_floor_y;
        self.e.fly_ceiling_y = cp.fly_ceiling_y;
        self.e.fly_ground_active = cp.fly_ground_active;
        self.e.fly_visual_only = cp.fly_visual_only;
        self.e.fly_camera_target = cp.fly_camera_target;
        // the ground the level layer draws comes back as it was at the checkpoint (`_respawnFromCheckpoint` restores `_flyGroundActive`, `_groundAnim*`, ...)
        if let (Some(g), Some(snap)) = (self.fx.ground.as_mut(), cp.ground.as_ref()) { g.copy_state_from(snap); }
        self.e.floor_y = if self.e.fly_ground_active { if self.e.fly_visual_only { 0.0 } else { self.e.fly_floor_y.unwrap_or(f64::NAN) } } else { 0.0 };
        self.e.ceiling_y = self.e.fly_ceiling_y;
        self.trig.gen.gameplay_sideways = cp.gameplay_sideways;
        self.trig.gen.direction = cp.direction;
        self.trig.gen.direction_dirty = true;
        self.trig.gen.sideways_world_x = 0.0;
        self.e.in_sideways_step = false;
        self.sync_floor_ceiling();
        self.g.player_speed = cp.speed;
        if cp.dual {
            self.e.is_dual = false;
            self.e.dual_ball_overlap_resolved = false; self.e.dual_ball_spawn_lock = false;
            crate::pdual::reset_state(&mut self.p2);
            crate::pdual::reset_object(&mut self.p2, self.e.player_world_x);
            self.p2.ignore_teleport_until_clear = true;
            self.set_dual(true);
            self.p2.is_dead = false;
            if cp.dual_y.is_finite() { self.p2.y = cp.dual_y; }
            if cp.dual_y_velocity.is_finite() { self.p2.y_velocity = cp.dual_y_velocity; }
            self.p2.is_mini = cp.dual_mini;
            let mode = cp.dual_mode;
            self.sim_for(true, |s| s.set_player_gamemode(mode, true));
            self.p2.gravity_flipped = cp.dual_gravity_flipped;
            self.p2.on_ground = cp.dual_on_ground; self.p2.on_ceiling = cp.dual_on_ceiling; self.p2.can_jump = cp.dual_can_jump; self.p2.is_jumping = cp.dual_is_jumping;
            let input = self.p.clone();
            self.sim_for(false, |s| s.copy_input_to_secondary(&input));
            if self.p2.is_swing {
                self.p2.gd_accel = cp.dual_swing_gd_accel; self.p2.was_boosted = cp.dual_swing_was_boosted;
            }
            self.e.dual_bounds_center_y = Some(cp.dual_bounds_center_y.unwrap_or((cp.fly_floor_y.unwrap_or(0.0) + cp.fly_ceiling_y.unwrap_or(0.0)) / 2.0));
            self.sim_for(false, |s| s.ensure_dual_fly_bounds());
        } else {
            self.e.is_dual = false;
            crate::pdual::reset_state(&mut self.p2);
            crate::pdual::reset_object(&mut self.p2, self.e.player_world_x);
        }
        self.reset_color();
        self.reset_rotate(); self.reset_alpha(); self.reset_shake(); self.reset_player_vis(); self.reset_animation(); self.reset_enter_effect(); self.reset_pulse(); self.reset_spawn(); self.reset_move(); self.reset_stop(); self.reset_follow_y(); self.reset_follow();
        self.reset_channel_blending();
        self.reset_toggle();
        self.reset_on_death(); self.reset_touch(); self.reset_pickup(); self.reset_count(); self.reset_collision(); self.reset_instant_count();
        self.reset_time_warp();
        self.check_toggle(cp.x);
        self.restore_time_warp(Some(&cp.time_warp), cp.x);
        self.check_pickup(cp.x); self.check_count(cp.x); self.check_collision_triggers(cp.x); self.check_instant_count(cp.x);
        self.check_on_death(cp.x); self.check_touch_triggers(cp.x);
        self.delta_buffer = 0.0;
        self.physics_frame = cp.physics_frame;
        let pf = self.physics_frame;
        if self.mb.recording { self.macro_rollback_recording(pf); }
        if self.mb.playing { self.macro_rollback_playback(pf); }
    }
}

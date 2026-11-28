//! Player physics, ported from assets/scripts/core/player.js (PlayerObject) and utils/config.js.
//! Spider mode is not ported yet (it scans level objects); it panics if reached.

use crate::jsmath as m;

pub const D: f64 = 0.9;
pub const P: f64 = 1.916398;

pub mod speed_portal {
    pub const HALF: f64 = 9.30222544655;
    pub const ONE_TIMES: f64 = 11.540004;
    pub const TWO_TIMES: f64 = 14.3488938625;
    pub const THREE_TIMES: f64 = 17.3333393414;
    pub const FOUR_TIMES: f64 = 21.3333407279;
}

/// A value that fits in one f64 slot. Lets the test harness move whole structs across the wasm boundary.
pub trait Slot: Sized {
    fn get(&self) -> f64;
    fn set(v: f64) -> Self;
}
impl Slot for f64 { fn get(&self) -> f64 { *self } fn set(v: f64) -> Self { v } }
impl Slot for bool { fn get(&self) -> f64 { *self as u8 as f64 } fn set(v: f64) -> Self { v != 0.0 } }
impl Slot for Option<f64> {
    fn get(&self) -> f64 { self.unwrap_or(f64::NAN) }
    fn set(v: f64) -> Self { if v.is_nan() { None } else { Some(v) } }
}

impl Slot for Option<bool> {
    fn get(&self) -> f64 { match self { Some(b) => *b as u8 as f64, None => f64::NAN } }
    fn set(v: f64) -> Self { if v.is_nan() { None } else { Some(v != 0.0) } }
}

macro_rules! slots {
    ($(#[$m:meta])* pub struct $name:ident { $(pub $f:ident: $t:ty = $def:expr,)* }) => {
        $(#[$m])* #[derive(Debug, Clone)]
        pub struct $name { $(pub $f: $t,)* }
        impl Default for $name { fn default() -> Self { $name { $($f: $def,)* } } }
        impl $name {
            pub fn names(prefix: &str, out: &mut Vec<String>) { $(out.push(format!("{}{}:{}", prefix, stringify!($f), stringify!($t).replace(" ", "")));)* }
            pub fn pack(&self, out: &mut Vec<f64>) { $(out.push(Slot::get(&self.$f));)* }
            pub fn unpack(&mut self, it: &mut dyn Iterator<Item = f64>) { $(self.$f = <$t as Slot>::set(it.next().unwrap());)* }
        }
    };
}

slots! {
    /// Window flags and globals the original reads while stepping.
    pub struct Globals {
        pub player_speed: f64 = speed_portal::ONE_TIMES,
        pub slope_jump_boost: bool = true,
        pub slope_ramp_time: f64 = 0.1,
        pub slope_ramp_floor: f64 = 0.4,
        pub slope_launch_num: f64 = 0.8,
        pub slope_launch_cap: f64 = 1.1,
        pub slope_launch_gain: f64 = 1.4,
        pub center_x: f64 = 0.0,
        pub no_clip: bool = false,
        pub no_clip_accuracy: bool = false,
        pub slope_boost_skip_snap: bool = true,
        pub slope_exit_cap: f64 = f64::INFINITY,
        pub slope_swing_cap: f64 = 13.0,
        // settingsMap.kA34 == "1"
        pub robot_pad_touch: bool = false,
        pub gd_cube_slopes: bool = true,
        pub slope_reach_guard: bool = true,
        pub slope_flat_cap: bool = true,
        pub slope_back_wall: bool = true,
        pub slope_tangent_d: f64 = 1.0,
    }
}

slots! {
    /// What the original asks the scene or level about (answered by the caller here).
    pub struct Env {
        pub is_dual: bool = false,
        pub macro_playing: bool = false,
        pub dual_ball_spawn_lock: bool = false,
        pub dual_ball_overlap_resolved: bool = false,
        pub physics_tick: f64 = 0.0,
        // A ring (orb) overlaps the player (the scan in `_isTouchingAvailableOrbForInput`).
        pub orb_touch: bool = false,
        pub floor_y: f64 = 0.0,
        // `getCeilingY()`: None = no ceiling
        pub ceiling_y: Option<f64> = None,
        pub player_world_x: f64 = 0.0,
        // answer of `_gameLayer._breakblock(id)` for the object being hit
        pub break_block: bool = false,
        pub second_player: bool = false,
        pub practice_mode: bool = false,
        pub try_collect_secret: bool = false,
        pub try_collect_user: bool = false,
        pub mirror_toggles: bool = false,
        // level layer fly bounds (`setFlyMode`); when `manage_layer` is set, floor_y/ceiling_y follow them
        pub manage_layer: bool = false,
        pub fly_ground_active: bool = false,
        pub fly_visual_only: bool = false,
        pub fly_floor_y: Option<f64> = None,
        pub fly_ceiling_y: Option<f64> = None,
        pub fly_camera_target: Option<f64> = None,
        pub portal_free_mode: bool = false,
        pub dual_bounds_center_y: Option<f64> = None,
        pub in_sideways_step: bool = false,
        pub stop_jump_latch: bool = false,
        pub has_level_events: bool = false,
    }
}

slots! {
    /// The parts of `PlayerState` (`this.p`) and `PlayerObject` the jump updates touch.
    pub struct Player {
        pub y_velocity: f64 = 0.0,
        pub pending_velocity: Option<f64> = None,
        pub is_dashing: bool = false,
        pub dash_y_velocity: f64 = 0.0,
        pub up_key_down: bool = false,
        pub on_ground: bool = false,
        pub is_jumping: bool = false,
        pub can_jump: bool = false,
        pub up_key_pressed: bool = false,
        pub queued_hold: bool = false,
        pub is_mini: bool = false,
        pub gravity_flipped: bool = false,
        pub is_flying: bool = false,
        pub is_wave: bool = false,
        pub is_ball: bool = false,
        pub is_ufo: bool = false,
        pub is_spider: bool = false,
        pub is_robot: bool = false,
        pub is_swing: bool = false,
        pub on_ceiling: bool = false,
        pub ball_should_rotate: bool = false,
        pub gd_accel: bool = false,
        pub robot_accel: f64 = 0.0,
        pub robot_touched_pad: bool = false,
        pub robot_hold: bool = false,
        pub slope_bounce_active: bool = false,
        pub was_boosted: bool = false,
        pub orb_activation_consumed_for_press: bool = false,
        // PlayerObject fields
        pub slope_exit_vel: Option<f64> = None,
        pub slope_contact_frames: f64 = 0.0,
        pub prev_slope_angle: f64 = 0.0,
        pub rotate_action_active: bool = false,
        pub rotate_action_time: f64 = 0.0,
        pub rotate_action_duration: f64 = 0.0,
        pub rotate_action_start: f64 = 0.0,
        pub rotate_action_total: f64 = 0.0,
        pub rotation: f64 = 0.0,
        pub slope_launch_spin: bool = false,
        pub on_slope_angle: Option<f64> = None,
        pub gd_was_on_slope: bool = false,
        pub skip_slope_exit: bool = false,
        pub gd_flip_tick: Option<f64> = None,
        // --- used by check_collisions ---
        pub y: f64 = 0.0,
        pub last_y: f64 = 0.0,
        pub last_ground_y: f64 = f64::NAN,
        pub collide_top: f64 = 0.0,
        pub collide_bottom: f64 = 0.0,
        pub touching_ring: bool = false,
        pub was_up_key_down: bool = false,
        pub died_this_frame: bool = false,
        pub died_last_frame: bool = false,
        pub is_dead: bool = false,
        pub ignore_portals: bool = false,
        pub mirrored: bool = false,
        pub ball_rotate_opposite: bool = false,
        pub ball_normal_rotate: f64 = 1.0,
        pub ball_hit_pad: bool = false,
        pub ball_rotation2: f64 = 0.0,
        pub active_slope_obj: Option<f64> = None,
        pub active_slope_angle: Option<f64> = None,
        pub gd_is_on_slope: bool = false,
        pub gd_mode_sig: Option<f64> = None,
        pub gd_mode_tick: Option<f64> = None,
        pub wave_slide_active: bool = false,
        pub last_collision_world_x: Option<f64> = None,
        pub last_collision_world_y: Option<f64> = None,
        pub prev_slope_obj: Option<f64> = None,
        pub slope_riding: bool = false,
        pub slope_exit_grace: f64 = 0.0,
        pub slope_entry_x: Option<f64> = None,
        pub last_land_object: Option<f64> = None,
        pub last_land_x: f64 = 0.0,
        pub last_land_y: f64 = 0.0,
        pub last_land_ty: f64 = 27.0,
        pub last_x_offset: f64 = 0.0,
        pub ignore_teleport_until_clear: bool = false,
        pub nc_total_frames: f64 = 0.0,
        pub nc_death_frames: f64 = 0.0,
        pub nc_deaths: f64 = 0.0,
        pub nc_accuracy: f64 = 100.0,
        pub last_ground_pos_y: f64 = 0.0,
        pub last_blue_pad_tick: Option<f64> = None,
        pub robot_hold_timer: f64 = 0.0,
        pub spider_teleport_pending: bool = false,
        pub spider_teleport_anim_timer: f64 = 0.0,
        pub spider_flash_timer: f64 = 0.0,
        pub spider_flash_duration: f64 = 0.5,
        pub gd_last_slope_obj: Option<f64> = None,
        pub gd_last_slope_top: Option<bool> = None,
        pub gd_last_slope_y_pos: Option<f64> = None,
        pub gd_last_slope_steepness: f64 = f64::NAN,
        pub gd_last_slope_up: Option<bool> = None,
        pub gd_slope_rotation: f64 = f64::NAN,
        pub spider_teleport_noclip_death_pending: bool = false,
        pub gravity_scale: f64 = 1.0,
        pub force_ticks: Option<f64> = None,
        pub force_acc_x: f64 = 0.0,
        pub force_acc_y: f64 = 0.0,
        pub safe_head_ticks: Option<f64> = None,
        pub flip_head_ticks: Option<f64> = None,
        pub j_block_ticks: Option<f64> = None,
        pub gd_fall_start_y: Option<f64> = None,
        pub gd_land_flag: bool = true,
        pub gd_prev_fall_vel: Option<f64> = None,
        pub gd_was_falling: bool = false,
        pub gd_prev_held: Option<bool> = None,
        pub gd_prev_ceiling: bool = false,
        pub press_count: Option<f64> = None,
        pub ship_head_uid: Option<f64> = None,
        pub ship_head_flipped: bool = false,
        pub prev_ship_head_uid: Option<f64> = None,
        pub prev_ship_head_flipped: bool = false,
    }
}

fn grav_consts(sp: f64) -> (f64, f64) {
    use speed_portal::*;
    if sp == HALF { return (1.880398, 21.240064); }
    if sp == TWO_TIMES { return (1.914398, 22.840064); }
    if sp == THREE_TIMES || sp == FOUR_TIMES { return (1.922398, 22.460064); }
    (P, 22.360064)
}

pub fn grav_consts_pub(sp: f64) -> (f64, f64) { grav_consts(sp) }

/// JavaScript `Math.round`: halves go toward +infinity.
pub fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// `_gdTrunc`: truncates to three decimals on the half value.
pub fn gd_trunc(v: f64) -> f64 {
    let h = v / 2.0;
    let i = h.trunc();
    2.0 * (i + ((h - i) * 1000.0).trunc() / 1000.0)
}

/// `slerp2D` (angles are doubled half-angles, like the original).
pub fn slerp2d(start: f64, end: f64, t: f64) -> f64 {
    let (half_start, half_end) = (start * 0.5, end * 0.5);
    let (cos_start, sin_start) = (m::cos(half_start), m::sin(half_start));
    let (mut cos_end, mut sin_end) = (m::cos(half_end), m::sin(half_end));
    let mut dot = cos_start * cos_end + sin_start * sin_end;
    if dot < 0.0 {
        dot = -dot;
        sin_end = -sin_end;
        cos_end = -cos_end;
    }
    let (w_start, w_end);
    if 1.0 - dot > 0.0001 {
        let theta = m::acos(dot);
        let sin_theta = m::sin(theta);
        w_start = m::sin(theta * (1.0 - t)) / sin_theta;
        w_end = m::sin(theta * t) / sin_theta;
    } else {
        w_start = 1.0 - t;
        w_end = t;
    }
    let out = m::atan2(sin_start * w_start + sin_end * w_end, cos_start * w_start + cos_end * w_end);
    out + out
}

pub struct Sim<'a> {
    pub p: &'a mut Player,
    pub g: &'a mut Globals,
    pub e: &'a mut Env,
    /// External calls the original makes (level layer, scene, effects), in order, for the tests to compare.
    pub log: Vec<String>,
    /// Visual-only events (landing, gravity flip, orb/pad hits, spider teleports) for the renderer; never read by the simulation.
    pub vlog: Vec<String>,
    /// The other player (second player while `p` is the primary, and the other way round); only set while dual mode can matter.
    pub other: Option<&'a mut Player>,
    /// Colliders `_getInitialDualPortalMode` looks at (the previous position's neighbourhood when that differs from the current one).
    pub cands: Vec<crate::pcol::Col>,
    /// `getNearbySectionObjects` returns one shared buffer, and `_getInitialDualPortalMode` refills it while `checkCollisions` is still
    /// looping over it: the loop carries on over the other list. These carry that: the global indices of the colliders being looped over,
    /// the replacement list (applied at the start of the next iteration), copies of its members, and the entries the loop left behind.
    pub near_idx: Vec<usize>,
    pub replace_idx: Option<Vec<usize>>,
    pub replace_pending: Option<Vec<usize>>,
    pub replace_cols: std::collections::BTreeMap<usize, crate::pcol::Col>,
    pub stash: Vec<(usize, crate::pcol::Col)>,
    /// the level's coin bookkeeping (`tryCollectSecretCoin` ...); None in the harnesses, which answer through `Env`
    pub coins: Option<&'a mut crate::coins::CoinState>,
    pub force_ids: Vec<f64>,
}

impl<'a> Sim<'a> {
    pub fn new(p: &'a mut Player, g: &'a mut Globals, e: &'a mut Env) -> Sim<'a> { Sim { p, g, e, log: Vec::new(), vlog: Vec::new(), other: None, cands: Vec::new(), near_idx: Vec::new(), replace_idx: None, replace_pending: None, replace_cols: Default::default(), stash: Vec::new(), coins: None, force_ids: Vec::new() } }

    pub fn flip_mod(&self) -> f64 { if self.p.gravity_flipped { -1.0 } else { 1.0 } }

    pub fn gravity_scale(&self) -> f64 { if self.p.gravity_scale > 0.0 { self.p.gravity_scale } else { 1.0 } }

    pub fn speed_consts(&self) -> (f64, f64) {
        let (grav, y_start) = grav_consts(self.g.player_speed);
        (grav * self.gravity_scale(), y_start)
    }

    pub fn player_is_falling(&self) -> bool {
        let thr = 2.0 * self.speed_consts().0;
        if self.p.gravity_flipped { self.p.y_velocity > -thr } else { self.p.y_velocity < thr }
    }

    fn is_falling_past_threshold(&self) -> bool {
        if self.p.gravity_flipped { self.p.y_velocity > 0.5 } else { self.p.y_velocity < -0.5 }
    }

    pub fn run_rotate_action(&mut self, direction: f64) {
        if self.p.is_swing { return; }
        let p = &mut *self.p;
        p.rotate_action_active = true;
        p.rotate_action_time = 0.0;
        let scale = if p.gravity_scale > 0.0 { p.gravity_scale } else { 1.0 };
        p.rotate_action_duration = (if p.is_mini { 1.0 / 3.0 } else { 0.39 / D }) / scale;
        p.rotate_action_start = p.rotation;
        p.rotate_action_total = std::f64::consts::PI * (if p.gravity_flipped { -1.0 } else { 1.0 }) * direction;
    }

    pub fn flip_gravity(&mut self, flipped: bool, factor: f64) {
        if self.p.gravity_flipped == flipped { return; }
        self.p.gravity_flipped = flipped;
        self.p.gd_flip_tick = Some(self.e.physics_tick);
        self.p.y_velocity *= factor;
        self.p.on_ground = false;
        self.p.can_jump = false;
        if self.p.is_ball { self.p.ball_should_rotate = true; }
    }

    fn slope_launch_multiplier(&self, angle: f64) -> f64 {
        let g = &*self.g;
        let acute = m::atan(m::tan(if angle == 0.0 || angle.is_nan() { 0.0 } else { angle }).abs());
        if !(acute > 1e-6) { return g.slope_launch_cap * g.slope_launch_gain; }
        (g.slope_launch_num / acute).min(g.slope_launch_cap) * g.slope_launch_gain
    }

    pub(crate) fn slope_exit_scale(&self) -> f64 {
        let (g, p) = (&*self.g, &*self.p);
        let held = p.slope_contact_frames / 240.0;
        let ramp = if held < g.slope_ramp_time { (held / g.slope_ramp_time).max(g.slope_ramp_floor) } else { 1.0 };
        let mode_scale = if p.is_flying || p.is_ufo || p.is_wave || p.is_swing || p.is_ball { 0.75 } else { 1.0 };
        self.slope_launch_multiplier(p.prev_slope_angle) * mode_scale * ramp
    }

    pub(crate) fn apply_slope_jump_boost(&mut self) {
        if !self.g.slope_jump_boost { return; }
        let exit = match self.p.slope_exit_vel { Some(v) => v, None => return };
        let base = self.p.y_velocity;
        if base == 0.0 || base.is_nan() { return; }
        let dir = if base >= 0.0 { 1.0 } else { -1.0 };
        let contact = exit * self.slope_exit_scale();
        if contact * dir <= 0.0 { return; }
        let cap = base * 1.4;
        let mut v = base + contact * 0.25;
        if dir > 0.0 { if v > cap { v = cap; } } else if v < cap { v = cap; }
        self.p.y_velocity = v;
    }

    fn convert_to_closest_rotation(&self) -> f64 {
        let q = std::f64::consts::PI / 2.0;
        js_round(self.p.rotation / q) * q
    }

    pub fn update_ground_rotation(&mut self, dt: f64) {
        if self.p.is_ball || self.p.is_wave { return; }
        let mut target = match self.p.on_slope_angle { Some(a) => a, None => self.convert_to_closest_rotation() };
        if let Some(slope) = self.p.on_slope_angle {
            if !self.p.is_robot && !self.p.is_spider && !self.p.is_flying && !self.p.is_swing {
                let slope_deg = slope * 180.0 / std::f64::consts::PI;
                let rot_deg = self.p.rotation * 180.0 / std::f64::consts::PI;
                let cur_deg = rot_deg.trunc() % 360.0;
                let (mut near, mut prev) = (slope_deg, slope_deg);
                if cur_deg > slope_deg {
                    loop { prev = near; near += 90.0; if !(near < cur_deg) { break; } }
                } else if cur_deg < slope_deg {
                    loop { prev = near; near -= 90.0; if !(near > cur_deg) { break; } }
                }
                let pick = if (cur_deg - near).abs() < (cur_deg - prev).abs() { near } else { prev };
                target = (rot_deg - (rot_deg % 360.0) + pick) * std::f64::consts::PI / 180.0;
            }
        }
        use speed_portal::*;
        let sp = self.g.player_speed;
        let gd_speed = if sp == HALF { 0.7 } else if sp == TWO_TIMES { 1.1 } else if sp == THREE_TIMES { 1.3 } else if sp == FOUR_TIMES { 1.6 } else { 0.9 };
        let frames = dt / D;
        let on_or_left = self.p.on_slope_angle.is_some() || self.p.gd_was_on_slope;
        let t = frames.min(gd_speed * (if on_or_left { 0.175 } else { 0.525 }) * frames);
        self.p.rotation = slerp2d(self.p.rotation, target, t);
    }

    /// player.js `updateShipRotation(delta)`: the ship turns towards the direction it moved in this frame (`delta` in 60 Hz frames)
    pub fn update_ship_rotation(&mut self, delta: f64) {
        let dy = -(self.p.y - self.p.last_y);
        let one = speed_portal::ONE_TIMES;
        let active = if self.g.player_speed.is_finite() { self.g.player_speed } else { one };
        let dx = delta * 10.3860036 * (active / one);
        if dx * dx + dy * dy >= delta * 0.6 {
            let target = m::atan2(dy, dx);
            let t = (delta * 1.0).min(0.15 * delta);
            self.p.rotation = slerp2d(self.p.rotation, target, t);
        }
    }

    pub(crate) fn orb_touching(&self, require_unconsumed: bool) -> bool {
        if require_unconsumed && self.p.orb_activation_consumed_for_press { return false; }
        self.e.orb_touch
    }

    pub(crate) fn should_prioritize_ground_orb_input(&self) -> bool {
        let p = &*self.p;
        if !(p.is_ball || p.is_spider) { return false; }
        if !p.up_key_pressed { return false; }
        if !(p.can_jump || p.on_ground || p.on_ceiling) { return false; }
        self.orb_touching(false)
    }

    fn should_prioritize_ufo_orb_input(&self) -> bool {
        self.p.is_ufo && self.p.up_key_pressed && self.orb_touching(true)
    }

    fn update_fly_jump(&mut self, dt: f64) {
        let f23 = if self.p.is_mini { 0.85 } else { 1.0 };
        let flip = self.flip_mod();
        let hold = self.p.up_key_down;
        let up = self.p.y_velocity * flip;
        if self.p.gd_accel && ((up >= 0.0 && up < 16.0 / f23) || (up <= 0.0 && up > -12.8 / f23)) {
            self.p.gd_accel = false;
        }
        let below = up < 2.0 * self.speed_consts().0;
        let (mult, k);
        if !hold {
            let mut m = if self.p.gd_accel { if up >= 0.0 { 0.8 } else { -1.0 } } else { 0.8 };
            if !below { m = 1.2; }
            mult = m;
            k = 0.4;
        } else {
            mult = if self.p.gd_accel { if up >= 0.0 { 0.8 } else { -1.0 } } else { -1.0 };
            k = if below { 0.5 } else { 0.4 };
        }
        let gs = self.gravity_scale();
        self.p.y_velocity = gd_trunc(self.p.y_velocity - flip * k * mult * P * gs * dt / f23);
        if hold { self.p.on_ground = false; }
        if !self.p.gd_accel {
            let up_now = (-12.8 / f23).max((16.0 / f23).min(self.p.y_velocity * flip));
            self.p.y_velocity = gd_trunc(up_now * flip);
        }
    }

    fn update_ball_jump(&mut self, dt: f64) {
        let ball_grav = P * 0.6 * self.gravity_scale();
        let prioritize_orb = self.should_prioritize_ground_orb_input();
        if !prioritize_orb && self.p.up_key_pressed && self.p.can_jump {
            let flip = self.flip_mod();
            self.p.up_key_pressed = false;
            self.p.queued_hold = false;
            self.p.y_velocity = gd_trunc(flip * self.speed_consts().1 * if self.p.is_mini { 0.8 } else { 1.0 });
            let flipped = !self.p.gravity_flipped;
            self.flip_gravity(flipped, 0.5);
            self.p.on_ground = false;
            self.p.can_jump = false;
            self.p.y_velocity *= 0.6;
            self.p.skip_slope_exit = true;
            return;
        }
        if self.player_is_falling() { self.p.can_jump = false; }
        self.p.y_velocity = gd_trunc(self.p.y_velocity - ball_grav * dt * self.flip_mod());
        self.p.y_velocity = if self.p.gravity_flipped { self.p.y_velocity.min(30.0) } else { self.p.y_velocity.max(-30.0) };
        if self.player_is_falling() {
            let past = if self.p.gravity_flipped { self.p.y_velocity > P * 2.0 } else { self.p.y_velocity < -(P * 2.0) };
            if past { self.p.on_ground = false; }
        }
    }

    fn update_wave_jump(&mut self, dt: f64) {
        let wave_vel = self.g.player_speed * if self.p.is_mini { 2.0 } else { 1.0 };
        let pushing_up = self.p.up_key_down;
        let mut vy = (if pushing_up { 1.0 } else { -1.0 }) * self.flip_mod() * wave_vel;
        if self.p.on_ground || self.p.on_ceiling {
            let away_ceiling = self.p.on_ceiling && !pushing_up;
            let away_floor = self.p.on_ground && pushing_up;
            if away_ceiling || away_floor {
                self.p.on_ground = false;
                self.p.on_ceiling = false;
            } else {
                vy = 0.0;
            }
        }
        self.p.y_velocity = vy;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        let angle = if self.p.is_mini { m::atan(2.0) } else { std::f64::consts::PI / 4.0 };
        let target = if vy == 0.0 { 0.0 } else if vy > 0.0 { -angle } else { angle };
        let turn_t = (0.55 * dt).min(1.0);
        self.p.rotation = slerp2d(self.p.rotation, target, turn_t);
    }

    fn update_robot_jump(&mut self, dt: f64) {
        if !self.p.rotate_action_active { self.update_ground_rotation(dt); }
        let (grav_c, y_start) = self.speed_consts();
        let flip = self.flip_mod();
        let grav = 0.9 * grav_c;
        if self.p.up_key_pressed && self.p.can_jump {
            self.p.up_key_pressed = false;
            self.p.is_jumping = true;
            self.p.gd_land_flag = false;
            self.p.on_ground = false;
            self.p.can_jump = false;
            self.p.queued_hold = false;
            self.p.robot_accel = 0.0;
            self.p.robot_touched_pad = false;
            self.p.y_velocity = gd_trunc(flip * y_start * 0.5 * if self.p.is_mini { 0.8 } else { 1.0 });
            self.apply_slope_jump_boost();
            self.p.robot_hold = true;
            return;
        }
        if self.p.is_jumping {
            let holding = self.p.up_key_down && !self.p.robot_touched_pad && self.p.robot_accel < 1.5;
            if holding {
                self.p.robot_accel += dt * 0.1;
                self.p.y_velocity += flip * grav * dt;
            } else {
                self.p.robot_hold = false;
            }
            self.p.y_velocity = gd_trunc(self.p.y_velocity - flip * grav * dt);
            if self.player_is_falling() {
                self.p.is_jumping = false;
                self.p.robot_hold = false;
            }
        } else {
            self.p.y_velocity = gd_trunc(self.p.y_velocity - flip * grav * dt);
            self.p.y_velocity = if self.p.gravity_flipped { self.p.y_velocity.min(30.0) } else { self.p.y_velocity.max(-30.0) };
        }
        if self.player_is_falling() { self.p.can_jump = false; }
    }

    fn update_swing_jump(&mut self, dt: f64) {
        if self.p.up_key_pressed && self.p.up_key_down {
            self.p.up_key_pressed = false;
            self.p.queued_hold = false;
            self.p.on_ground = false;
            let flipped = !self.p.gravity_flipped;
            self.flip_gravity(flipped, 0.8);
            self.p.y_velocity = gd_trunc(self.p.y_velocity);
        }
        let flip = self.flip_mod();
        let gs = self.gravity_scale();
        let k = if self.p.is_mini { 0.6 } else { 0.4 };
        self.p.y_velocity = gd_trunc(self.p.y_velocity - flip * k * P * gs * dt);
        self.p.y_velocity = gd_trunc((-16.0f64).max(16.0f64.min(self.p.y_velocity)));
        self.p.can_jump = false;
        self.p.is_jumping = false;
    }

    fn step_game_events(&mut self) {
        let v = (if self.p.gravity_flipped { -1.0 } else { 1.0 }) * self.p.y_velocity / 2.0;
        self.p.gd_prev_fall_vel = Some(v);
        let falling = self.player_is_falling();
        if self.p.on_ground { self.p.gd_fall_start_y = Some(self.p.y); }
        else if falling && !self.p.gd_was_falling { self.p.gd_fall_start_y = Some(self.p.y); }
        self.p.gd_was_falling = falling;
        self.p.gd_prev_held = Some(self.p.up_key_down);
        self.p.gd_prev_ceiling = self.p.on_ceiling;
    }

    pub fn force_mode_factor(&self) -> f64 {
        let p = &*self.p;
        if p.is_flying { if p.is_mini { 0.5875 } else { 0.47 } }
        else if p.is_ufo { if p.is_mini { 0.725 } else { 0.58 } }
        else if p.is_swing { if p.is_mini { 0.6154 } else { 0.4 } }
        else if p.is_ball || p.is_spider { if p.is_mini { 0.725 } else { 0.6 } }
        else if p.is_robot { if p.is_mini { 0.725 } else { 0.9 } }
        else if p.is_mini { 0.725 } else { 1.0 }
    }

    fn update_ufo_jump(&mut self, dt: f64) {
        let f23 = if self.p.is_mini { 0.85 } else { 1.0 };
        let flip = self.flip_mod();
        let up_start = self.p.y_velocity * flip;
        if self.p.gd_accel && ((up_start >= 0.0 && up_start < 16.0 / f23) || (up_start <= 0.0 && up_start > -12.8 / f23)) {
            self.p.gd_accel = false;
        }
        if self.p.up_key_pressed {
            let prioritize_orb = self.should_prioritize_ufo_orb_input();
            self.p.up_key_pressed = false;
            if prioritize_orb {
                self.p.queued_hold = false;
            } else {
                let ufo_jump = if self.p.is_mini { 13.6 } else { 14.0 };
                if self.p.y_velocity * flip < ufo_jump {
                    self.p.y_velocity = gd_trunc(ufo_jump * flip);
                    if let Some(exit) = self.p.slope_exit_vel {
                        let slope_vel = exit * self.slope_launch_multiplier(self.p.prev_slope_angle) * 0.75;
                        if slope_vel * flip > 0.0 {
                            let boosted = self.p.y_velocity + slope_vel * 0.5;
                            let cap = self.p.y_velocity * 1.4;
                            self.p.y_velocity = gd_trunc(if boosted.abs() > cap.abs() { cap } else { boosted });
                        }
                    }
                }
                self.p.on_ground = false;
                self.p.can_jump = false;
                self.p.is_jumping = true;
                self.p.gd_land_flag = false;
            }
        }
        let ufo_mult = if self.p.y_velocity * flip >= 2.0 * self.speed_consts().0 { 1.2 } else { 0.8 };
        let gs = self.gravity_scale();
        self.p.y_velocity = gd_trunc(self.p.y_velocity - flip * ufo_mult * P * gs * dt * 0.5 / f23);
        if !self.p.gd_accel {
            let up_now = (-12.8 / f23).max((16.0 / f23).min(self.p.y_velocity * flip));
            self.p.y_velocity = gd_trunc(up_now * flip);
        }
        if self.p.up_key_down { self.p.on_ground = false; }
        if self.p.is_jumping && self.player_is_falling() { self.p.is_jumping = false; }
    }

    /// `updateJump(dt)`.
    pub fn update_jump(&mut self, dt: f64, cols: &[crate::pcol::Col]) {
        self.step_game_events();
        if let Some(v) = self.p.pending_velocity.take() { self.p.y_velocity = v; }
        if let Some(t) = self.p.force_ticks.filter(|t| *t > 0.0) {
            self.p.force_ticks = Some(t - 1.0);
            let fy = self.p.force_acc_y;
            if fy != 0.0 {
                self.p.y_velocity += 2.0 * 0.9 * self.force_mode_factor() * dt * fy;
                if if self.p.gravity_flipped { fy < 0.0 } else { fy > 0.0 } { self.p.on_ground = false; }
            }
        }
        self.p.force_acc_x = 0.0;
        self.p.force_acc_y = 0.0;
        if self.p.is_dashing {
            if !self.p.up_key_down {
                self.p.is_dashing = false;
                self.p.dash_y_velocity = 0.0;
                self.p.y_velocity = 0.0;
            } else {
                self.p.y_velocity = self.p.dash_y_velocity;
                if self.is_dash_spinning() {
                    self.p.on_ground = false;
                    self.p.on_ceiling = false;
                }
                return;
            }
        }
        if self.p.is_flying { return self.update_fly_jump(dt); }
        if self.p.is_wave { return self.update_wave_jump(dt); }
        if self.p.is_ball { return self.update_ball_jump(dt); }
        if self.p.is_ufo { return self.update_ufo_jump(dt); }
        if self.p.is_spider { return self.update_spider_jump(dt, cols); }
        if self.p.is_robot { return self.update_robot_jump(dt); }
        if self.p.is_swing { return self.update_swing_jump(dt); }
        let (grav, y_start) = self.speed_consts();
        if self.p.up_key_down && self.p.can_jump {
            self.p.is_jumping = true;
            self.p.gd_land_flag = false;
            self.p.on_ground = false;
            self.p.can_jump = false;
            self.p.up_key_pressed = false;
            self.p.queued_hold = false;
            self.p.y_velocity = gd_trunc(self.flip_mod() * y_start * if self.p.is_mini { 0.8 } else { 1.0 });
            self.apply_slope_jump_boost();
            self.run_rotate_action(1.0);
        } else if self.p.is_jumping {
            self.p.y_velocity = gd_trunc(self.p.y_velocity - grav * dt * self.flip_mod());
            if self.player_is_falling() {
                self.p.is_jumping = false;
                self.p.on_ground = false;
            }
        } else {
            if self.player_is_falling() { self.p.can_jump = false; }
            self.p.y_velocity = gd_trunc(self.p.y_velocity - grav * dt * self.flip_mod());
            self.p.y_velocity = if self.p.gravity_flipped { self.p.y_velocity.min(30.0) } else { self.p.y_velocity.max(-30.0) };
            if self.is_falling_past_threshold() && !self.p.rotate_action_active && !self.p.slope_launch_spin {
                self.run_rotate_action(1.0);
            }
            if self.player_is_falling() {
                let past = if self.p.gravity_flipped { self.p.y_velocity > P * 2.0 } else { self.p.y_velocity < -(P * 2.0) };
                if past { self.p.on_ground = false; }
            }
        }
    }
}

/// Field names in pack order: Player, then `g.` Globals, then `e.` Env.
pub fn layout() -> Vec<String> {
    let mut v = Vec::new();
    Player::names("", &mut v);
    Globals::names("g.", &mut v);
    Env::names("e.", &mut v);
    v
}

//! Dual mode: the scene-level helpers of game-scene.js (`_enableDualMode`, `_disableDualMode`, `_setPlayerGamemode`,
//! the gravity / mirror sync between the two players, ball overlap, fly bounds) as `Sim` methods.
//! `Sim.p` is the player whose code is running; `Sim.other` is the other one (`e.second_player` says which is which).
use crate::pcol::Col;
use crate::physics::{Player, Sim};

const F: f64 = 600.0;
const A: f64 = 60.0;

/// `PlayerState.reset()`
pub fn reset_state(p: &mut Player) {
    p.y = 30.0; p.last_y = 30.0; p.last_ground_pos_y = 30.0; p.y_velocity = 0.0; p.on_ground = true; p.can_jump = true; p.is_jumping = false;
    p.gravity_flipped = false; p.is_flying = false; p.is_ball = false; p.is_wave = false; p.is_ufo = false; p.is_spider = false; p.is_robot = false;
    p.is_swing = false; p.is_mini = false; p.was_boosted = false; p.gd_accel = false; p.robot_accel = 0.0; p.robot_touched_pad = false;
    p.slope_bounce_active = false; p.pending_velocity = None; p.collide_top = 0.0; p.collide_bottom = 0.0; p.on_ceiling = false;
    p.up_key_down = false; p.up_key_pressed = false; p.queued_hold = false; p.orb_activation_consumed_for_press = false; p.is_dead = false;
    p.mirrored = false; p.is_dashing = false; p.dash_y_velocity = 0.0; p.ignore_portals = false; p.robot_hold = false; p.robot_hold_timer = 0.0;
    p.spider_teleport_noclip_death_pending = false; p.ball_should_rotate = false; p.ball_rotate_opposite = false; p.ball_normal_rotate = 0.86;
    p.ball_hit_pad = false; p.ball_rotation2 = 0.86;
    p.gravity_scale = 1.0;
    p.gd_fall_start_y = None; p.gd_land_flag = true; p.gd_prev_fall_vel = None; p.gd_was_falling = false; p.gd_prev_held = None; p.gd_prev_ceiling = false;
}

/// `PlayerObject.reset()`; `world_x` is the scene's `_playerWorldX`.
pub fn reset_object(p: &mut Player, world_x: f64) {
    p.last_land_object = None; p.last_land_x = 0.0; p.last_land_y = 0.0; p.last_land_ty = 27.0; p.last_x_offset = 0.0; p.gd_is_on_slope = false; p.gd_was_on_slope = false;
    p.gd_last_slope_obj = None; p.gd_last_slope_top = None; p.gd_last_slope_y_pos = None; p.gd_last_slope_steepness = 0.0; p.gd_last_slope_up = Some(false);
    p.gd_slope_rotation = 0.0; p.slope_launch_spin = false; p.prev_slope_obj = None; p.prev_slope_angle = f64::NAN; p.active_slope_obj = None;
    p.active_slope_angle = None; p.on_slope_angle = None; p.slope_entry_x = None;
    p.rotate_action_active = false; p.rotate_action_time = 0.0; p.rotation = 0.0;
    p.robot_hold = false; p.robot_hold_timer = 0.0;
    p.last_collision_world_x = if world_x.is_finite() { Some(world_x) } else { None };
    p.last_collision_world_y = if p.y.is_finite() { Some(p.y) } else { None };
    p.ignore_teleport_until_clear = false;
}

fn shared_signature(p: &Player) -> u8 { (p.gravity_flipped as u8) | ((p.mirrored as u8) << 1) }
pub fn dual_shared_signature(p: &Player) -> u8 { shared_signature(p) }

fn mode_id(p: &Player) -> &'static str {
    if p.is_swing { "swing" } else if p.is_flying { "ship" } else if p.is_ball { "ball" } else if p.is_ufo { "ufo" } else if p.is_wave { "wave" } else if p.is_robot { "robot" } else if p.is_spider { "spider" } else { "cube" }
}

fn portal_mode(ty: f64) -> Option<&'static str> {
    // portal_cube 13, portal_fly 12, portal_ball 14, portal_wave 15, portal_ufo 16, portal_robot 17, portal_spider 18
    match ty as i64 { 13 => Some("cube"), 12 => Some("ship"), 14 => Some("ball"), 15 => Some("wave"), 16 => Some("ufo"), 17 => Some("robot"), 18 => Some("spider"), 26 => Some("swing"), _ => None }
}

#[derive(Default, Clone, Copy)]
pub struct SyncOpts { pub skip_ball_input: bool, pub skip_spider_input: bool, pub force: bool }

fn copy_input_flags(from: &Player, to: &mut Player) {
    to.up_key_down = from.up_key_down;
    to.up_key_pressed = from.up_key_pressed;
    to.queued_hold = from.queued_hold;
    let consumed = from.orb_activation_consumed_for_press;
    to.orb_activation_consumed_for_press = from.up_key_down && (to.orb_activation_consumed_for_press || consumed);
}

fn copy_initial_flags(from: &Player, to: &mut Player) {
    to.is_mini = from.is_mini;
    to.mirrored = from.mirrored;
    to.gravity_flipped = !from.gravity_flipped;
    copy_input_flags(from, to);
}

impl<'a> Sim<'a> {
    /// (primary, secondary) whichever one `self.p` is.
    fn pair(&mut self) -> (&mut Player, &mut Player) {
        let other = self.other.as_deref_mut().expect("dual mode needs the other player");
        if self.e.second_player { (other, &mut *self.p) } else { (&mut *self.p, other) }
    }

    /// Runs `f` on a `Sim` for the secondary (`secondary = true`) or primary player, sharing globals, env and the log.
    fn run_as<R>(&mut self, secondary: bool, f: impl FnOnce(&mut Sim) -> R) -> R {
        let second_now = self.e.second_player;
        let log = std::mem::take(&mut self.log);
        let target_is_self = secondary == second_now;
        let (r, log) = {
            let p: &mut Player = if target_is_self { &mut *self.p } else { self.other.as_deref_mut().expect("dual mode needs the other player") };
            let mut s = Sim::new(p, &mut *self.g, &mut *self.e);
            s.log = log;
            s.e.second_player = secondary;
            let r = f(&mut s);
            s.e.second_player = second_now;
            (r, s.log)
        };
        self.log = log;
        r
    }

    /// `_ensureDualFlyBounds()`: the bounds are centred on `_dualBoundsCenterY` (set by the dual portal), else on the primary player.
    pub fn ensure_dual_fly_bounds(&mut self) {
        if !self.e.is_dual || self.e.portal_free_mode { return; }
        let ten_block = { let (p1, p2) = self.pair(); p1.is_flying || p1.is_wave || p1.is_ufo || p1.is_swing || p2.is_flying || p2.is_wave || p2.is_ufo || p2.is_swing };
        let span = if ten_block { F } else { F - A };
        let center = match self.e.dual_bounds_center_y { Some(v) => v, None => self.primary_y() };
        let floor = (((center - span / 2.0) / A).floor() * A).max(0.0);
        let ceiling = floor + span;
        if !self.e.fly_ground_active || self.e.fly_visual_only || self.e.fly_floor_y != Some(floor) || self.e.fly_ceiling_y != Some(ceiling) {
            self.level_fly(true, center, Some(span), None);
        }
    }

    fn primary_y(&mut self) -> f64 { self.pair().0.y }

    /// `_refreshFlyBoundsAfterDual`
    pub fn refresh_fly_bounds_after_dual(&mut self) {
        if self.e.portal_free_mode { return; }
        let (flying, wave, ufo, swing, ball, spider, y) = { let p = self.pair().0; (p.is_flying, p.is_wave, p.is_ufo, p.is_swing, p.is_ball, p.is_spider, p.y) };
        if flying || wave || ufo || swing {
            let span = self.e.fly_ceiling_y.unwrap_or(0.0) - self.e.fly_floor_y.unwrap_or(0.0);
            if self.e.fly_ground_active && !self.e.fly_visual_only && span == F { return; }
            self.level_fly(true, y, Some(F), None);
        }
        else if ball { self.level_fly(true, y, Some(F - A * 2.0), None); }
        else if spider { let h = A.max(F - A); self.level_fly(true, y, Some(h), Some(h)); }
        else { self.level_fly(false, 0.0, None, None); }
    }

    fn skip_ball_sync(from: &Player, to: &Player, o: SyncOpts) -> bool {
        if !from.is_ball { return false; }
        if o.force { return false; }
        if o.skip_ball_input { return true; }
        !(to.on_ground || to.on_ceiling || to.can_jump)
    }
    fn skip_spider_sync(from: &Player, o: SyncOpts) -> bool { from.is_spider && !o.force && o.skip_spider_input }

    /// `_syncDualGlobalsFromPrimary`
    pub fn sync_globals_from_primary(&mut self, o: SyncOpts) -> bool {
        if !self.e.is_dual { return false; }
        let synced = {
            let (p1, p2) = self.pair();
            if p2.is_dead { return false; }
            let mut synced = false;
            if !Self::skip_ball_sync(p1, p2, o) && !Self::skip_spider_sync(p1, o) { p2.gravity_flipped = !p1.gravity_flipped; synced = true; }
            p2.mirrored = p1.mirrored;
            synced
        };
        self.ensure_dual_fly_bounds();
        synced
    }

    /// `_syncDualGlobalsFromSecondary`
    pub fn sync_globals_from_secondary(&mut self, o: SyncOpts) -> bool {
        if !self.e.is_dual { return false; }
        let synced = {
            let (p1, p2) = self.pair();
            if p1.is_dead || p2.is_dead { return false; }
            let mut synced = false;
            if !Self::skip_ball_sync(p2, p1, o) && !Self::skip_spider_sync(p2, o) { p1.gravity_flipped = !p2.gravity_flipped; synced = true; }
            p1.mirrored = p2.mirrored;
            synced
        };
        self.ensure_dual_fly_bounds();
        synced
    }

    /// `_syncOtherDualGravityForBlueBoost`
    pub fn sync_other_dual_gravity(&mut self) {
        if !self.e.is_dual { return; }
        if self.other.is_none() { // isolated-player tests: the scene is a stub that only records the call
            self.log.push(if self.e.second_player { "syncdual:secondary" } else { "syncdual:primary" }.into());
            return;
        }
        let o = SyncOpts { force: true, ..Default::default() };
        if self.e.second_player { self.sync_globals_from_secondary(o); } else { self.sync_globals_from_primary(o); }
    }

    /// `_copyDualInputFlags(primary, secondary)` followed by the gravity-suppression rule, as `update` does it. Returns nothing; flags change on the secondary.
    pub fn copy_input_to_secondary(&mut self, input: &Player) {
        let p2 = self.pair().1;
        copy_input_flags(input, p2);
    }

    /// `_setPlayerGamemode(player, state, mode, keepVelocity)` for the player `self.p` is.
    pub fn set_player_gamemode(&mut self, mode: &str, keep_velocity: bool) {
        let current = mode_id(self.p);
        if current == mode { return; }
        let saved = (self.p.y, self.p.y_velocity, self.p.gravity_flipped, self.p.up_key_down, self.p.up_key_pressed, self.p.queued_hold, self.p.orb_activation_consumed_for_press, self.p.mirrored, self.p.is_mini);
        let y = saved.0;
        match mode {
            "swing" => self.enter_swing_mode(Some(y), true, false),
            "ship" => self.enter_ship_mode(Some(y), true, false),
            "ball" => self.enter_ball_mode(Some(y), false),
            "ufo" => self.enter_ufo_mode(Some(y), true, false),
            "wave" => self.enter_wave_mode(Some(y), false),
            "robot" => self.enter_robot_mode(true),
            "spider" => self.enter_spider_mode(Some(y), true, false),
            _ => {
                self.exit_swing_mode(); self.exit_ship_mode(); self.exit_ball_mode(); self.exit_ufo_mode(); self.exit_wave_mode(); self.exit_robot_mode(); self.exit_spider_mode();
                self.p.is_flying = false; self.p.is_swing = false; self.p.is_ball = false; self.p.is_ufo = false; self.p.is_wave = false; self.p.is_robot = false; self.p.is_spider = false;
            }
        }
        self.p.y = saved.0;
        if keep_velocity && saved.1.is_finite() { self.p.y_velocity = saved.1; }
        self.p.gravity_flipped = saved.2;
        self.p.up_key_down = saved.3;
        self.p.up_key_pressed = saved.4;
        self.p.queued_hold = saved.5;
        self.p.orb_activation_consumed_for_press = saved.6;
        self.p.mirrored = saved.7;
        self.p.is_mini = saved.8;
    }

    /// `_getInitialDualPortalMode`
    fn initial_dual_portal_mode(&mut self) -> Option<&'static str> {
        let world_x = self.e.player_world_x;
        let (p1_lx, p1_ly, p1_last_y, p2_y, p2_wave, p2_mini) = {
            let (p1, p2) = self.pair();
            (p1.last_collision_world_x, p1.last_collision_world_y, p1.last_y, p2.y, p2.is_wave, p2.is_mini)
        };
        let world_y = p2_y;
        if !world_x.is_finite() || !world_y.is_finite() { return None; }
        let half = if p2_wave { if p2_mini { 6.0 } else { 9.0 } } else if p2_mini { 18.0 } else { 30.0 };
        // `Number(null)` is 0, so a missing last position counts as 0 here, like in the original
        let prev_x_raw = p1_lx.unwrap_or(0.0);
        let prev_y_raw = p1_ly.unwrap_or(0.0);
        let prev_x = if prev_x_raw.is_finite() { prev_x_raw } else { world_x };
        let prev_y = if prev_y_raw.is_finite() { prev_y_raw } else if p1_last_y.is_finite() { p1_last_y } else { world_y };
        let cands = std::mem::take(&mut self.cands);
        // the shared nearby-buffer is refilled with the previous position's list (see `Sim::replace_idx`)
        if let Some(r) = self.replace_idx.clone() { self.replace_pending = Some(r); }
        let mut mode = None;
        for c in &cands {
            let Some(m) = portal_mode(c.ty) else { continue };
            let touching;
            if let Some(r) = c.hitbox_radius {
                let radius = r + half;
                let (dx, dy) = (world_x - c.x, world_y - c.y);
                let mut t = dx * dx + dy * dy <= radius * radius;
                if !t && (prev_x != world_x || prev_y != world_y) {
                    let (pdx, pdy) = (prev_x - c.x, prev_y - c.y);
                    t = pdx * pdx + pdy * pdy <= radius * radius;
                    if !t {
                        let steps = 12f64.min(2f64.max((((world_x - prev_x).abs()).max((world_y - prev_y).abs()) / 1f64.max(half)).ceil()));
                        let mut i = 1.0;
                        while i < steps {
                            let tt = i / steps;
                            let (sx, sy) = (prev_x + (world_x - prev_x) * tt, prev_y + (world_y - prev_y) * tt);
                            let (sdx, sdy) = (sx - c.x, sy - c.y);
                            if sdx * sdx + sdy * sdy <= radius * radius { t = true; break; }
                            i += 1.0;
                        }
                    }
                }
                touching = t;
            } else {
                touching = self.touching_portal(c, world_x, world_y, half, Some((prev_x, prev_y)));
            }
            if touching { mode = Some(m); }
        }
        self.cands = cands;
        mode
    }

    /// `_enableDualMode`
    pub fn enable_dual_mode(&mut self, portal_y: Option<f64>) {
        if self.other.is_none() { self.log.push("dual:on".into()); return; }
        if self.e.is_dual { return; }
        self.e.is_dual = true;
        self.e.dual_ball_spawn_lock = false;
        self.e.dual_ball_overlap_resolved = false;
        let world_x = self.e.player_world_x;
        {
            let (p1, p2) = self.pair();
            reset_state(p2);
            p2.is_dead = false;
            copy_initial_flags(p1, p2);
        }
        let p1y = self.primary_y();
        let primary_y = if p1y.is_finite() { p1y } else { 30.0 };
        {
            let p2 = self.pair().1;
            p2.y = primary_y; p2.last_y = primary_y; p2.last_ground_pos_y = primary_y;
            p2.on_ground = false; p2.on_ceiling = false; p2.can_jump = false; p2.is_jumping = false;
            reset_object(p2, world_x);
        }
        let mode = { let p1 = self.pair().0; mode_id(p1) };
        self.run_as(true, |s| s.set_player_gamemode(mode, true));
        { let (p1, p2) = self.pair(); copy_initial_flags(p1, p2); }
        if let Some(m) = self.initial_dual_portal_mode() { self.run_as(true, |s| s.set_player_gamemode(m, true)); }
        let balls = { let (p1, p2) = self.pair(); p1.is_ball && p2.is_ball };
        if balls {
            let (p1g, ()) = { let (p1, p2) = self.pair(); let g = p1.gravity_flipped; p2.gravity_flipped = !g; (g, ()) };
            let _ = p1g;
            self.e.dual_ball_overlap_resolved = true;
            self.e.dual_ball_spawn_lock = true;
        }
        let p1y = self.primary_y();
        self.e.dual_bounds_center_y = Some(portal_y.unwrap_or(p1y));
        self.ensure_dual_fly_bounds();
    }

    /// `_disableDualMode`
    pub fn disable_dual_mode(&mut self) {
        if self.other.is_none() { self.log.push("dual:off".into()); return; }
        if !self.e.is_dual { return; }
        if self.e.second_player && !self.p.is_dead {
            let mode = mode_id(self.p);
            self.run_as(false, |s| s.set_player_gamemode(mode, true));
            let (s2y, s2last, s2ground, s2vel, s2flip, s2mini, s2on_ground, s2on_ceiling, s2can_jump, s2jumping) = {
                let p = &*self.p;
                (p.y, p.last_y, p.last_ground_pos_y, p.y_velocity, p.gravity_flipped, p.is_mini, p.on_ground, p.on_ceiling, p.can_jump, p.is_jumping)
            };
            let (p1, _) = self.pair();
            p1.y = s2y; p1.last_y = s2last; p1.last_ground_pos_y = s2ground; p1.y_velocity = s2vel; p1.gravity_flipped = s2flip; p1.is_mini = s2mini;
            p1.on_ground = s2on_ground; p1.on_ceiling = s2on_ceiling; p1.can_jump = s2can_jump; p1.is_jumping = s2jumping;
        }
        self.e.is_dual = false;
        self.e.dual_ball_overlap_resolved = false;
        self.e.dual_ball_spawn_lock = false;
        { let p2 = self.pair().1; p2.is_dead = true; p2.up_key_down = false; p2.up_key_pressed = false; p2.queued_hold = false; p2.is_dashing = false; p2.dash_y_velocity = 0.0; }
        self.refresh_fly_bounds_after_dual();
    }

    pub fn balls_overlapping(&mut self) -> bool {
        if !self.e.is_dual { return false; }
        let (p1, p2) = self.pair();
        if p1.is_dead || p2.is_dead || !p1.is_ball || !p2.is_ball { return false; }
        let s1 = if p1.is_mini { 18.0 } else { 30.0 };
        let s2 = if p2.is_mini { 18.0 } else { 30.0 };
        (p1.y - p2.y).abs() < s1 + s2
    }

    fn on_surface(p: &Player) -> bool { p.on_ground || p.on_ceiling || p.can_jump }

    fn flip_ball_for_overlap(&mut self, secondary: bool) -> bool {
        self.run_as(secondary, |s| {
            if !s.p.is_ball { return false; }
            let flip_mod = if s.p.gravity_flipped { -1.0 } else { 1.0 };
            s.p.y_velocity = flip_mod * 22.360064 * if s.p.is_mini { 0.8 } else { 1.0 };
            let target = !s.p.gravity_flipped;
            s.flip_gravity(target, 0.5);
            s.p.on_ground = false; s.p.on_ceiling = false; s.p.can_jump = false; s.p.is_jumping = false;
            s.p.y_velocity *= 0.6;
            true
        })
    }

    /// `_resolveDualBallOverlap`
    pub fn resolve_dual_ball_overlap(&mut self) -> bool {
        if !self.balls_overlapping() {
            self.e.dual_ball_overlap_resolved = false;
            self.e.dual_ball_spawn_lock = false;
            return false;
        }
        if self.e.dual_ball_spawn_lock {
            let (same, target) = { let (p1, p2) = self.pair(); (p2.gravity_flipped == p1.gravity_flipped, !p1.gravity_flipped) };
            if same {
                self.run_as(true, |s| { s.flip_gravity(target, 1.0); s.p.on_ceiling = false; });
            }
            self.e.dual_ball_overlap_resolved = true;
            return false;
        }
        if self.e.dual_ball_overlap_resolved { return false; }
        let (a, b) = { let (p1, p2) = self.pair(); (Self::on_surface(p1), Self::on_surface(p2)) };
        if !a && !b { return false; }
        let flipped = if a && !b { self.flip_ball_for_overlap(true) } else if b && !a { self.flip_ball_for_overlap(false) } else { self.flip_ball_for_overlap(true) };
        if flipped { self.e.dual_ball_overlap_resolved = true; }
        flipped
    }

    /// `_killDualPlayers`-style: kills the secondary player (`_player2.killPlayer()`).
    pub fn kill_secondary(&mut self) { self.run_as(true, |s| s.kill_player()); }
    pub fn kill_secondary_target_primary(&mut self) { self.run_as(false, |s| s.kill_player()); }
}

#[allow(dead_code)]
fn _unused(_: &Col) {}

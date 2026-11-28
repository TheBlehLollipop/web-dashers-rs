//! `PlayerObject.checkCollisions` and the helpers it uses (player.js), ported in layers.
//! Done: solids, hazards, sblocks, floor/ceiling, landing, death, slope bookkeeping, snap jump.
//! Still `unimplemented!`: portals, speed portals, pads, orbs, coins, slope surfaces.
use crate::jsmath as m;
use crate::physics::{js_round, Sim};

fn or_zero(x: f64) -> f64 { if x == 0.0 || x.is_nan() { 0.0 } else { x } }

pub const TYPES: &[&str] = &[
    "solid", "hazard", "dblock", "sblock", "slope", "jump_pad", "jump_ring", "coin", "speed",
    "portal_gravity_down", "portal_gravity_up", "portal_gravity_toggle", "portal_fly", "portal_cube", "portal_ball",
    "portal_wave", "portal_ufo", "portal_robot", "portal_spider", "portal_teleport", "portal_mirror_on", "portal_mirror_off",
    "portal_mini_on", "portal_mini_off", "portal_dual_on", "portal_dual_off", "portal_swing", "deco", "hblock", "fblock", "jblock", "forceblock",
];
pub fn type_code(name: &str) -> f64 { TYPES.iter().position(|t| *t == name).unwrap_or(28) as f64 }
const T_SOLID: f64 = 0.0;
const T_HAZARD: f64 = 1.0;
const T_DBLOCK: f64 = 2.0;
const T_SBLOCK: f64 = 3.0;
pub const T_SLOPE: f64 = 4.0;
const T_PAD: f64 = 5.0;
const T_RING: f64 = 6.0;
const T_COIN: f64 = 7.0;
const T_SPEED: f64 = 8.0;
const T_TELEPORT: f64 = 19.0;
pub const T_DECO: f64 = 27.0;
const T_HBLOCK: f64 = 28.0;
const T_FBLOCK: f64 = 29.0;
const T_JBLOCK: f64 = 30.0;
const T_FORCEBLOCK: f64 = 31.0;
const T_PORTAL_SWING: f64 = 26.0;
fn is_portal(t: f64) -> bool { (9.0..=26.0).contains(&t) }

use crate::physics::Slot;
macro_rules! col_slots {
    ($(pub $f:ident: $t:ty = $def:expr,)*) => {
        /// A collider as the player code sees it (numeric type code instead of a string).
        #[derive(Debug, Clone)]
        pub struct Col { $(pub $f: $t,)* }
        impl Default for Col { fn default() -> Self { Col { $($f: $def,)* } } }
        impl Col {
            pub fn names(out: &mut Vec<String>) { $(out.push(format!("{}:{}", stringify!($f), stringify!($t).replace(" ", "")));)* }
            pub fn len() -> usize { let mut v = Vec::new(); Col::names(&mut v); v.len() }
            pub fn pack(&self, out: &mut Vec<f64>) { $(out.push(Slot::get(&self.$f));)* }
            pub fn unpack(it: &mut dyn Iterator<Item = f64>) -> Col { Col { $($f: <$t as Slot>::set(it.next().unwrap()),)* } }
        }
    };
}
col_slots! {
    pub ty: f64 = 27.0,
    pub x: f64 = 0.0,
    pub y: f64 = 0.0,
    pub w: f64 = 0.0,
    pub h: f64 = 0.0,
    pub rotation_degrees: f64 = 0.0,
    pub hitbox_radius: Option<f64> = None,
    pub act_main: bool = false,
    pub act_dual: bool = false,
    pub activated: bool = false,
    pub objid: Option<f64> = None,
    pub uid: f64 = 0.0,
    pub id: Option<f64> = None,
    pub multi_activate: bool = false,
    pub free_mode: bool = false,
    pub flip_y: bool = false,
    pub orb_rotation: f64 = 0.0,
    pub speed_value: Option<f64> = None,
    pub teleport_target_x: Option<f64> = None,
    pub teleport_target_y: Option<f64> = None,
    pub portal_x: Option<f64> = None,
    pub portal_y: Option<f64> = None,
    pub dash_hold_main: f64 = 0.0,
    pub dash_hold_dual: f64 = 0.0,
    pub slope_angle_deg: f64 = 0.0,
    pub slope_dir: f64 = 1.0,
    pub slope_is_filled: bool = false,
    pub slope_flip_y: bool = false,
    pub hypo_ax: f64 = 0.0,
    pub hypo_ay: f64 = 0.0,
    pub hypo_bx: f64 = 0.0,
    pub hypo_by: f64 = 0.0,
    pub right_ax: f64 = 0.0,
    pub right_ay: f64 = 0.0,
    pub slope_solid_below: bool = true,
    pub slope_type: Option<f64> = None,
    pub gd_solid_above: bool = false,
    pub gd_wall_on_left: bool = false,
    pub secret_coin: bool = false,
    pub user_coin: bool = false,
    pub secret_completed: bool = false,
    pub user_completed: bool = false,
    pub claim_touch: bool = false,
    pub ee_toggle_press: Option<f64> = None,
    pub force: f64 = 0.0,
    pub force_min: f64 = 0.0,
    pub force_max: f64 = 0.0,
    pub force_radial: bool = false,
    pub force_range: bool = false,
    pub force_id: f64 = 0.0,
    pub force_flip_y: bool = false,
}

impl Col {
    /// `Collider.getSlopeSurfaceY`
    pub fn slope_surface_y(&self, world_x: f64) -> Option<f64> {
        if self.ty != T_SLOPE { return None; }
        let (ax, ay) = (self.x + self.hypo_ax, self.y + self.hypo_ay);
        let (bx, by) = (self.x + self.hypo_bx, self.y + self.hypo_by);
        let (lo, hi) = if ax <= bx { ((ax, ay), (bx, by)) } else { ((bx, by), (ax, ay)) };
        if hi.0 - lo.0 < 0.01 { return None; }
        if world_x < lo.0 || world_x > hi.0 { return None; }
        let t = (world_x - lo.0) / (hi.0 - lo.0);
        Some(lo.1 + t * (hi.1 - lo.1))
    }
    /// `Collider.getSlopeAngleRad`
    pub fn slope_angle_rad(&self) -> f64 {
        if self.ty != T_SLOPE { return 0.0; }
        m::atan2(self.hypo_by - self.hypo_ay, self.hypo_bx - self.hypo_ax)
    }
}

impl<'a> Sim<'a> {
    /// `fx:x,y,type,id,flags` — what `_orbpadHitEffect` needs (flags: 1 secret coin, 2 user coin)
    /// `_markActivatedOrbSprites`: the visual layer starts the click pulse of the orb sprites at this position
    fn mark_orb(&mut self, x: f64, y: f64) { self.vlog.push(format!("orb:{},{}", x, y)); }
    fn fx_line(c: &Col) -> String { format!("fx:{},{},{},{},{}", c.x, c.y, c.ty, c.id.unwrap_or(-1.0), c.secret_coin as u8 | (c.user_coin as u8) << 1) }

    /// `tryCollectSecretCoin` / `tryCollectUserCoin` (the harnesses answer through `Env` instead)
    fn coin_gate(&mut self, col: &Col) -> bool {
        match self.coins.as_deref_mut() {
            Some(c) => c.gate(col),
            None => !(col.secret_coin && !col.secret_completed && !self.e.try_collect_secret) && !(col.user_coin && !col.user_completed && !self.e.try_collect_user),
        }
    }
    pub fn stop_rotation(&mut self) { self.p.rotate_action_active = false; }
    /// `_iconEffectBoostTrail` (the trail starts for a boost in the air): visual only
    pub(crate) fn boost_trail(&mut self, on: bool) { self.vlog.push(format!("boost:{},{}", self.e.second_player as u8, on as u8)); }

    /// `_isDashSpinning`: a dash orb in the plain cube form
    pub fn is_dash_spinning(&self) -> bool {
        let p = &self.p;
        p.is_dashing && !p.is_flying && !p.is_ball && !p.is_wave && !p.is_ufo && !p.is_spider && !p.is_robot && !p.is_swing
    }

    fn ticking(t: Option<f64>) -> bool { matches!(t, Some(v) if v > 0.0) }

    pub fn release_jump_hold(&mut self) {
        self.p.up_key_down = false;
        self.p.up_key_pressed = false;
        self.p.queued_hold = false;
        self.e.stop_jump_latch = true;
    }

    fn landing_game_event(&mut self) {
        if !self.e.has_level_events || self.p.is_wave { return; }
        if self.p.gd_fall_start_y.is_some() { self.p.gd_fall_start_y = None; }
        self.p.gd_land_flag = true;
    }

    pub fn hit_ground(&mut self, skip_burst: bool) {
        if Self::ticking(self.p.j_block_ticks) && !self.p.on_ground { self.release_jump_hold(); }
        self.boost_trail(false);
        let dash_sliding = self.is_dash_spinning();
        let skip_burst = skip_burst || dash_sliding;
        self.p.slope_launch_spin = false;
        let burst = !skip_burst && !self.p.on_ground;
        if !self.p.on_ground { self.landing_game_event(); }
        if !self.p.is_flying && !self.p.is_wave && !self.p.is_ufo && !self.p.is_swing { self.p.last_ground_y = self.p.y; }
        self.p.y_velocity = 0.0;
        self.p.on_ground = !dash_sliding;
        self.p.can_jump = !dash_sliding;
        self.p.is_jumping = false;
        self.p.queued_hold = false;
        if self.p.is_ball {
            if burst { self.p.rotation = js_round(self.p.rotation / std::f64::consts::PI) * std::f64::consts::PI; }
            self.p.ball_should_rotate = true;
            self.p.ball_rotate_opposite = false;
            self.p.ball_hit_pad = false;
            let dir = if self.p.gravity_flipped { -1.0 } else { 1.0 };
            self.p.ball_normal_rotate = if self.p.mirrored { -dir } else { dir };
        } else if self.p.is_spider {
            if burst { self.p.rotation = js_round(self.p.rotation / std::f64::consts::PI) * std::f64::consts::PI; }
        } else if self.p.is_wave {
            self.p.rotation = 0.0;
        }
        if !dash_sliding { self.stop_rotation(); }
        if burst && !self.p.is_flying && !self.p.is_wave && !self.p.is_spider && !self.p.is_swing { self.vlog.push(format!("land:{}", self.p.gravity_flipped as u8)); }
    }

    pub fn kill_player(&mut self) {
        if self.p.is_dead || self.e.macro_playing { return; }
        self.p.is_dead = true;
        self.boost_trail(false);
        self.p.is_dashing = false;
        self.p.dash_y_velocity = 0.0;
        self.log.push("kill".into());
    }

    fn check_snap_jump(&mut self, obj: &Col) {
        if self.p.is_swing { return; }
        const OFFS: [(f64, f64); 3] = [(240.0, 60.0), (300.0, -60.0), (180.0, 120.0)];
        if let Some(last_uid) = self.p.last_land_object {
            let (lt, lx, ly) = (self.p.last_land_ty, self.p.last_land_x, self.p.last_land_y);
            if last_uid != obj.uid && (lt == T_SOLID || lt == T_SLOPE) {
                let sign = if self.p.gravity_flipped { -1.0 } else { 1.0 };
                let mut snap = false;
                for (dx, dy) in OFFS {
                    if (obj.x - (lx + dx)).abs() <= 2.0 && (obj.y - (ly + dy * sign)).abs() <= 2.0 { snap = true; break; }
                }
                if snap {
                    let target = obj.x + self.p.last_x_offset;
                    let pwx = self.e.player_world_x;
                    let nx = if (target - pwx).abs() <= 2.0 { target } else if target > pwx { pwx + 2.0 } else { pwx - 2.0 };
                    self.e.player_world_x = nx;
                }
            }
        }
        self.p.last_land_object = Some(obj.uid);
        self.p.last_land_ty = obj.ty;
        self.p.last_land_x = obj.x;
        self.p.last_land_y = obj.y;
        self.p.last_x_offset = self.e.player_world_x - obj.x;
    }

    pub(crate) fn apply_slope_exit_velocity(&mut self) {
        let exit = match self.p.slope_exit_vel { Some(v) => v, None => return };
        if self.p.skip_slope_exit { return; }
        if self.p.is_wave { return; }
        if self.p.is_jumping && !self.p.is_flying && !self.p.is_ufo && !self.p.is_wave && !self.p.is_swing { return; }
        let up_sign = if self.p.gravity_flipped { -1.0 } else { 1.0 };
        if exit * up_sign < 0.0 {
            if self.p.up_key_down { return; }
            self.p.y_velocity = js_round(exit * 1000.0) / 1000.0;
            self.p.on_ground = false;
            self.p.can_jump = false;
            let p = &*self.p;
            if !p.is_ball && !p.is_flying && !p.is_swing && !p.is_wave && !p.is_ufo && !p.is_spider && !p.is_robot && !p.is_dashing {
                self.run_rotate_action(1.0);
            }
            return;
        }
        let exit_scale = self.slope_exit_scale();
        let exit_cap = if self.p.is_swing { self.g.slope_swing_cap } else { self.g.slope_exit_cap };
        let exit_vel = exit * exit_scale;
        let dir = if exit_vel >= 0.0 { 1.0 } else { -1.0 };
        if exit_vel * dir <= self.p.y_velocity * dir { return; }
        self.p.y_velocity = (-exit_cap).max(exit_cap.min(js_round(exit_vel * 1000.0) / 1000.0));
        self.p.on_ground = false;
        self.p.can_jump = false;
        let rising = if self.p.gravity_flipped { self.p.y_velocity < 0.0 } else { self.p.y_velocity > 0.0 };
        let is_cube = !self.p.is_ball && !self.p.is_swing && !self.p.is_flying;
        let p = &*self.p;
        if rising && is_cube && !p.is_wave && !p.is_ufo && !p.is_spider && !p.is_robot && !p.is_dashing {
            let half_turn = if self.p.is_mini { 2.0 / 3.0 } else { 0.8666667 };
            self.run_rotate_action(-1.0);
            self.p.rotate_action_duration = half_turn * 16.0;
            self.p.rotate_action_total = -std::f64::consts::PI * 16.0 * self.flip_mod();
            self.p.slope_launch_spin = true;
        }
    }

    /// `checkCollisions(offset)`. `cols` is the nearby-section list, in order.
    /// Applies a refill of the shared nearby buffer (see `Sim::replace_idx`) at the start of a loop iteration.
    fn apply_replace(&mut self, cols: &mut Vec<Col>) {
        let Some(idxs) = self.replace_pending.take() else { return };
        let new: Vec<Col> = idxs.iter().map(|gi| match self.near_idx.iter().position(|x| x == gi) {
            Some(k) => cols[k].clone(),
            None => self.replace_cols.get(gi).cloned().expect("candidate collider"),
        }).collect();
        for (k, c) in cols.drain(..).enumerate() { self.stash.push((self.near_idx[k], c)); }
        *cols = new;
        self.near_idx = idxs;
    }

    pub fn check_collisions(&mut self, piece_offset: f64, cols: &mut Vec<Col>) {
        self.p.nc_total_frames += 1.0;
        self.p.died_this_frame = false;
        for t in [&mut self.p.safe_head_ticks, &mut self.p.flip_head_ticks, &mut self.p.j_block_ticks] {
            if let Some(v) = *t { if v > 0.0 { *t = Some(v - 1.0); } }
        }
        self.force_ids.clear();
        if self.p.spider_teleport_noclip_death_pending {
            self.p.died_this_frame = true;
            self.p.spider_teleport_noclip_death_pending = false;
        }
        if !self.p.up_key_down || (self.p.up_key_down && !self.p.was_up_key_down) {
            self.p.orb_activation_consumed_for_press = false;
        }
        let player_size = if self.p.is_mini { 18.0 } else { 30.0 };
        let wave_hit = if self.p.is_mini { 6.0 } else { 9.0 };
        let piece_width = piece_offset + self.g.center_x;
        let previous_world_x = self.p.last_collision_world_x.filter(|v| v.is_finite()).unwrap_or(piece_width);
        let players_y = self.p.y;
        let players_last_y = self.p.last_y;
        let previous_world_y = self.p.last_collision_world_y.filter(|v| v.is_finite()).unwrap_or(players_last_y);
        let gamemode_addition = if self.p.is_wave { 0.0 } else if self.p.is_flying || self.p.is_ufo || self.p.is_swing { 12.0 } else { 20.0 };
        self.p.collide_top = 0.0;
        self.p.collide_bottom = 0.0;
        self.p.on_ceiling = false;
        self.p.touching_ring = false;
        self.p.on_slope_angle = None;
        self.p.active_slope_obj = None;
        self.p.active_slope_angle = None;
        self.p.gd_was_on_slope = self.p.gd_is_on_slope;
        self.p.gd_is_on_slope = false;
        self.p.prev_ship_head_uid = self.p.ship_head_uid;
        self.p.prev_ship_head_flipped = self.p.ship_head_uid.is_some() && self.p.ship_head_flipped;
        self.p.ship_head_uid = None;
        self.p.ship_head_flipped = false;
        let p = &*self.p;
        let mode_sig = (p.is_flying as u32) | ((p.is_ball as u32) << 1) | ((p.is_wave as u32) << 2) | ((p.is_ufo as u32) << 3)
            | ((p.is_spider as u32) << 4) | ((p.is_robot as u32) << 5) | ((p.is_swing as u32) << 6);
        let mode_sig = mode_sig as f64;
        if self.p.gd_mode_sig != Some(mode_sig) {
            if self.p.gd_mode_sig.is_some() { self.p.gd_mode_tick = Some(self.e.physics_tick); }
            self.p.gd_mode_sig = Some(mode_sig);
        }
        let mut grounded = false; // `_0x30410f`
        let mut boosted_this_step = false;
        let mut teleported_this_step = false;
        let mut orb_input_consumed_this_step = false;
        let mut touching_teleport_during_ignore = false;
        let mut best_slope_candidate: Option<(crate::pslope::Cand, f64)> = None;
        let prefer_highest = !self.p.gravity_flipped;
        let mut slope_kill_pending = false;
        let mut snapped_slope_this_frame = false;
        let mut slope_tangent_this_frame = 0.0;
        let mut slope_blocked_this_step = false;
        let _ = &mut slope_kill_pending;
        self.p.wave_slide_active = false;
        if self.p.is_wave {
            for d in cols.iter() {
                if d.ty != T_DBLOCK { continue; }
                let rad = d.rotation_degrees * std::f64::consts::PI / 180.0;
                let hw = (d.w / 2.0 * m::cos(rad)).abs() + (d.h / 2.0 * m::sin(rad)).abs();
                let hh = (d.w / 2.0 * m::sin(rad)).abs() + (d.h / 2.0 * m::cos(rad)).abs();
                if piece_width + wave_hit > d.x - hw && piece_width - wave_hit < d.x + hw && players_y + wave_hit > d.y - hh && players_y - wave_hit < d.y + hh {
                    self.p.wave_slide_active = true;
                    break;
                }
            }
        }
        let mut next_i = 0usize;
        loop {
            self.apply_replace(cols);
            if next_i >= cols.len() { break; }
            let i = next_i;
            next_i += 1;
            let (mut left, mut right, mut top, mut bottom);
            let (rhw, rhh);
            let (x, y, w, h, ty, rot, circle, uid);
            {
                let c = &cols[i];
                x = c.x; y = c.y; w = c.w; h = c.h; ty = c.ty; rot = c.rotation_degrees; circle = c.hitbox_radius; uid = c.uid;
            }
            let _ = uid;
            left = x - w / 2.0; right = x + w / 2.0; top = y - h / 2.0; bottom = y + h / 2.0;
            {
                let rad = rot * std::f64::consts::PI / 180.0;
                let (cos, sin) = (m::cos(rad), m::sin(rad));
                let (half_w, half_h) = (w / 2.0, h / 2.0);
                rhw = (half_w * cos).abs() + (half_h * sin).abs();
                rhh = (half_w * sin).abs() + (half_h * cos).abs();
            }
            let (rl, rr, rt, rb) = (x - rhw, x + rhw, y - rhh, y + rhh);
            let broad_size = if self.p.is_wave { wave_hit } else { player_size };
            let has_circle = circle.is_some();
            let mut broad_hit;
            if let Some(r) = circle {
                let (dx, dy) = (piece_width - x, players_y - y);
                broad_hit = (dx * dx + dy * dy) <= (r + broad_size) * (r + broad_size);
            } else if ty == T_HAZARD && rot % 360.0 != 0.0 {
                let player_rotation = if self.p.is_wave { 0.0 } else { -self.p.rotation * 180.0 / std::f64::consts::PI };
                broad_hit = (piece_width - x).abs() < broad_size + w / 2.0 && (players_y - y).abs() < broad_size + h / 2.0
                    && Self::rotated_rectangles_overlap(piece_width, players_y, broad_size, broad_size, player_rotation, x, y, w / 2.0, h / 2.0, -rot);
            } else {
                broad_hit = !(piece_width + broad_size <= rl) && !(piece_width - broad_size >= rr) && !(players_y + broad_size <= rt) && !(players_y - broad_size >= rb);
            }
            if !broad_hit && ty == T_SLOPE && self.was_holding_ship_head_slope(&cols[i], piece_width) { broad_hit = true; }
            if !broad_hit && ty == T_TELEPORT {
                broad_hit = self.touching_portal(&cols[i], piece_width, players_y, broad_size, Some((previous_world_x, previous_world_y)));
            }
            if !broad_hit { continue; }
            let is_pc = is_portal(ty) || ty == T_SPEED;
            if !has_circle && is_pc {
                broad_hit = if ty == T_TELEPORT {
                    self.touching_portal(&cols[i], piece_width, players_y, broad_size, Some((previous_world_x, previous_world_y)))
                } else {
                    self.touching_portal(&cols[i], piece_width, players_y, broad_size, None)
                };
                if !broad_hit { continue; }
            }
            if ty == T_TELEPORT && self.p.ignore_teleport_until_clear {
                touching_teleport_during_ignore = true;
                self.set_activated(&mut cols[i], true);
                continue;
            }
            if self.p.ignore_portals && is_pc {
                if ty == T_TELEPORT { touching_teleport_during_ignore = true; }
                self.set_activated(&mut cols[i], true);
                continue;
            }
            if (matches!(ty, t if t == 12.0 || t == 13.0 || t == 14.0 || t == 15.0 || t == 16.0 || t == 17.0 || t == 18.0 || t == T_PORTAL_SWING) || ty == 24.0 || ty == 25.0) && !self.is_activated(&cols[i]) {
                self.set_portal_free_mode(cols[i].free_mode);
            }
            let make_guide = is_pc && !self.is_activated(&cols[i]);
            if make_guide { self.vlog.push(Self::fx_line(&cols[i])); }
            if is_pc && !self.is_activated(&cols[i]) { self.p.gd_land_flag = false; }
            let portal_y = cols[i].portal_y.unwrap_or(cols[i].y);
            let free = cols[i].free_mode;
            if ty == T_TELEPORT {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    let mut target_y = cols[i].teleport_target_y.unwrap_or(f64::NAN);
                    if target_y.is_finite() {
                        if let Some(cy) = self.e.ceiling_y {
                            let exit = cy - player_size - 1.0;
                            if target_y >= exit { target_y = exit; }
                        }
                        self.p.y = target_y;
                        self.p.last_y = target_y;
                        self.p.last_ground_pos_y = target_y;
                        if (target_y - players_y).abs() > 0.001 {
                            self.p.on_ground = false;
                            self.p.can_jump = false;
                        }
                        boosted_this_step = true;
                        teleported_this_step = true;
                        self.p.last_collision_world_y = Some(target_y);
                        break;
                    }
                }
            } else if ty == 12.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    self.exit_robot_mode(); self.exit_ball_mode(); self.exit_wave_mode(); self.exit_ship_mode(); self.exit_ufo_mode(); self.exit_swing_mode();
                    self.enter_ship_mode(Some(portal_y), false, free);
                }
            } else if ty == 15.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    self.exit_robot_mode(); self.exit_ball_mode(); self.exit_ship_mode(); self.exit_wave_mode(); self.exit_ufo_mode(); self.exit_swing_mode();
                    self.enter_wave_mode(Some(portal_y), free);
                }
            } else if ty == 16.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    self.exit_robot_mode(); self.exit_ball_mode(); self.exit_wave_mode(); self.exit_ship_mode(); self.exit_swing_mode();
                    self.enter_ufo_mode(Some(portal_y), false, free);
                }
            } else if ty == 13.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    self.exit_spider_mode(); self.exit_robot_mode(); self.exit_ship_mode(); self.exit_ball_mode(); self.exit_wave_mode(); self.exit_ufo_mode(); self.exit_swing_mode();
                }
            } else if ty == 14.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    self.exit_robot_mode(); self.exit_ship_mode(); self.exit_wave_mode(); self.exit_ufo_mode(); self.exit_ball_mode(); self.exit_swing_mode();
                    self.enter_ball_mode(Some(portal_y), free);
                }
            } else if ty == 17.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    let vy = self.p.y_velocity;
                    self.exit_ship_mode(); self.exit_ball_mode(); self.exit_wave_mode(); self.exit_ufo_mode(); self.exit_spider_mode(); self.exit_robot_mode(); self.exit_swing_mode();
                    if vy.is_finite() { self.p.y_velocity = vy; }
                    self.enter_robot_mode(true);
                }
            } else if ty == 18.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    let vy = self.p.y_velocity * if self.p.is_swing && !self.p.slope_riding { 0.5 } else { 1.0 };
                    self.exit_ship_mode(); self.exit_robot_mode(); self.exit_ball_mode(); self.exit_wave_mode(); self.exit_ufo_mode(); self.exit_spider_mode(); self.exit_swing_mode();
                    if vy.is_finite() { self.p.y_velocity = vy; }
                    self.enter_spider_mode(Some(portal_y), true, free);
                }
            } else if ty == T_PORTAL_SWING {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    self.enter_swing_mode(Some(portal_y), false, free);
                }
            } else if ty == 9.0 || ty == 10.0 || ty == 11.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    if self.p.is_ball { self.p.ball_should_rotate = true; self.p.ball_rotate_opposite = true; }
                    if (ty == 9.0 && self.p.gravity_flipped) || (ty == 10.0 && !self.p.gravity_flipped) || ty == 11.0 { self.boost_trail(true); }
                    let target = if ty == 9.0 { false } else if ty == 10.0 { true } else { !self.p.gravity_flipped };
                    if (ty == 9.0 && self.p.gravity_flipped) || (ty == 10.0 && !self.p.gravity_flipped) || ty == 11.0 { self.vlog.push(format!("grav:{}", target as u8)); }
                    self.flip_gravity(target, 0.5);
                }
            } else if ty == 20.0 || ty == 21.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    self.log.push(format!("mirror:{}", ty == 20.0));
                }
            } else if ty == 22.0 || ty == 23.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    self.p.is_mini = ty == 22.0;
                }
            } else if ty == 24.0 || ty == 25.0 {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    if ty == 24.0 { self.enable_dual_mode(Some(portal_y)); } else { self.disable_dual_mode(); }
                }
            } else if ty == T_SPEED {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    if let Some(v) = cols[i].speed_value { self.g.player_speed = v; }
                }
            }
            if make_guide {
                // `_makeportalguide`: visual only
            } else if ty == T_PAD {
                if !self.is_activated(&cols[i]) {
                    self.set_activated(&mut cols[i], true);
                    self.vlog.push(Self::fx_line(&cols[i]));
                    self.p.gd_land_flag = false;
                    let pad_id = cols[i].id;
                    if pad_id == Some(67.0) {
                        let tick = self.e.physics_tick;
                        if tick.is_finite() {
                            if tick - self.p.last_blue_pad_tick.unwrap_or(f64::NEG_INFINITY) < 5.0 { continue; }
                            self.p.last_blue_pad_tick = Some(tick);
                        }
                    }
                    let fm = self.flip_mod();
                    if pad_id == Some(3005.0) {
                        let ps = if self.p.is_mini { 18.0 } else { 30.0 };
                        if !self.spider_teleport(ps, cols) { continue; }
                        self.sync_other_dual_gravity();
                        self.vlog.push(format!("grav:{}", self.p.gravity_flipped as u8));
                        if self.p.is_ball {
                            let dir = if self.p.gravity_flipped { -1.0 } else { 1.0 };
                            self.p.ball_normal_rotate = if self.p.mirrored { -dir } else { dir };
                            self.p.ball_should_rotate = true;
                            self.p.ball_rotate_opposite = self.p.gravity_flipped;
                            self.p.ball_hit_pad = true;
                        }
                        self.p.is_jumping = false;
                        boosted_this_step = true;
                        teleported_this_step = true;
                        best_slope_candidate = None;
                        break;
                    } else {
                        let pad_mini = self.p.is_mini;
                        let mut bump = 1.0;
                        let mut pad_flip = false;
                        if pad_id == Some(140.0) {
                            bump = if self.p.is_flying || self.p.is_swing { 0.35 } else if self.p.is_ufo { 0.4 } else if self.p.is_ball || self.p.is_spider { 0.7 } else { 0.65 };
                        } else if pad_id == Some(1332.0) {
                            bump = if self.p.is_flying || self.p.is_swing { if pad_mini { 0.95 } else { 0.63 } } else if self.p.is_ufo { if pad_mini { 0.98 } else { 0.6 } } else { 1.25 };
                        } else if pad_id == Some(67.0) {
                            bump = 0.8;
                            pad_flip = true;
                        }
                        let mut pad_vel = 16.0 * 2.0 * bump * (if pad_mini { 0.8 } else { 1.0 }) * (if self.p.is_ball || self.p.is_spider { 0.6 } else { 1.0 });
                        if self.p.is_swing {
                            if pad_id == Some(35.0) { pad_vel = 19.2; }
                            else if pad_id == Some(140.0) { pad_vel = 12.48; }
                            else if pad_id == Some(1332.0) { pad_vel = 24.0; }
                            else if pad_id == Some(67.0) { pad_vel = 7.68; }
                        }
                        self.p.gd_accel = pad_id == Some(1332.0) || self.p.is_swing;
                        if self.p.is_robot && self.g.robot_pad_touch { self.p.robot_touched_pad = true; }
                        self.p.is_jumping = true;
                        self.p.gd_land_flag = false;
                        self.p.on_ground = false;
                        self.p.can_jump = false;
                        self.p.y_velocity = crate::physics::gd_trunc(fm * pad_vel);
                        if self.p.is_ball {
                            let dir = if self.p.gravity_flipped { -1.0 } else { 1.0 };
                            self.p.ball_normal_rotate = if self.p.mirrored { -dir } else { dir };
                            self.p.ball_should_rotate = true;
                            self.p.ball_rotate_opposite = self.p.gravity_flipped;
                            self.p.ball_hit_pad = true;
                        }
                        if pad_flip {
                            let flipped = !self.p.gravity_flipped;
                            self.flip_gravity(flipped, if self.p.is_swing { 1.0 } else { 0.5 });
                            if pad_id == Some(67.0) { self.sync_other_dual_gravity(); }
                        }
                        self.run_rotate_action(1.0);
                        boosted_this_step = true;
                        self.boost_trail(true);
                    }
                }
            } else if ty == T_RING {
                let orb_id = cols[i].id;
                let is_dash = orb_id == Some(1704.0) || orb_id == Some(1751.0);
                let just_pressed = self.p.up_key_down && !self.p.was_up_key_down;
                let needs_click = !orb_input_consumed_this_step && !self.p.orb_activation_consumed_for_press
                    && (if self.p.is_flying || self.p.is_ufo || self.p.is_wave || self.p.is_swing { just_pressed } else { just_pressed || (self.p.queued_hold && self.p.up_key_down) });
                if orb_id != Some(3643.0) || cols[i].claim_touch { self.p.touching_ring = true; }
                if orb_id == Some(3643.0) {
                    if needs_click && (cols[i].multi_activate || !self.is_activated(&cols[i])) && cols[i].ee_toggle_press != self.p.press_count {
                        cols[i].ee_toggle_press = self.p.press_count;
                        self.set_activated(&mut cols[i], true);
                        if cols[i].claim_touch {
                            orb_input_consumed_this_step = true;
                            self.consume_orb_activation_input();
                        }
                    }
                    continue;
                }
                if (cols[i].multi_activate || !self.is_activated(&cols[i])) && needs_click {
                    self.vlog.push(Self::fx_line(&cols[i]));
                    self.p.gd_land_flag = false;
                    if is_dash {
                        let hold = self.dash_hold_ticks(&cols[i]) + 1.0;
                        self.set_dash_hold_ticks(&mut cols[i], hold);
                        if hold < 2.0 {
                            self.set_activated(&mut cols[i], true);
                            orb_input_consumed_this_step = true;
                            self.consume_orb_activation_input();
                            let dash_angle = cols[i].orb_rotation;
                            let dash_rad = dash_angle * std::f64::consts::PI / 180.0;
                            let max_sin = m::sin(70.0 * std::f64::consts::PI / 180.0);
                            let raw_sin = -m::sin(dash_rad);
                            let dash_sin = (-max_sin).max(max_sin.min(raw_sin));
                            let dash_vel_y = dash_sin * 18.0;
                            if orb_id == Some(1751.0) {
                                let flipped = !self.p.gravity_flipped;
                                self.flip_gravity(flipped, 0.5);
                                self.sync_other_dual_gravity();
                            }
                            self.p.is_dashing = true;
                            self.p.dash_y_velocity = dash_vel_y;
                            self.p.y_velocity = dash_vel_y;
                            self.p.on_ground = false;
                            self.p.can_jump = false;
                            self.p.is_jumping = false;
                            if self.p.is_ball {
                                let dir = if self.p.gravity_flipped { -1.0 } else { 1.0 };
                                self.p.ball_normal_rotate = if self.p.mirrored { -dir } else { dir };
                                self.p.ball_should_rotate = true;
                                self.p.ball_rotate_opposite = false;
                                self.p.ball_hit_pad = true;
                                self.p.ball_rotation2 = 0.0;
                            }
                            self.stop_rotation();
                            boosted_this_step = true;
                            self.boost_trail(true);
                            self.mark_orb(cols[i].x, cols[i].y);
                        }
                    } else {
                        self.set_activated(&mut cols[i], true);
                        orb_input_consumed_this_step = true;
                        self.consume_orb_activation_input();
                        let fm = self.flip_mod();
                        let mut orb_vel = 0.0;
                        let mut flip_before = false;
                        let mut flip_after = false;
                        if orb_id == Some(1594.0) {
                            let flipped = !self.p.gravity_flipped;
                            self.flip_gravity(flipped, 0.5);
                            self.sync_other_dual_gravity();
                            boosted_this_step = true;
                            self.boost_trail(true);
                        } else if orb_id == Some(444.0) || orb_id == Some(3004.0) {
                            let ps = if self.p.is_mini { 18.0 } else { 30.0 };
                            if !self.spider_teleport(ps, cols) { continue; }
                            self.vlog.push(format!("grav:{}", self.p.gravity_flipped as u8));
                            self.sync_other_dual_gravity();
                            self.run_rotate_action(1.0);
                            boosted_this_step = true;
                            teleported_this_step = true;
                            best_slope_candidate = None;
                            self.mark_orb(cols[i].x, cols[i].y);
                            break;
                        } else if self.p.is_wave {
                            if orb_id == Some(84.0) || orb_id == Some(1022.0) {
                                let flipped = !self.p.gravity_flipped;
                                self.flip_gravity(flipped, 0.5);
                                self.sync_other_dual_gravity();
                                boosted_this_step = true;
                                self.boost_trail(true);
                                self.mark_orb(cols[i].x, cols[i].y);
                            }
                        } else {
                            let y_s = crate::physics::grav_consts_pub(self.g.player_speed).1;
                            let o_mini = if self.p.is_mini { 0.8 } else { 1.0 };
                            let (o_ship, o_ufo, o_ball, o_robot, o_spider) = (self.p.is_flying || self.p.is_swing, self.p.is_ufo, self.p.is_ball, self.p.is_robot, self.p.is_spider);
                            if orb_id == Some(1330.0) {
                                orb_vel = if o_ship { -28.0 } else if o_ufo { -22.4 } else if o_spider { -33.0 } else { -30.0 };
                                self.p.gd_accel = true;
                            } else {
                                let base;
                                if orb_id == Some(141.0) {
                                    base = if o_ship { 0.37 } else if o_ufo { 0.42 } else if o_ball { 0.77 } else { 0.72 };
                                } else if orb_id == Some(1333.0) {
                                    base = if o_ship { if self.p.is_mini { 1.4 } else { 1.0 } }
                                        else if o_ufo { if self.p.is_mini { 1.36 } else { 1.02 } }
                                        else if o_ball { 1.34 } else if o_robot { 1.28 } else if o_spider { 1.34 } else { 1.38 };
                                    self.p.gd_accel = true;
                                } else if orb_id == Some(84.0) {
                                    base = 0.8;
                                    flip_after = true;
                                } else if orb_id == Some(1022.0) {
                                    base = if o_ship { 0.7 } else { 1.0 };
                                    flip_before = true;
                                } else {
                                    base = if o_robot { 0.9 } else { 1.0 };
                                }
                                let mut base = base;
                                if self.p.is_swing {
                                    if orb_id == Some(36.0) { base = 0.6; }
                                    else if orb_id == Some(141.0) { base = 0.432; }
                                    else if orb_id == Some(1333.0) { base = 0.828; }
                                    else if orb_id == Some(84.0) { base = 0.24; }
                                    else if orb_id == Some(1022.0) { base = 0.6; }
                                }
                                orb_vel = y_s * base * o_mini;
                                if o_ball || o_spider { orb_vel *= 0.7; }
                            }
                            if self.p.is_swing { self.p.gd_accel = true; }
                            self.p.is_jumping = true;
                            self.p.gd_land_flag = false;
                            self.p.on_ground = false;
                            self.p.can_jump = false;
                            if flip_before {
                                let flipped = !self.p.gravity_flipped;
                                self.flip_gravity(flipped, 0.5);
                                self.sync_other_dual_gravity();
                                self.p.y_velocity = crate::physics::gd_trunc(self.flip_mod() * orb_vel);
                            } else {
                                self.p.y_velocity = crate::physics::gd_trunc(fm * orb_vel);
                            }
                            if orb_id == Some(1330.0) { self.p.was_boosted = false; }
                            if self.p.is_ball {
                                let dir = if self.p.gravity_flipped { -1.0 } else { 1.0 };
                                self.p.ball_normal_rotate = if self.p.mirrored { -dir } else { dir };
                                self.p.ball_should_rotate = true;
                                self.p.ball_rotate_opposite = true;
                                self.p.ball_rotation2 = 0.0;
                            }
                            self.run_rotate_action(1.0);
                            boosted_this_step = true;
                            self.boost_trail(true);
                            self.mark_orb(cols[i].x, cols[i].y);
                            if flip_after {
                                let flipped = !self.p.gravity_flipped;
                                self.flip_gravity(flipped, if self.p.is_swing { 1.0 } else { 0.5 });
                                self.sync_other_dual_gravity();
                            }
                        }
                        let _ = (flip_before, flip_after, orb_vel);
                    }
                } else if is_dash && !self.p.up_key_down {
                    self.set_dash_hold_ticks(&mut cols[i], 0.0);
                }
            } else if ty == T_COIN {
                if self.g.no_clip { continue; }
                if !self.is_activated(&cols[i]) {
                    if cols[i].secret_coin && self.e.practice_mode { continue; }
                    if cols[i].user_coin && self.e.practice_mode { continue; }
                    if !self.coin_gate(&cols[i]) { continue; }
                    self.set_activated(&mut cols[i], true);
                    if let Some(c) = self.coins.as_deref_mut() { c.pickups.push(cols[i].uid as usize); }
                    self.vlog.push(Self::fx_line(&cols[i]));
                }
            } else { match ty {
                t if t == T_SLOPE => {
                    let res = self.handle_slope_collision(&cols[i], piece_width, players_y, players_last_y, player_size, wave_hit, gamemode_addition, cols);
                    if res.died {
                        if Self::ticking(self.p.safe_head_ticks) { continue; }
                        if self.g.no_clip { self.p.died_this_frame = true; continue; }
                        self.kill_player();
                        return;
                    }
                    let inner_half = if self.p.is_wave { wave_hit / 3.0 } else { 9.0 };
                    let lowest_endpoint = cols[i].y + cols[i].hypo_ay.min(cols[i].hypo_by);
                    let snap_y = res.push.as_ref().map(|p| p.y).or(res.candidate.as_ref().map(|c| c.y));
                    if cols[i].slope_solid_below && snap_y.map_or(false, |sy| sy > players_y) && players_y + inner_half <= lowest_endpoint { continue; }
                    if let Some(push) = res.push {
                        let away = if self.p.gravity_flipped { -1.0 } else { 1.0 };
                        self.p.y = push.y;
                        if push.h_block {
                            self.bonk_head(push.y);
                        } else if let Some(tan) = push.ship_head_tangent.filter(|v| v.is_finite()) {
                            self.p.y_velocity = tan;
                            self.p.ship_head_uid = Some(cols[i].uid);
                            self.p.ship_head_flipped = self.p.gravity_flipped;
                            self.p.skip_slope_exit = true;
                        } else if push.grace || push.fall {
                            if self.p.y_velocity * away > -4.0 { self.p.y_velocity = -4.0 * away; }
                        } else if push.head != Some(false) && self.p.y_velocity * away > 0.0 {
                            self.p.y_velocity = 0.0;
                        }
                        self.p.on_ground = false;
                        continue;
                    }
                    if res.landed {
                        if let Some(cand) = res.candidate {
                            let is_better = match &best_slope_candidate {
                                None => true,
                                Some((b, _)) => if prefer_highest { cand.y > b.y } else { cand.y < b.y },
                            };
                            if is_better { best_slope_candidate = Some((cand, cols[i].uid)); }
                        }
                    }
                    continue;
                }
                t if t == T_HAZARD => {
                    if self.g.no_clip { self.p.died_this_frame = true; continue; }
                    if let Some(r) = circle {
                        let (dx, dy) = (piece_width - x, players_y - y);
                        let dist_sq = dx * dx + dy * dy;
                        let min_dist = r + if self.p.is_wave { wave_hit } else { player_size };
                        if dist_sq > min_dist * min_dist { continue; }
                    }
                    if self.breakable(&cols[i]) { continue; }
                    self.kill_player();
                    return;
                }
                t if t == T_SOLID => {
                    left = rl; right = rr; top = rt; bottom = rb;
                    let solid_size = if self.p.is_wave { wave_hit } else { player_size };
                    let a146 = players_y - solid_size + gamemode_addition;
                    let a869 = players_last_y - solid_size + gamemode_addition;
                    let a3e7 = players_y + solid_size - gamemode_addition;
                    let a135 = players_last_y + solid_size - gamemode_addition;
                    let tight = if self.p.is_wave { wave_hit / 3.0 } else { player_size * 0.3 };
                    let colliding;
                    if let Some(r) = circle {
                        let (dx, dy) = (piece_width - x, players_y - y);
                        let dsq = dx * dx + dy * dy;
                        let tr = r + tight;
                        colliding = dsq <= tr * tr;
                        left = x - r; right = x + r; top = y - r; bottom = y + r;
                    } else {
                        colliding = piece_width + tight > left && piece_width - tight < right && players_y + tight > top && players_y - tight < bottom;
                    }
                    if self.p.is_wave && !self.p.wave_slide_active {
                        if !colliding { continue; }
                        if self.g.no_clip { self.p.died_this_frame = true; continue; }
                        if self.breakable(&cols[i]) { continue; }
                        self.kill_player();
                        return;
                    }
                    let land_bot_flag = (self.p.y_velocity <= 0.0 || self.p.on_ground) && (a146 >= bottom || a869 >= bottom);
                    let land_top_flag = (self.p.y_velocity >= 0.0 || self.p.on_ground) && (a3e7 <= top || a135 <= top);
                    let standing = if self.p.gravity_flipped { land_top_flag } else { land_bot_flag };
                    let slope_lead_in = self.p.slope_riding && (if self.p.gravity_flipped { a3e7 <= top + solid_size } else { a146 >= bottom - solid_size });
                    let s_dist = (players_y - players_last_y).abs();
                    let wave_tol = if self.p.is_wave { solid_size - tight } else { 0.0 };
                    let normal_land_bot = a146 >= bottom && a146 - bottom <= s_dist + gamemode_addition;
                    let coyote_bot = self.p.on_ground && !self.p.slope_riding && self.p.slope_exit_grace == 0.0 && a146 >= bottom && a146 - bottom < solid_size + gamemode_addition;
                    let skip_bot = a146 < bottom && a869 >= bottom;
                    let wave_land_bot = self.p.is_wave && (self.p.y_velocity <= 0.0 || self.p.on_ground) && a146 < bottom && bottom - a146 <= wave_tol;
                    let land_bot = normal_land_bot || coyote_bot || skip_bot || wave_land_bot;
                    let normal_land_top = a3e7 <= top && top - a3e7 <= s_dist + gamemode_addition;
                    let coyote_top = self.p.on_ground && !self.p.slope_riding && self.p.slope_exit_grace == 0.0 && a3e7 <= top && top - a3e7 < solid_size + gamemode_addition;
                    let skip_top = a3e7 > top && a135 <= top;
                    let wave_land_top = self.p.is_wave && (self.p.y_velocity >= 0.0 || self.p.on_ground) && a3e7 > top && a3e7 - top <= wave_tol;
                    let land_top = normal_land_top || coyote_top || skip_top || wave_land_top;
                    let safe_on_surface = if self.p.is_wave { land_bot || land_top } else { standing };
                    if (Self::ticking(self.p.safe_head_ticks) || Self::ticking(self.p.flip_head_ticks)) && !self.p.is_flying && !self.p.is_wave && !self.p.is_ufo && !safe_on_surface
                        && piece_width + solid_size - 5.0 > left && piece_width - solid_size + 5.0 < right {
                        let mut hit_head = false;
                        if !self.p.gravity_flipped && land_top && self.p.y_velocity > 0.0 {
                            self.p.y = top - solid_size;
                            self.p.y_velocity = 0.0;
                            hit_head = true;
                        } else if self.p.gravity_flipped && land_bot && self.p.y_velocity < 0.0 {
                            self.p.y = bottom + solid_size;
                            self.p.y_velocity = 0.0;
                            hit_head = true;
                        }
                        if hit_head {
                            if Self::ticking(self.p.flip_head_ticks) {
                                let flipped = !self.p.gravity_flipped;
                                self.flip_gravity(flipped, 1.0);
                                self.p.y_velocity = if self.p.gravity_flipped { 4.0 } else { -4.0 };
                                if Self::ticking(self.p.j_block_ticks) { self.release_jump_hold(); }
                            }
                            continue;
                        }
                    }
                    let head_y = if self.p.gravity_flipped { bottom } else { top };
                    let head_crossing = if self.p.gravity_flipped {
                        self.p.y_velocity < 0.0 && players_last_y - solid_size >= bottom - 2.0 && players_y - solid_size <= bottom
                    } else {
                        self.p.y_velocity > 0.0 && players_last_y + solid_size <= top + 2.0 && players_y + solid_size >= top
                    };
                    if head_crossing && piece_width + solid_size - 5.0 > left && piece_width - solid_size + 5.0 < right
                        && self.head_block_covers(piece_width, head_y, left, right, solid_size, cols) {
                        self.bonk_head(head_y + if self.p.gravity_flipped { solid_size } else { -solid_size });
                        continue;
                    }
                    if colliding && !safe_on_surface && !slope_lead_in {
                        if self.g.no_clip { self.p.died_this_frame = true; continue; }
                        if self.breakable(&cols[i]) { continue; }
                        self.kill_player();
                        return;
                    }
                    if piece_width + solid_size - 5.0 > left && piece_width - solid_size + 5.0 < right {
                        if !self.p.gravity_flipped && land_bot && (self.p.y_velocity <= 0.0 || self.p.on_ground) {
                            self.p.y = bottom + solid_size;
                            self.hit_ground(false);
                            grounded = true;
                            self.p.collide_bottom = bottom;
                            if !self.p.is_flying { let c = cols[i].clone(); self.check_snap_jump(&c); }
                            continue;
                        }
                        if self.p.gravity_flipped && !self.p.is_flying && land_top && (self.p.y_velocity >= 0.0 || self.p.on_ground) {
                            self.p.y = top - solid_size;
                            self.hit_ground(false);
                            grounded = true;
                            self.p.on_ceiling = true;
                            self.p.collide_top = top;
                            if !self.p.is_flying { let c = cols[i].clone(); self.check_snap_jump(&c); }
                            continue;
                        }
                        if self.p.is_ufo {
                            if !self.p.gravity_flipped && land_top && (self.p.y_velocity >= 0.0 || self.p.on_ground) {
                                self.p.y = top - solid_size;
                                self.hit_ground(false);
                                self.p.on_ceiling = true;
                                self.p.collide_top = top;
                                continue;
                            }
                            if self.p.gravity_flipped && land_bot && (self.p.y_velocity <= 0.0 || self.p.on_ground) {
                                self.p.y = bottom + solid_size;
                                self.hit_ground(false);
                                grounded = true;
                                self.p.on_ceiling = true;
                                self.p.collide_top = bottom;
                                continue;
                            }
                            continue;
                        }
                        if land_top && (self.p.y_velocity >= 0.0 || self.p.on_ground) && (self.p.is_flying || self.p.is_wave || self.p.is_swing) {
                            self.p.y = top - solid_size;
                            self.hit_ground(false);
                            self.p.on_ceiling = true;
                            self.p.collide_top = top;
                            continue;
                        }
                        if !self.p.gravity_flipped && (a3e7 <= top || a135 <= top) && self.p.y_velocity >= 0.0 {
                            if colliding {
                                if self.breakable(&cols[i]) { continue; }
                                if self.g.no_clip { self.p.died_this_frame = true; } else { self.kill_player(); return; }
                            }
                            continue;
                        }
                        if self.p.gravity_flipped && land_bot && (self.p.y_velocity <= 0.0 || self.p.on_ground) && (self.p.is_flying || self.p.is_wave || self.p.is_swing) {
                            self.p.y = bottom + solid_size;
                            self.hit_ground(false);
                            grounded = true;
                            self.p.on_ceiling = true;
                            self.p.collide_top = bottom;
                            continue;
                        }
                    }
                }
                t if t == T_HBLOCK => { self.p.safe_head_ticks = Some(2.0); }
                t if t == T_FBLOCK => { self.p.flip_head_ticks = Some(2.0); }
                t if t == T_JBLOCK => { self.p.j_block_ticks = Some(2.0); }
                t if t == T_FORCEBLOCK => { let c = cols[i].clone(); self.add_block_force(&c, piece_width, players_y); }
                t if t == T_SBLOCK => {
                    if self.p.is_dashing { self.p.is_dashing = false; self.p.dash_y_velocity = 0.0; }
                }
                _ => {}
            } }
        }
        if best_slope_candidate.is_some() && boosted_this_step && self.g.slope_boost_skip_snap {
            self.p.skip_slope_exit = true;
            best_slope_candidate = None;
        }
        if let Some((best, best_uid)) = best_slope_candidate.take() {
            let was_jumping_slope = self.p.is_jumping;
            self.p.y = best.y;
            if best.no_ground {
                let away = if self.p.gravity_flipped { -1.0 } else { 1.0 };
                if self.p.y_velocity * away < 4.0 { self.p.y_velocity = 4.0 * away; }
                self.p.on_ground = false;
            } else {
                self.hit_ground(true);
                if let Some(v) = best.keep_y_velocity { self.p.y_velocity = v; }
            }
            if let Some(gd) = &best.gd {
                self.p.gd_is_on_slope = true;
                self.p.gd_last_slope_obj = Some(gd.obj);
                self.p.gd_last_slope_top = Some(gd.is_floor_top);
                self.p.gd_last_slope_y_pos = Some(gd.target);
                self.p.gd_last_slope_steepness = gd.steepness;
                self.p.gd_last_slope_up = Some(gd.up);
                self.p.gd_slope_rotation = gd.rotation;
            }
            self.p.on_ceiling = best.on_ceiling;
            if let Some(v) = best.collide_bottom { self.p.collide_bottom = v; }
            if let Some(v) = best.collide_top { self.p.collide_top = v; }
            self.p.on_slope_angle = Some(-crate::pslope::fold_slope_angle(best.slope_angle));
            if self.p.is_wave { self.p.rotation = self.p.on_slope_angle.unwrap(); }
            if self.p.prev_slope_obj != Some(best_uid) { self.p.slope_entry_x = Some(piece_width); }
            self.p.active_slope_obj = Some(best_uid);
            self.p.active_slope_angle = Some(best.slope_angle);
            grounded = true;
            if best.blocked {
                slope_blocked_this_step = true;
            } else if !was_jumping_slope {
                snapped_slope_this_frame = true;
                slope_tangent_this_frame = best.tangent;
            }
            if !self.p.is_flying && !self.p.is_wave {
                if let Some(c) = cols.iter().find(|c| c.uid == best_uid).cloned() { self.check_snap_jump(&c); }
            }
        }
        if self.p.collide_top != 0.0 && self.p.collide_bottom != 0.0 && (self.p.collide_top - self.p.collide_bottom).abs() < 48.0 {
            if self.g.no_clip { self.p.died_this_frame = true; }
            if !self.g.no_clip { self.kill_player(); return; }
        }
        let floor_y = self.e.floor_y;
        let is_cube = !self.p.is_flying && !self.p.is_ball && !self.p.is_wave && !self.p.is_ufo && !self.p.is_spider && !self.p.is_swing;
        let eff = if self.p.is_wave { wave_hit } else { player_size };
        if !grounded && !boosted_this_step {
            let grav_ceil = self.e.ceiling_y;
            if self.p.y <= floor_y + eff {
                if !self.p.gravity_flipped || !is_cube {
                    self.p.y = floor_y + eff;
                    self.hit_ground(false);
                    if self.p.gravity_flipped { self.p.on_ceiling = true; }
                } else if self.p.gravity_flipped && is_cube && self.p.y_velocity < -0.5 {
                    if self.g.no_clip { self.p.died_this_frame = true; } else { self.kill_player(); return; }
                }
            }
            if let Some(c) = grav_ceil {
                if self.p.y >= c - eff && self.p.gravity_flipped {
                    self.p.y = c - eff;
                    self.hit_ground(false);
                    self.p.on_ceiling = true;
                }
            }
            if !self.p.gravity_flipped && !self.g.no_clip && self.p.y < floor_y - 30.0 {
                self.p.y = floor_y + eff;
                self.p.y_velocity = 0.0;
                self.hit_ground(false);
            }
            if self.p.gravity_flipped {
                if let Some(c) = self.e.ceiling_y {
                    if self.p.y >= c - eff {
                        self.p.y = c - eff;
                        self.hit_ground(false);
                        self.p.on_ceiling = true;
                    }
                    if !self.g.no_clip && self.p.y > c + 30.0 {
                        self.p.y = c - eff;
                        self.p.y_velocity = 0.0;
                        self.hit_ground(false);
                        self.p.on_ceiling = true;
                    }
                }
            }
        }
        let ceil2 = self.e.ceiling_y;
        if !teleported_this_step {
            if let Some(c) = ceil2 {
                if self.p.y >= c - eff {
                    if self.p.gravity_flipped || !is_cube {
                        self.p.y = c - eff;
                        self.hit_ground(false);
                        self.p.on_ceiling = true;
                    } else if self.p.y_velocity > 0.5 || self.p.y > c - eff {
                        if self.g.no_clip { self.p.died_this_frame = true; } else { self.kill_player(); return; }
                    }
                }
            }
        }
        if (if self.e.in_sideways_step { self.e.player_world_x } else { self.p.y }) > 1890.0 * 4.0 {
            if self.g.no_clip { self.p.died_this_frame = true; } else { self.kill_player(); return; }
        }
        if self.p.ignore_teleport_until_clear && !touching_teleport_during_ignore { self.p.ignore_teleport_until_clear = false; }
        if snapped_slope_this_frame {
            self.p.slope_contact_frames = if self.p.slope_riding { self.p.slope_contact_frames + 1.0 } else { 0.0 };
            self.p.slope_riding = true;
            self.p.slope_exit_vel = Some(slope_tangent_this_frame);
            self.p.slope_exit_grace = 20.0;
        } else if self.p.slope_riding {
            self.p.slope_riding = false;
            if !slope_blocked_this_step { self.apply_slope_exit_velocity(); }
            self.p.slope_exit_vel = None;
        }
        self.p.skip_slope_exit = false;
        if self.p.slope_exit_grace > 0.0 && !self.p.slope_riding { self.p.slope_exit_grace -= 1.0; }
        if self.p.is_flying || self.p.is_wave || self.p.is_ufo || self.p.is_spider || self.p.is_swing {
            let at_floor = self.p.y <= floor_y + eff;
            let at_ceiling = ceil2.map_or(false, |c| self.p.y >= c - eff);
            if !grounded && !at_floor && self.p.collide_top == 0.0 && !at_ceiling { self.p.on_ground = false; }
        }
        self.p.prev_slope_obj = self.p.active_slope_obj;
        self.p.prev_slope_angle = self.p.active_slope_angle.unwrap_or(f64::NAN);
        self.p.last_collision_world_x = Some(piece_width);
        self.p.last_collision_world_y = Some(self.p.y);
        self.p.was_up_key_down = self.p.up_key_down;
        if self.p.died_this_frame && self.g.no_clip_accuracy {
            self.p.nc_death_frames += 1.0;
            if !self.p.died_last_frame { self.p.nc_deaths += 1.0; }
        }
        if self.p.nc_total_frames > 0.0 {
            let safe = self.p.nc_total_frames - self.p.nc_death_frames;
            self.p.nc_accuracy = (safe / self.p.nc_total_frames) * 100.0;
        }
        self.p.died_last_frame = self.p.died_this_frame;
    }

    fn uses_independent_activation(c: &Col) -> bool { is_portal(c.ty) || c.ty == T_SPEED || c.ty == T_RING }
    pub fn is_activated(&self, c: &Col) -> bool {
        if !Self::uses_independent_activation(c) { return c.activated; }
        if self.e.second_player { c.act_dual } else { c.act_main }
    }
    pub fn set_activated(&mut self, c: &mut Col, on: bool) {
        if !Self::uses_independent_activation(c) { c.activated = on; return; }
        if self.e.second_player { c.act_dual = on; } else { c.act_main = on; }
        c.activated = c.act_main || c.act_dual;
    }
    fn dash_hold_ticks(&self, c: &Col) -> f64 { if self.e.second_player { c.dash_hold_dual } else { c.dash_hold_main } }
    fn set_dash_hold_ticks(&mut self, c: &mut Col, t: f64) { if self.e.second_player { c.dash_hold_dual = t; } else { c.dash_hold_main = t; } }
    fn consume_orb_activation_input(&mut self) {
        self.p.orb_activation_consumed_for_press = true;
        self.p.up_key_pressed = false;
        self.p.queued_hold = false;
    }

    fn rotated_rectangles_overlap(cx1: f64, cy1: f64, hw1: f64, hh1: f64, rot1: f64, cx2: f64, cy2: f64, hw2: f64, hh2: f64, rot2: f64) -> bool {
        let make = |cx: f64, cy: f64, hw: f64, hh: f64, rot: f64| {
            let rad = or_zero(rot) * std::f64::consts::PI / 180.0;
            let (cos, sin) = (m::cos(rad), m::sin(rad));
            (cx, cy, hw, hh, (cos, sin), (-sin, cos))
        };
        let a = make(cx1, cy1, hw1, hh1, rot1);
        let b = make(cx2, cy2, hw2, hh2, rot2);
        let axes = [a.4, a.5, b.4, b.5];
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        for axis in axes {
            let center_distance = (dx * axis.0 + dy * axis.1).abs();
            let ra = a.2 * (axis.0 * a.4 .0 + axis.1 * a.4 .1).abs() + a.3 * (axis.0 * a.5 .0 + axis.1 * a.5 .1).abs();
            let rb = b.2 * (axis.0 * b.4 .0 + axis.1 * b.4 .1).abs() + b.3 * (axis.0 * b.5 .0 + axis.1 * b.5 .1).abs();
            if center_distance >= ra + rb { return false; }
        }
        true
    }

    /// `_isPlayerTouchingPortalHitbox`. `prev = None` mirrors the default `null` arguments (which become 0, 0).
    pub(crate) fn touching_portal(&self, c: &Col, px: f64, py: f64, half: f64, prev: Option<(f64, f64)>) -> bool {
        let portal_half_w = or_zero(c.w) / 2.0;
        let portal_half_h = or_zero(c.h) / 2.0;
        let player_half = or_zero(half).max(0.0);
        if portal_half_w <= 0.0 || portal_half_h <= 0.0 || player_half <= 0.0 { return false; }
        let (pcx, pcy) = (or_zero(c.x), or_zero(c.y));
        let portal_rotation = -or_zero(c.rotation_degrees);
        let touches_at = |wx: f64, wy: f64| Self::rotated_rectangles_overlap(pcx, pcy, portal_half_w, portal_half_h, portal_rotation, or_zero(wx), or_zero(wy), player_half, player_half, 0.0);
        if touches_at(px, py) { return true; }
        let (prev_x, prev_y) = prev.unwrap_or((0.0, 0.0));
        let (cur_x, cur_y) = (px, py);
        if !prev_x.is_finite() || !prev_y.is_finite() || !cur_x.is_finite() || !cur_y.is_finite() { return false; }
        let (dx, dy) = (cur_x - prev_x, cur_y - prev_y);
        let distance = dx.abs().max(dy.abs());
        if distance <= 0.0 { return false; }
        let rad = portal_rotation * std::f64::consts::PI / 180.0;
        let (cos, sin) = (m::cos(rad), m::sin(rad));
        let to_local = |wx: f64, wy: f64| { let (lx, ly) = (wx - pcx, wy - pcy); (lx * cos + ly * sin, lx * -sin + ly * cos) };
        let start = to_local(prev_x, prev_y);
        let end = to_local(cur_x, cur_y);
        let (seg_dx, seg_dy) = (end.0 - start.0, end.1 - start.1);
        let (ehw, ehh) = (portal_half_w + player_half, portal_half_h + player_half);
        let (mut t_min, mut t_max) = (0.0f64, 1.0f64);
        let mut clip = |sv: f64, dv: f64, minv: f64, maxv: f64| -> bool {
            if dv.abs() < 1e-8 { return sv >= minv && sv <= maxv; }
            let mut t1 = (minv - sv) / dv;
            let mut t2 = (maxv - sv) / dv;
            if t1 > t2 { std::mem::swap(&mut t1, &mut t2); }
            t_min = t_min.max(t1);
            t_max = t_max.min(t2);
            t_min <= t_max
        };
        if clip(start.0, seg_dx, -ehw, ehw) && clip(start.1, seg_dy, -ehh, ehh) { return true; }
        let steps = 12f64.min(2f64.max((distance / 1f64.max(player_half)).ceil()));
        let mut i = 1.0;
        while i < steps {
            let t = i / steps;
            if touches_at(prev_x + dx * t, prev_y + dy * t) { return true; }
            i += 1.0;
        }
        false
    }

    /// `breakabletheblock`: only object 143 can break, and only when the level layer says so.
    fn breakable(&self, c: &Col) -> bool {
        match c.objid { Some(v) if v == 143.0 => self.e.break_block, _ => false }
    }
}

impl Col {
    /// The player-side view of a freshly spawned `Collider` (what the test harness' `colRow` reads from the JS object).
    /// Saved coin progress (`secret_completed`/`user_completed`) is not part of this.
    pub fn from_collider(c: &crate::collider::Collider, uid: f64) -> Col {
        let f = |v: Option<i64>| v.map(|n| n as f64);
        Col {
            ty: type_code(&c.ty), x: c.x, y: c.y, w: c.w, h: c.h, rotation_degrees: c.rotation_degrees,
            hitbox_radius: c.hitbox_radius, act_main: false, act_dual: false, activated: c.activated,
            objid: f(c.objid), uid,
            id: f(c.pad_id).or(f(c.orb_id)).or(f(c.speed_id)).or(f(c.coin_id)),
            multi_activate: c.multi_activate, free_mode: c.free_mode, flip_y: c.flip_y, orb_rotation: c.orb_rotation.unwrap_or(0.0), speed_value: c.speed_value,
            teleport_target_x: c.teleport_target_x, teleport_target_y: c.teleport_target_y, portal_x: c.portal_x, portal_y: c.portal_y,
            dash_hold_main: 0.0, dash_hold_dual: 0.0, slope_angle_deg: c.slope_angle_deg, slope_dir: c.slope_dir,
            slope_is_filled: c.slope_is_filled, slope_flip_y: c.slope_flip_y,
            hypo_ax: c.hypo_a.0, hypo_ay: c.hypo_a.1, hypo_bx: c.hypo_b.0, hypo_by: c.hypo_b.1, right_ax: c.right_a.0, right_ay: c.right_a.1,
            slope_solid_below: c.slope_solid_below, slope_type: f(c.slope_type), gd_solid_above: c.gd_solid_above, gd_wall_on_left: c.gd_wall_on_left,
            secret_coin: c.coin_id == Some(142), user_coin: c.coin_id == Some(1329), secret_completed: false, user_completed: false,
            claim_touch: c.claim_touch, ee_toggle_press: None, force: c.force, force_min: c.force_min, force_max: c.force_max, force_radial: c.force_radial, force_range: c.force_range, force_id: c.force_id as f64, force_flip_y: c.force_flip_y,
        }
    }
}

impl<'a> Sim<'a> {
    /// `_gameLayer.setPortalFreeMode(enabled)`
    fn set_portal_free_mode(&mut self, enabled: bool) {
        self.e.portal_free_mode = enabled;
        if enabled && (self.e.fly_ground_active || self.e.fly_ceiling_y.is_some() || self.e.fly_camera_target.is_some()) {
            self.level_fly(false, 0.0, None, None);
        }
    }

    /// `_isHoldingShipHeadSlope`
    pub(crate) fn is_holding_ship_head_slope(&self, c: &Col) -> bool {
        self.p.is_flying && self.p.up_key_down && !self.p.is_dashing && self.p.gravity_flipped == c.slope_solid_below && self.g.gd_cube_slopes
    }
    /// `_wasHoldingShipHeadSlope`
    pub(crate) fn was_holding_ship_head_slope(&self, c: &Col, piece_width: f64) -> bool {
        self.is_holding_ship_head_slope(c) && self.p.prev_ship_head_uid == Some(c.uid)
            && self.p.prev_ship_head_flipped == self.p.gravity_flipped && c.slope_surface_y(piece_width).is_some()
    }

    /// `_headBlockCovers`: an h-block ("head block") under the spot where the head hits a ceiling piece
    pub(crate) fn head_block_covers(&self, world_x: f64, face_y: f64, left: f64, right: f64, half_size: f64, objects: &[Col]) -> bool {
        let p = &*self.p;
        if p.is_flying || p.is_ball || p.is_wave || p.is_ufo || p.is_swing { return false; }
        objects.iter().any(|o| {
            if o.ty != T_HBLOCK { return false; }
            let rad = or_zero(o.rotation_degrees) * std::f64::consts::PI / 180.0;
            let hw = (o.w / 2.0 * m::cos(rad)).abs() + (o.h / 2.0 * m::sin(rad)).abs();
            let hh = (o.w / 2.0 * m::sin(rad)).abs() + (o.h / 2.0 * m::cos(rad)).abs();
            let overlap_left = left.max(o.x - hw);
            let overlap_right = right.min(o.x + hw);
            overlap_left < overlap_right && world_x + half_size > overlap_left && world_x - half_size < overlap_right
                && face_y >= o.y - hh - 1.0 && face_y <= o.y + hh + 1.0
        })
    }

    /// `_bonkHead`
    pub(crate) fn bonk_head(&mut self, y: f64) {
        self.p.y = y;
        self.p.y_velocity = 0.0;
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.p.robot_hold = false;
        self.p.robot_hold_timer = 0.0;
    }

    /// `_theresNothingThereTeleport`: a spider pad/orb with nothing above sends the player to the death line
    pub(crate) fn theres_nothing_there_teleport(&mut self, world_x: f64, player_size: f64, from_jump: bool, cols: &[Col]) -> bool {
        let death_y = 6000.0;
        let old_y = self.p.y;
        let hazard = self.find_spider_teleport_hazard(true, world_x, player_size, death_y, cols);
        let target_y = match hazard { Some(b) if !self.g.no_clip => (b.lower + b.upper) / 2.0, _ => death_y };
        self.flip_gravity(true, 1.0);
        self.p.y = target_y;
        self.p.last_y = target_y;
        self.p.y_velocity = 0.0;
        self.p.pending_velocity = None;
        self.p.on_ground = false;
        self.p.on_ceiling = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.p.up_key_pressed = false;
        self.p.queued_hold = false;
        self.p.skip_slope_exit = true;
        self.p.last_collision_world_y = Some(target_y);
        self.vlog.push(format!("tp:{},{}", old_y, target_y));
        if !self.g.no_clip {
            self.kill_player();
        } else {
            self.p.died_this_frame = true;
            if from_jump { self.p.spider_teleport_noclip_death_pending = true; }
        }
        false
    }

    /// `_addBlockForce`
    pub(crate) fn add_block_force(&mut self, obj: &Col, px: f64, py: f64) {
        if obj.force_id >= 1.0 {
            if self.force_ids.contains(&obj.force_id) { return; }
            self.force_ids.push(obj.force_id);
        }
        let radius = match obj.hitbox_radius { Some(r) if r > 0.0 => r, _ => obj.w.max(obj.h) / 2.0 };
        let mut force = obj.force;
        let ang;
        if obj.force_radial {
            ang = m::atan2(obj.y - py, obj.x - px) + if obj.force_flip_y { 0.0 } else { std::f64::consts::PI };
        } else {
            ang = (90.0 - or_zero(obj.rotation_degrees) + if obj.force_flip_y { 180.0 } else { 0.0 }) * std::f64::consts::PI / 180.0;
        }
        if obj.force_range {
            let t;
            if obj.force_radial {
                let dist = m::hypot2(obj.x - px, obj.y - py);
                t = if dist < radius { 1.0 - dist / radius } else { 0.0 };
            } else {
                t = ((radius + (obj.x - px) * m::cos(ang) + (obj.y - py) * m::sin(ang)) / radius) * 0.5;
            }
            let t = (0.0f64).max((1.0f64).min(t));
            force = obj.force_min + t * (obj.force_max - obj.force_min);
        }
        self.p.force_acc_x += force * m::cos(ang);
        self.p.force_acc_y += force * m::sin(ang);
        let len = m::hypot2(self.p.force_acc_x, self.p.force_acc_y);
        if len > 10000.0 {
            self.p.force_acc_x *= 10000.0 / len;
            self.p.force_acc_y *= 10000.0 / len;
        }
        self.p.force_ticks = Some(2.0);
    }
}

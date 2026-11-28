//! Slope collision, ported from player.js (`_handleSlopeCollision`, `_gdCubeSlopeCollision` and helpers).
//! Both the default "GD cube slopes" path and the legacy path (`window.gdCubeSlopes === false`) are ported.
use crate::jsmath as m;
use crate::pcol::Col;
use crate::physics::Sim;

#[derive(Debug, Clone)]
pub struct GdInfo { pub obj: f64, pub is_floor_top: bool, pub target: f64, pub steepness: f64, pub up: bool, pub rotation: f64 }

#[derive(Debug, Clone)]
pub struct Cand {
    pub y: f64,
    pub on_ceiling: bool,
    pub collide_bottom: Option<f64>,
    pub collide_top: Option<f64>,
    pub slope_angle: f64,
    pub blocked: bool,
    pub tangent: f64,
    pub keep_y_velocity: Option<f64>,
    pub no_ground: bool,
    pub gd: Option<GdInfo>,
}

#[derive(Debug, Clone)]
pub struct Push { pub y: f64, pub grace: bool, pub fall: bool, pub head: Option<bool>, pub h_block: bool, pub ship_head_tangent: Option<f64> }

#[derive(Debug, Clone, Default)]
pub struct SlopeResult { pub landed: bool, pub died: bool, pub push: Option<Push>, pub candidate: Option<Cand> }

impl SlopeResult { fn none() -> Self { SlopeResult::default() } }

fn or0(v: f64) -> f64 { if v == 0.0 || v.is_nan() { 0.0 } else { v } }

impl<'a> Sim<'a> {
    fn slope_back_wall_hit(&self, c: &Col, piece_width: f64, players_y: f64, surface_y: Option<f64>) -> bool {
        if !self.g.slope_back_wall { return false; }
        if c.slope_type.is_none() { return false; }
        if !c.gd_wall_on_left { return false; }
        let half_w = or0(c.w) / 2.0;
        let half_h = or0(c.h) / 2.0;
        if !(half_w > 0.0) || !(half_h > 0.0) { return false; }
        if let Some(sy) = surface_y {
            let inside_solid = if c.slope_solid_below { players_y < sy } else { players_y > sy };
            if !inside_solid { return false; }
        }
        let wall_w = 2.0;
        let wall_left = if c.gd_wall_on_left { c.x - half_w } else { c.x + half_w - wall_w };
        let wall_right = wall_left + wall_w;
        let box_bot = c.y - half_h;
        let box_top = c.y + half_h;
        let inner = if self.p.is_wave { if self.p.is_mini { 2.0 } else { 3.0 } } else if self.p.is_mini { 5.4 } else { 9.0 };
        (piece_width + inner > wall_left) && (piece_width - inner < wall_right) && (players_y + inner > box_bot) && (players_y - inner < box_top)
    }

    fn slope_flat_cap_land(&self, c: &Col, piece_width: f64, players_y: f64, players_last_y: f64, player_size: f64, gamemode_addition: f64) -> Option<SlopeResult> {
        if !self.g.slope_flat_cap { return None; }
        if c.slope_type.is_none() { return None; }
        if self.p.is_wave { return None; }
        let half_w = or0(c.w) / 2.0;
        let half_h = or0(c.h) / 2.0;
        if !(half_w > 0.0) || !(half_h > 0.0) { return None; }
        if piece_width + player_size <= c.x - half_w || piece_width - player_size >= c.x + half_w { return None; }
        let box_top = c.y + half_h;
        let box_bot = c.y - half_h;
        let cap_at_top = c.gd_solid_above;
        let g_flip = self.p.gravity_flipped;
        if cap_at_top == g_flip { return None; }
        let face = if cap_at_top { box_top } else { box_bot };
        let feet = if cap_at_top { players_y - player_size } else { players_y + player_size };
        let last_feet = if cap_at_top { players_last_y - player_size } else { players_last_y + player_size };
        let falling = if cap_at_top { self.p.y_velocity <= 0.0 } else { self.p.y_velocity >= 0.0 };
        if !falling && !self.p.on_ground { return None; }
        let crossed = if cap_at_top {
            feet <= face && feet + gamemode_addition >= face && (last_feet >= face || self.p.on_ground)
        } else {
            feet >= face && feet - gamemode_addition <= face && (last_feet <= face || self.p.on_ground)
        };
        if !crossed { return None; }
        Some(SlopeResult { landed: true, died: false, push: None, candidate: Some(Cand {
            y: if cap_at_top { face + player_size } else { face - player_size },
            on_ceiling: !cap_at_top,
            collide_bottom: if cap_at_top { Some(face) } else { None },
            collide_top: if cap_at_top { None } else { Some(face) },
            slope_angle: 0.0, blocked: true, tangent: 0.0, keep_y_velocity: None, no_ground: false, gd: None,
        }) })
    }

    fn gd_slope_grace(&self) -> bool {
        let tick = self.e.physics_tick;
        if tick.is_nan() { return false; }
        self.p.gd_mode_tick.map_or(false, |t| tick - t < 24.0) || self.p.gd_flip_tick.map_or(false, |t| tick - t < 24.0)
    }

    fn gd_slope_line_y(c: &Col, world_x: f64) -> Option<f64> {
        let dx = c.hypo_bx - c.hypo_ax;
        if dx.abs() < 0.01 { return None; }
        Some(c.y + c.hypo_ay + (world_x - c.x - c.hypo_ax) * (c.hypo_by - c.hypo_ay) / dx)
    }

    fn gd_cube_slope_collision(&self, c: &Col, piece_width: f64, players_y: f64, player_size: f64, nearby: &[Col]) -> SlopeResult {
        let upside_down = self.p.gravity_flipped;
        let half_w = or0(c.w) / 2.0;
        let half_h = or0(c.h) / 2.0;
        if !(half_w > 0.0) || !(half_h > 0.0) { return SlopeResult::none(); }
        let (min_x, max_x, min_y, max_y) = (c.x - half_w, c.x + half_w, c.y - half_h, c.y + half_h);
        let held_head_contact = self.was_holding_ship_head_slope(c, piece_width);
        if piece_width + player_size <= min_x || piece_width - player_size >= max_x
            || (!held_head_contact && (players_y + player_size <= min_y || players_y - player_size >= max_y)) {
            return SlopeResult::none();
        }
        let line_y = match Self::gd_slope_line_y(c, piece_width) { Some(v) => v, None => return SlopeResult::none() };
        let uphill = (c.hypo_by - c.hypo_ay) / (c.hypo_bx - c.hypo_ax) > 0.0;
        let is_floor_top = !c.slope_solid_below;
        let sliding_down = if upside_down { uphill } else { !uphill };
        let was_on_slope = self.p.gd_was_on_slope;
        let last_slope_up = self.p.gd_last_slope_up == Some(true);
        if was_on_slope && last_slope_up && sliding_down && self.p.gd_last_slope_top == Some(is_floor_top) && self.p.gd_last_slope_top == Some(upside_down) {
            return SlopeResult::none();
        }
        let steepness = c.h / c.w;
        if was_on_slope && last_slope_up && self.p.gd_last_slope_steepness > steepness {
            return SlopeResult::none();
        }
        let stick = if sliding_down { if was_on_slope { 8.0 } else { 2.0 } } else { 0.0 };
        let cos_rot = m::cos(or0(self.p.gd_slope_rotation));
        let ride_half = player_size / (if cos_rot == 0.0 || cos_rot.is_nan() { 1.0 } else { cos_rot });
        if !was_on_slope {
            if !held_head_contact && !(players_y + player_size > min_y + 2.0 && players_y - player_size < max_y - 2.0) { return SlopeResult::none(); }
        } else if if !upside_down { max_y < players_y - ride_half - stick } else { min_y > players_y + ride_half + stick } {
            return SlopeResult::none();
        }
        let slope_angle = m::atan(steepness);
        let offset = player_size / m::cos(slope_angle);
        let slope_intersection = was_on_slope && self.p.gd_last_slope_obj != Some(c.uid) && self.p.gd_last_slope_top != Some(is_floor_top);
        let tolerance = if slope_intersection { 40.0 * if self.p.is_mini { 0.6 } else { 1.0 } } else { 0.0 };
        let mut target;
        if !is_floor_top {
            let max_pos = player_size + max_y - tolerance;
            target = line_y + offset - tolerance;
            if target >= max_pos { target = max_pos; }
            if target <= min_y { target = min_y; }
        } else {
            let min_pos = min_y - player_size + tolerance;
            target = line_y - offset + tolerance;
            if target <= min_pos { target = min_pos; }
            if target >= max_y { target = max_y; }
        }
        let (ship, ufo, wave) = (self.p.is_flying, self.p.is_ufo, self.p.is_wave);
        let flyer = ship || ufo || wave || self.p.is_swing;
        let holding = self.p.up_key_down;
        if upside_down != is_floor_top && flyer {
            if ship && piece_width > max_x { return SlopeResult::none(); }
            let held_ship_head = self.is_holding_ship_head_slope(c) && c.slope_surface_y(piece_width).is_some();
            let tangent = if held_ship_head { Some(m::tan(c.slope_angle_rad()) * self.g.player_speed) } else { None };
            let head_stick = if ship { 0.0 } else if slope_intersection || sliding_down || !holding { 0.0 } else if was_on_slope { 4.0 } else { 2.0 };
            let into = if upside_down { players_y < target + head_stick } else { players_y > target - head_stick };
            if !into { return SlopeResult::none(); }
            if slope_intersection || wave { return SlopeResult { landed: false, died: true, ..Default::default() }; }
            let fall = !holding && sliding_down;
            return SlopeResult { landed: false, died: false, push: Some(Push { y: target, grace: false, fall, head: Some(!(fall && was_on_slope)), h_block: false, ship_head_tangent: tangent }), candidate: None };
        }
        if upside_down != is_floor_top {
            let touching = if upside_down { players_y < target } else { players_y > target };
            if !touching { return SlopeResult::none(); }
            let moving_away = if is_floor_top { self.p.y_velocity < 0.0 } else { self.p.y_velocity > 0.0 };
            if moving_away { return SlopeResult::none(); }
            if self.p.is_ball {
                if slope_intersection { return SlopeResult { landed: false, died: true, ..Default::default() }; }
                return SlopeResult { landed: false, died: false, push: Some(Push { y: target, grace: false, fall: false, head: None, h_block: false, ship_head_tangent: None }), candidate: None };
            }
            let crash = if upside_down { players_y + 4.0 < target } else { players_y - 4.0 > target };
            if self.head_block_covers(piece_width, line_y, min_x, max_x, player_size, nearby) {
                return SlopeResult { landed: false, died: false, push: Some(Push { y: target, grace: false, fall: false, head: None, h_block: true, ship_head_tangent: None }), candidate: None };
            }
            if !crash { return SlopeResult::none(); }
            if self.gd_slope_grace() {
                return SlopeResult { landed: false, died: false, push: Some(Push { y: target, grace: true, fall: false, head: None, h_block: false, ship_head_tangent: None }), candidate: None };
            }
            return SlopeResult { landed: false, died: true, ..Default::default() };
        }
        let special = !slope_intersection && sliding_down && (if ship { !holding } else if ufo || wave || self.p.is_swing { true } else { !self.p.is_jumping });
        let touching = if upside_down { players_y > target } else { players_y < target };
        let mut within = !ship && special && (if upside_down { players_y > target - stick } else { players_y < target + stick });
        if within && !touching && ufo { within = if upside_down { self.p.y_velocity >= 0.0 } else { self.p.y_velocity <= 0.0 }; }
        let mut collide = touching || within;
        if wave && holding && sliding_down { collide = false; }
        if collide && was_on_slope {
            if let Some(last) = self.p.gd_last_slope_y_pos {
                if (last - target).abs() < 1e-6 { collide = false; }
            }
        }
        if !collide { return SlopeResult::none(); }
        let wd_angle = c.slope_angle_rad();
        let tan_speed = self.g.player_speed * self.g.slope_tangent_d;
        let surface_y = c.slope_surface_y(piece_width).unwrap_or(line_y);
        let rising_fast = if upside_down { self.p.y_velocity < -10.0 } else { self.p.y_velocity > 10.0 };
        SlopeResult { landed: true, died: false, push: None, candidate: Some(Cand {
            y: target,
            on_ceiling: upside_down,
            collide_bottom: if upside_down { None } else { Some(surface_y) },
            collide_top: if upside_down { Some(surface_y) } else { None },
            slope_angle: wd_angle,
            tangent: m::tan(wd_angle) * tan_speed,
            keep_y_velocity: if rising_fast { Some(self.p.y_velocity) } else { None },
            no_ground: flyer && holding && !sliding_down,
            blocked: false,
            gd: Some(GdInfo { obj: c.uid, is_floor_top, target, steepness, up: !sliding_down, rotation: slope_angle }),
        }) }
    }


    /// `_slopeReached` (`window.slopeReachGuard === false` skips the check)
    fn slope_reached(&self, p_edge: f64, surface_y: f64, gamemode_addition: f64, above: bool) -> bool {
        if !self.g.slope_reach_guard { return true; }
        if self.p.on_ground { return true; }
        if above { p_edge <= surface_y + gamemode_addition } else { p_edge >= surface_y - gamemode_addition }
    }

    fn legacy_land(&self, y: f64, on_ceiling: bool, bottom: Option<f64>, top: Option<f64>, angle: f64, blocked: bool, tangent: f64) -> SlopeResult {
        SlopeResult { landed: true, died: false, push: None, candidate: Some(Cand {
            y, on_ceiling, collide_bottom: bottom, collide_top: top, slope_angle: angle, blocked, tangent, keep_y_velocity: None, no_ground: false, gd: None,
        }) }
    }

    /// The `gdCubeSlopes === false` branch of `_handleSlopeCollision` (the wave case has already returned; `previousWorldX` is never passed).
    fn legacy_slope_collision(&self, c: &Col, piece_width: f64, players_y: f64, players_last_y: f64, player_size: f64, gamemode_addition: f64) -> SlopeResult {
        let surface_y = match c.slope_surface_y(piece_width) { Some(v) => v, None => return SlopeResult::none() };
        let last_surface_y = surface_y;
        if !self.p.is_flying && !self.p.is_swing && self.slope_back_wall_hit(c, piece_width, players_y, Some(surface_y)) {
            return SlopeResult { landed: false, died: true, ..Default::default() };
        }
        if let Some(cap) = self.slope_flat_cap_land(c, piece_width, players_y, players_last_y, player_size, gamemode_addition) { return cap; }
        let angle = c.slope_angle_rad();
        let is_ceil_slope = !c.slope_solid_below;
        let ride_y = |above: bool| if above { surface_y + player_size } else { surface_y - player_size };
        let p_low = players_y - player_size + gamemode_addition;
        let p_high = players_y + player_size - gamemode_addition;
        let p_last_low = players_last_y - player_size + gamemode_addition;
        let p_last_high = players_last_y + player_size - gamemode_addition;
        let tangent = m::tan(angle) * (self.g.player_speed * self.g.slope_tangent_d);
        let none = SlopeResult::none();

        if (self.p.is_flying && !self.p.is_ufo) || self.p.is_swing {
            let g_flip = self.p.gravity_flipped;
            let slope_tol = player_size;
            let acts_as_floor = (!is_ceil_slope && !g_flip) || (is_ceil_slope && g_flip);
            let snap_above = acts_as_floor != g_flip;
            let stick_rest = self.p.on_ground && !self.p.up_key_down;
            let stick_push = self.p.on_ground && self.p.up_key_down;
            if snap_above {
                let crossed_down = p_last_low >= surface_y - gamemode_addition && p_low < surface_y;
                if (self.p.y_velocity <= 0.0 || (if g_flip { stick_push } else { stick_rest }) || crossed_down)
                    && p_low >= surface_y - slope_tol && p_low <= surface_y + gamemode_addition {
                    if self.p.slope_riding && self.p.slope_exit_vel.map_or(false, |v| v > 0.0) && tangent < 0.0 { return none; }
                    return self.legacy_land(ride_y(true), false, Some(surface_y), None, angle, false, tangent);
                }
                return none;
            }
            let crossed_up = p_last_high <= surface_y + gamemode_addition && p_high > surface_y;
            if (self.p.y_velocity >= 0.0 || (if g_flip { stick_rest } else { stick_push }) || crossed_up)
                && p_high >= surface_y - slope_tol * 1.5 && p_high <= surface_y + slope_tol {
                if self.p.slope_riding && self.p.slope_exit_vel.map_or(false, |v| v < 0.0) && tangent > 0.0 { return none; }
                return self.legacy_land(ride_y(false), true, None, Some(surface_y), angle, false, tangent);
            }
            return none;
        }

        let rideable = is_ceil_slope == self.p.gravity_flipped;
        if !is_ceil_slope {
            if !rideable {
                if p_low < surface_y && p_low >= surface_y - player_size {
                    return self.legacy_land(ride_y(true), false, Some(surface_y), None, angle, true, 0.0);
                }
                return none;
            }
            if (p_last_low >= last_surface_y || self.p.on_ground) && (self.p.y_velocity <= 0.0 || self.p.on_ground) {
                if p_low >= surface_y - player_size && self.slope_reached(p_low, surface_y, gamemode_addition, true) {
                    if self.p.slope_riding && self.p.slope_exit_vel.map_or(false, |v| v > 0.0) && tangent < 0.0 { return none; }
                    return self.legacy_land(ride_y(true), false, Some(surface_y), None, angle, false, tangent);
                }
                return none;
            }
            return none;
        }
        if !rideable {
            if p_high > surface_y && p_high <= surface_y + player_size {
                return self.legacy_land(ride_y(false), true, None, Some(surface_y), angle, true, 0.0);
            }
            return none;
        }
        if (p_last_high <= last_surface_y || self.p.on_ground) && (self.p.y_velocity >= 0.0 || self.p.on_ground) {
            if p_high <= surface_y + player_size && self.slope_reached(p_high, surface_y, gamemode_addition, false) {
                if self.p.slope_riding && self.p.slope_exit_vel.map_or(false, |v| v < 0.0) && tangent > 0.0 { return none; }
                return self.legacy_land(ride_y(false), true, None, Some(surface_y), angle, false, tangent);
            }
            return none;
        }
        none
    }

    #[allow(clippy::too_many_arguments)]
    pub fn handle_slope_collision(&self, c: &Col, piece_width: f64, players_y: f64, players_last_y: f64, player_size: f64, wave_hit: f64, gamemode_addition: f64, nearby: &[Col]) -> SlopeResult {
        if self.p.is_wave && !self.p.wave_slide_active {
            let surface_y = match c.slope_surface_y(piece_width) { Some(v) => v, None => return SlopeResult::none() };
            let inside_solid = if c.slope_solid_below { players_y - wave_hit < surface_y } else { players_y + wave_hit > surface_y };
            let back_wall = self.slope_back_wall_hit(c, piece_width, players_y, Some(surface_y));
            return SlopeResult { landed: false, died: inside_solid || back_wall, ..Default::default() };
        }
        if !self.g.gd_cube_slopes { return self.legacy_slope_collision(c, piece_width, players_y, players_last_y, player_size, gamemode_addition); }
        let gd_surface_y = c.slope_surface_y(piece_width);
        if gd_surface_y.is_some() && self.slope_back_wall_hit(c, piece_width, players_y, gd_surface_y) {
            return SlopeResult { landed: false, died: true, ..Default::default() };
        }
        if let Some(cap) = self.slope_flat_cap_land(c, piece_width, players_y, players_last_y, player_size, gamemode_addition) { return cap; }
        self.gd_cube_slope_collision(c, piece_width, players_y, if self.p.is_wave { wave_hit } else { player_size }, nearby)
    }
}

pub fn fold_slope_angle(angle: f64) -> f64 {
    let mut a = or0(angle);
    while a > std::f64::consts::PI / 2.0 { a -= std::f64::consts::PI; }
    while a <= -std::f64::consts::PI / 2.0 { a += std::f64::consts::PI; }
    a
}

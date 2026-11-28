//! Game-mode entry/exit and the scene/level effects the player triggers (player.js enter*/exit*Mode).
//! Visual-only calls (sprites, particles, trails) are left out; scene and level-layer calls are recorded in `log`.
use crate::physics::Sim;

const F: f64 = 600.0; // `f` in config.js
const A: f64 = 60.0; // `a`

fn n(v: f64) -> String { if v == 0.0 { "0".into() } else { format!("{}", v) } }

impl<'a> Sim<'a> {
    /// `_setGamemodeFlyBounds(...args)` -> `gameLayer.setFlyMode(...args)` (skipped in dual mode).
    /// `h`/`cam` are `None` when the original call leaves them out (defaults: 600 and the height).
    pub fn set_fly_bounds(&mut self, enabled: bool, y: f64, h: Option<f64>, cam: Option<f64>) {
        if self.e.is_dual { return; }
        self.level_fly(enabled, y, h, cam);
    }

    /// `gameLayer.setFlyMode(...)` itself.
    pub fn level_fly(&mut self, enabled: bool, y: f64, h: Option<f64>, cam: Option<f64>) {
        let mut args = vec![enabled.to_string(), n(y)];
        if let Some(h) = h { args.push(n(h)); args.push("false".into()); }
        if let Some(c) = cam { args.push(n(c)); }
        self.log.push(format!("fly:[{}]", args.join(",")));
        if self.e.manage_layer {
            if enabled && self.e.portal_free_mode { return; }
            if enabled {
                let height = h.unwrap_or(F);
                // `_computeFlyBounds(centerY, height, isPortal = visualOnly = false)`
                let mut floor_y = y - height / 2.0;
                floor_y = (floor_y / A).floor() * A;
                floor_y = floor_y.max(0.0);
                self.e.fly_floor_y = Some(floor_y);
                self.e.fly_ceiling_y = Some(floor_y + height);
                self.e.fly_visual_only = false;
                self.e.fly_ground_active = true;
                let camera_span = cam.unwrap_or(height);
                let fly_center = floor_y + camera_span / 2.0;
                self.e.fly_camera_target = Some((fly_center - 320.0 + 180.0).max(0.0));
            } else {
                self.e.fly_camera_target = None;
                self.e.fly_ceiling_y = None;
                self.e.fly_floor_y = None;
                self.e.fly_visual_only = false;
                self.e.fly_ground_active = false;
            }
            self.e.floor_y = if self.e.fly_ground_active { if self.e.fly_visual_only { 0.0 } else { self.e.fly_floor_y.unwrap_or(f64::NAN) } } else { 0.0 };
            self.e.ceiling_y = self.e.fly_ceiling_y;
        }
    }
    fn fly_on(&mut self, y: f64, height: f64) { self.set_fly_bounds(true, y, Some(height), None); }
    fn fly_on_portal(&mut self, y: f64, height: f64, free_mode: bool) {
        if free_mode { self.set_fly_bounds(false, 0.0, None, None); } else { self.fly_on(y, height); }
    }
    fn fly_off(&mut self) { self.set_fly_bounds(false, 0.0, None, None); }
    fn glitter(&mut self, on: bool) { self.log.push(format!("glitter:{}", on)); }

    pub fn exit_ship_mode(&mut self) {
        if self.p.is_flying {
            self.p.is_flying = false;
            self.glitter(false);
            self.p.y_velocity *= 0.5;
            self.p.on_ground = false;
            self.p.can_jump = false;
            self.p.is_jumping = false;
            self.stop_rotation();
            self.p.rotation = 0.0;
            self.fly_off();
        }
    }
    pub fn enter_ship_mode(&mut self, portal_y: Option<f64>, from_checkpoint: bool, free_mode: bool) {
        if self.p.is_flying { return; }
        self.exit_swing_mode();
        self.exit_spider_mode(); self.exit_robot_mode(); self.exit_ball_mode(); self.exit_wave_mode();
        self.p.is_flying = true;
        self.glitter(true);
        if !from_checkpoint { self.p.y_velocity *= 0.5; }
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        let spawn_y = portal_y.unwrap_or(self.p.y);
        self.fly_on_portal(spawn_y, F, free_mode);
    }
    pub fn enter_ball_mode(&mut self, portal_y: Option<f64>, free_mode: bool) {
        if self.p.is_ball { return; }
        self.exit_swing_mode();
        self.exit_spider_mode(); self.exit_robot_mode(); self.exit_wave_mode();
        self.p.is_ball = true;
        self.p.ball_should_rotate = false;
        self.p.ball_rotate_opposite = false;
        self.p.ball_normal_rotate = 0.86;
        self.p.ball_hit_pad = false;
        self.p.ball_rotation2 = 0.86;
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        let y = portal_y.unwrap_or(self.p.y);
        self.fly_on_portal(y, F - A * 2.0, free_mode);
    }
    pub fn exit_ball_mode(&mut self) {
        if !self.p.is_ball { return; }
        self.p.is_ball = false;
        self.p.ball_should_rotate = false;
        self.p.ball_rotate_opposite = false;
        self.p.ball_normal_rotate = 0.86;
        self.p.ball_hit_pad = false;
        self.p.ball_rotation2 = 0.86;
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        self.fly_off();
    }
    pub fn enter_wave_mode(&mut self, portal_y: Option<f64>, free_mode: bool) {
        if self.p.is_wave { return; }
        self.exit_swing_mode();
        self.exit_spider_mode(); self.exit_robot_mode(); self.exit_ship_mode(); self.exit_ball_mode();
        self.p.is_wave = true;
        self.p.y_velocity = 0.0;
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        let y = portal_y.unwrap_or(self.p.y);
        self.fly_on_portal(y, F, free_mode);
    }
    pub fn exit_wave_mode(&mut self) {
        if !self.p.is_wave { return; }
        self.p.is_wave = false;
        self.p.y_velocity = 0.0;
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        self.fly_off();
    }
    pub fn enter_spider_mode(&mut self, portal_y: Option<f64>, from_portal: bool, free_mode: bool) {
        if self.p.is_spider { return; }
        self.exit_swing_mode();
        let portal_vy = if from_portal { Some(self.p.y_velocity) } else { None };
        self.exit_ship_mode(); self.exit_robot_mode(); self.exit_ball_mode(); self.exit_wave_mode();
        let saved = portal_vy.filter(|v| v.is_finite());
        self.p.is_spider = true;
        if !from_portal { self.p.y_velocity = 0.0; }
        self.p.on_ground = !from_portal;
        self.p.on_ceiling = !from_portal && self.p.gravity_flipped;
        self.p.can_jump = !from_portal;
        self.p.is_jumping = false;
        self.p.spider_teleport_pending = false;
        self.p.spider_teleport_anim_timer = 0.0;
        self.p.spider_flash_timer = 0.0;
        self.p.spider_flash_duration = 0.5;
        self.stop_rotation();
        self.p.rotation = 0.0;
        let y = portal_y.unwrap_or(self.p.y);
        let fly_h = A.max(F - A);
        if free_mode { self.set_fly_bounds(false, 0.0, None, None); } else { self.set_fly_bounds(true, y, Some(fly_h), Some(fly_h)); }
        if let Some(v) = saved { self.p.y_velocity = v; }
    }
    pub fn exit_spider_mode(&mut self) {
        if !self.p.is_spider { return; }
        self.p.is_spider = false;
        self.p.y_velocity = 0.0;
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.p.spider_teleport_pending = false;
        self.p.spider_teleport_anim_timer = 0.0;
        self.p.spider_flash_timer = 0.0;
        self.p.spider_flash_duration = 0.5;
        self.stop_rotation();
        self.p.rotation = 0.0;
        self.fly_off();
    }
    pub fn enter_robot_mode(&mut self, from_portal: bool) {
        if self.p.is_robot { return; }
        self.exit_swing_mode();
        let portal_vy = if from_portal { Some(self.p.y_velocity) } else { None };
        self.exit_ship_mode(); self.exit_ball_mode(); self.exit_wave_mode(); self.exit_spider_mode(); self.exit_ufo_mode();
        let saved = portal_vy.filter(|v| v.is_finite());
        self.p.is_robot = true;
        self.p.robot_hold = false;
        self.p.robot_accel = 1.5;
        self.p.robot_hold_timer = 0.0;
        self.p.on_ground = !from_portal;
        self.p.on_ceiling = !from_portal && self.p.gravity_flipped;
        self.p.can_jump = !from_portal;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        self.fly_off();
        if let Some(v) = saved { self.p.y_velocity = v; }
    }
    pub fn exit_robot_mode(&mut self) {
        if !self.p.is_robot { return; }
        self.p.is_robot = false;
        self.p.robot_hold = false;
        self.p.robot_hold_timer = 0.0;
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        self.fly_off();
    }
    pub fn enter_ufo_mode(&mut self, portal_y: Option<f64>, from_checkpoint: bool, free_mode: bool) {
        if self.p.is_ufo { return; }
        self.exit_swing_mode();
        self.exit_spider_mode(); self.exit_robot_mode(); self.exit_ball_mode(); self.exit_wave_mode(); self.exit_ship_mode();
        self.p.is_ufo = true;
        self.glitter(true);
        if !from_checkpoint { self.p.y_velocity *= 0.5; }
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        let y = portal_y.unwrap_or(self.p.y);
        self.fly_on_portal(y, F, free_mode);
    }
    pub fn exit_ufo_mode(&mut self) {
        if !self.p.is_ufo { return; }
        self.p.is_ufo = false;
        self.glitter(false);
        self.p.y_velocity *= 0.5;
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        self.fly_off();
    }

    pub fn enter_swing_mode(&mut self, portal_y: Option<f64>, from_checkpoint: bool, free_mode: bool) {
        if self.p.is_swing { return; }
        self.exit_ship_mode(); self.exit_ball_mode(); self.exit_wave_mode(); self.exit_ufo_mode(); self.exit_spider_mode(); self.exit_robot_mode();
        self.p.is_swing = true;
        if !from_checkpoint { self.p.y_velocity *= 0.5; }
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        let spawn_y = portal_y.unwrap_or(self.p.y);
        self.fly_on_portal(spawn_y, F, free_mode);
    }
    pub fn exit_swing_mode(&mut self) {
        if !self.p.is_swing { return; }
        self.p.is_swing = false;
        self.p.y_velocity *= 0.5;
        self.p.on_ground = false;
        self.p.can_jump = false;
        self.p.is_jumping = false;
        self.stop_rotation();
        self.p.rotation = 0.0;
        self.fly_off();
    }
}

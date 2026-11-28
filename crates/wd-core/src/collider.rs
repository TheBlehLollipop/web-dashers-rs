//! Collider creation, ported from `_spawnObject` (the collision half), `_createSlopeCollider` and friends in level.js.
use crate::jsmath as m;
use crate::json::{num, opt, str_lit, Obj};
use crate::level::LevelObject;
use crate::objects::{is_decoration_slope_id, object_from_id, slope_data, ObjectDef};
use crate::physics::speed_portal as sp;

pub const A: f64 = 60.0;
const S_BLOCK_OBJECT_ID: i64 = 1829;
const H_BLOCK_OBJECT_ID: i64 = 1859;
const J_BLOCK_OBJECT_ID: i64 = 1813;
const F_BLOCK_OBJECT_ID: i64 = 2866;
const FORCE_BLOCK_IDS: [i64; 2] = [2069, 3645];

#[derive(Debug, Clone, Copy)]
pub struct ToggleBlock { pub group: i64, pub activate: bool, pub claim_touch: bool, pub spawn_only: bool }

#[derive(Debug, Clone, Copy)]
pub struct Force { pub force: f64, pub min: f64, pub max: f64, pub radial: bool, pub range: bool, pub id: i64, pub flip_y: bool }

#[derive(Debug, Clone)]
pub struct Collider {
    pub ty: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub rotation_degrees: f64,
    pub activated: bool,
    pub objid: Option<i64>,
    pub slope_angle_deg: f64,
    pub slope_dir: f64,
    pub slope_is_filled: bool,
    pub slope_flip_y: bool,
    pub hypo_a: (f64, f64),
    pub hypo_b: (f64, f64),
    pub right_a: (f64, f64),
    pub slope_solid_below: bool,
    pub slope_type: Option<i64>,
    pub gd_solid_above: bool,
    pub gd_wall_on_left: bool,
    pub hitbox_radius: Option<f64>,
    pub particle_color: Option<f64>,
    pub portal_x: Option<f64>,
    pub portal_y: Option<f64>,
    pub teleport_target_x: Option<f64>,
    pub teleport_target_y: Option<f64>,
    pub teleport_y_offset: Option<f64>,
    pub pad_id: Option<i64>,
    pub orb_id: Option<i64>,
    pub multi_activate: bool,
    pub free_mode: bool,
    pub camera_settings: Option<(f64, f64)>,
    pub toggle: Option<ToggleBlock>,
    pub flip_y: bool,
    pub orb_rotation: Option<f64>,
    pub coin_id: Option<i64>,
    pub speed_value: Option<f64>,
    pub speed_id: Option<i64>,
    // set by registerCollider
    pub base_x: f64,
    pub base_y: f64,
    pub base_rotation_degrees: f64,
    /// A bare `{ type, activated, x: 0, y: 0 }` entry for objects without a collider.
    pub placeholder: bool,
    pub claim_touch: bool,
    pub force: f64,
    pub force_min: f64,
    pub force_max: f64,
    pub force_radial: bool,
    pub force_range: bool,
    pub force_id: i64,
    pub force_flip_y: bool,
}

impl Collider {
    fn new(ty: &str, x: f64, y: f64, w: f64, h: f64, rotation: f64) -> Self {
        Collider {
            ty: ty.to_string(), x, y, w, h, rotation_degrees: rotation, activated: false, objid: None,
            slope_angle_deg: 0.0, slope_dir: 1.0, slope_is_filled: false, slope_flip_y: false,
            hypo_a: (0.0, 0.0), hypo_b: (0.0, 0.0), right_a: (0.0, 0.0), slope_solid_below: true,
            slope_type: None, gd_solid_above: false, gd_wall_on_left: false, hitbox_radius: None, particle_color: None,
            portal_x: None, portal_y: None, teleport_target_x: None, teleport_target_y: None, teleport_y_offset: None,
            pad_id: None, orb_id: None, multi_activate: false, free_mode: false, camera_settings: None, toggle: None, flip_y: false, orb_rotation: None, coin_id: None, speed_value: None,
            speed_id: None, base_x: 0.0, base_y: 0.0, base_rotation_degrees: 0.0, placeholder: false,
            claim_touch: false, force: 0.0, force_min: 0.0, force_max: 0.0, force_radial: false, force_range: false, force_id: 0, force_flip_y: false,
        }
    }

    fn register(mut self) -> Self {
        self.base_x = self.x;
        self.base_y = self.y;
        self.base_rotation_degrees = self.rotation_degrees;
        self
    }

    pub fn to_json(&self) -> String {
        if self.placeholder {
            let mut o = Obj::new();
            o.field("type", str_lit(&self.ty)); o.field("activated", "false".into()); o.field("x", "0".into()); o.field("y", "0".into());
            return o.finish();
        }
        let pair = |p: (f64, f64)| format!("[{},{}]", num(p.0), num(p.1));
        let of = |v: Option<f64>| v.map(num).unwrap_or_else(|| "null".into());
        let mut o = Obj::new();
        o.field("type", str_lit(&self.ty));
        o.field("x", num(self.x)); o.field("y", num(self.y)); o.field("w", num(self.w)); o.field("h", num(self.h));
        o.field("rotationDegrees", num(self.rotation_degrees));
        o.field("activated", self.activated.to_string());
        o.field("objid", opt(self.objid));
        o.field("slopeAngleDeg", num(self.slope_angle_deg));
        o.field("slopeDir", num(self.slope_dir));
        o.field("slopeIsFilled", self.slope_is_filled.to_string());
        o.field("slopeFlipY", self.slope_flip_y.to_string());
        o.field("hypoA", pair(self.hypo_a)); o.field("hypoB", pair(self.hypo_b)); o.field("rightA", pair(self.right_a));
        o.field("slopeSolidBelow", self.slope_solid_below.to_string());
        o.field("slopeType", opt(self.slope_type));
        o.field("gdSolidAbove", self.gd_solid_above.to_string());
        o.field("gdWallOnLeft", self.gd_wall_on_left.to_string());
        o.field("hitbox_radius", of(self.hitbox_radius));
        o.field("particleColor", of(self.particle_color));
        o.field("portalX", of(self.portal_x)); o.field("portalY", of(self.portal_y));
        o.field("teleportTargetX", of(self.teleport_target_x)); o.field("teleportTargetY", of(self.teleport_target_y));
        o.field("teleportYOffset", of(self.teleport_y_offset));
        o.field("padId", opt(self.pad_id)); o.field("orbId", opt(self.orb_id));
        o.field("multiActivate", self.multi_activate.to_string());
        o.field("freeMode", self.free_mode.to_string()); o.field("flipY", self.flip_y.to_string());
        o.field("cameraSettings", self.camera_settings.map_or("null".to_string(), |(e, p)| format!("[{},{}]", num(e), num(p))));
        let isf = self.ty == "forceblock";
        let fo = |v: String| if isf { v } else { "null".to_string() };
        let tb = |f: fn(&ToggleBlock) -> String| self.toggle.as_ref().map_or("null".to_string(), f);
        o.field("toggleGroup", tb(|t| t.group.to_string())); o.field("toggleActivate", tb(|t| t.activate.to_string()));
        o.field("claimTouch", tb(|t| t.claim_touch.to_string())); o.field("spawnOnly", tb(|t| t.spawn_only.to_string()));
        o.field("force", fo(num(self.force))); o.field("forceMin", fo(num(self.force_min))); o.field("forceMax", fo(num(self.force_max)));
        o.field("forceRadial", fo(self.force_radial.to_string())); o.field("forceRange", fo(self.force_range.to_string())); o.field("forceId", fo(self.force_id.to_string())); o.field("forceFlipY", fo(self.force_flip_y.to_string()));
        o.field("orbRotation", of(self.orb_rotation));
        o.field("coinId", opt(self.coin_id));
        o.field("speedValue", of(self.speed_value)); o.field("speedId", opt(self.speed_id));
        o.field("_baseX", num(self.base_x)); o.field("_baseY", num(self.base_y)); if self.toggle.is_none() { o.field("_baseRotationDegrees", num(self.base_rotation_degrees)); }
        o.finish()
    }
}

/// JavaScript `x || 0` for numbers.
fn or0(x: f64) -> f64 { if x == 0.0 || x.is_nan() { 0.0 } else { x } }

fn finite_or(v: f64, fallback: f64) -> f64 { if v.is_finite() { v } else { fallback } }

/// `gdObjectScale`.
fn gd_object_scale(o: &LevelObject) -> (f64, f64) {
    let fallback = finite_or(o.scale, 1.0);
    (finite_or(o.scale_x, fallback), finite_or(o.scale_y, fallback))
}

fn rot90(rot_deg: f64) -> i64 { js_mod(js_round_div(((rot_deg % 360.0) + 360.0) % 360.0, 90.0), 4.0) as i64 }
fn js_round_div(a: f64, b: f64) -> f64 { crate::physics::js_round(a / b) }
fn js_mod(a: f64, b: f64) -> f64 { a % b }

fn slope_type_of(flip_x: bool, flip_y: bool, rotation: f64) -> Option<i64> {
    let rot = (crate::js::parse_int(&js_int_string(rotation)).unwrap_or(0)) % 360;
    let r0 = rot == 0;
    let r180 = rot.abs() == 180;
    let r90 = rot == 90 || rot == -270;
    let r270 = rot == -90 || rot == 270;
    let pick = |a: i64, b: i64, c: i64, d: i64| -> Option<i64> {
        if r0 { Some(a) } else if r90 { Some(b) } else if r180 { Some(c) } else if r270 { Some(d) } else { None }
    };
    match (flip_x, flip_y) {
        (false, false) => pick(0, 4, 3, 5),
        (false, true) => pick(1, 7, 2, 6),
        (true, false) => pick(2, 6, 1, 7),
        (true, true) => pick(3, 5, 0, 4),
    }
}

/// `parseInt(number, 10)` goes through `String(number)`.
fn js_int_string(v: f64) -> String {
    if v.is_nan() { return "NaN".into(); }
    if v.is_infinite() { return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() }; }
    if v != 0.0 && v.abs() < 1e-6 || v.abs() >= 1e21 {
        // exponent form: parseInt reads the leading digits only
        return format!("{:e}", v).replace("e", "e");
    }
    format!("{}", v)
}

fn slope_solid_above(t: Option<i64>) -> bool { matches!(t, Some(t) if t < 7 && ((0x6au32 >> t) & 1) != 0) }
fn slope_wall_on_left(t: Option<i64>) -> bool { matches!(t, Some(t) if (t - 2) as u32 as i64 >= 0 && ((t - 2) as u32) < 3 || t == 6) }

/// `_resolveSlopeOrientation(...).dir`.
fn resolve_slope_dir(o: &LevelObject) -> f64 {
    let mut dir = if o.flip_x { -1.0 } else { 1.0 };
    let mut flip_y = o.flip_y;
    match rot90(or0(o.rot)) {
        1 => { let next = if flip_y { 1.0 } else { -1.0 }; flip_y = dir < 0.0; dir = next; }
        2 => { dir = -dir; flip_y = !flip_y; }
        3 => { let next = if flip_y { -1.0 } else { 1.0 }; flip_y = !(dir < 0.0); dir = next; }
        _ => {}
    }
    let _ = flip_y;
    dir
}

fn rotate_slope_point(lx: f64, ly: f64, rot_deg: f64) -> (f64, f64) {
    let theta = -rot_deg * std::f64::consts::PI / 180.0;
    let (c, s) = (m::cos(theta), m::sin(theta));
    (lx * c - ly * s, lx * s + ly * c)
}

fn snap(p: (f64, f64)) -> (f64, f64) {
    let r = |v: f64| crate::physics::js_round(v * 1e6) / 1e6;
    (r(p.0), r(p.1))
}

fn create_slope_collider(o: &LevelObject, wx: f64, wy: f64) -> Option<Collider> {
    let id = o.id?;
    let sd = slope_data(id)?;
    if sd.sq { return None; }
    if is_decoration_slope_id(id) { return None; }
    let scale = if o.scale.is_finite() && o.scale > 0.0 { o.scale } else { 1.0 };
    let sx = { let v = o.scale_x.abs(); if v == 0.0 || v.is_nan() { scale } else { v } };
    let sy = { let v = o.scale_y.abs(); if v == 0.0 || v.is_nan() { scale } else { v } };
    let (hw0, hh0) = ((sd.gw * A * sx) / 2.0, (sd.gh * A * sy) / 2.0);
    let (mut v_right, mut v_lo, mut v_hi) = ((hw0, -hh0), (-hw0, -hh0), (hw0, hh0));
    let fx = if o.flip_x { -1.0 } else { 1.0 };
    let fy = if o.flip_y { -1.0 } else { 1.0 };
    let flip = |p: (f64, f64)| (p.0 * fx, p.1 * fy);
    v_right = flip(v_right); v_lo = flip(v_lo); v_hi = flip(v_hi);
    let rot_deg = or0(o.rot);
    v_right = rotate_slope_point(v_right.0, v_right.1, rot_deg);
    v_lo = rotate_slope_point(v_lo.0, v_lo.1, rot_deg);
    v_hi = rotate_slope_point(v_hi.0, v_hi.1, rot_deg);
    v_right = snap(v_right); v_lo = snap(v_lo); v_hi = snap(v_hi);
    let xs = [v_right.0, v_lo.0, v_hi.0];
    let ys = [v_right.1, v_lo.1, v_hi.1];
    let fold = |a: &[f64], f: fn(f64, f64) -> f64, init: f64| a.iter().fold(init, |acc, &v| f(acc, v));
    let w = fold(&xs, f64::max, f64::NEG_INFINITY) - fold(&xs, f64::min, f64::INFINITY);
    let h = fold(&ys, f64::max, f64::NEG_INFINITY) - fold(&ys, f64::min, f64::INFINITY);
    let mut c = Collider::new("slope", wx, wy, w, h, 0.0);
    c.objid = Some(id);
    c.hypo_a = v_lo; c.hypo_b = v_hi; c.right_a = v_right;
    let hypo_dx = v_hi.0 - v_lo.0;
    c.slope_solid_below = if hypo_dx.abs() < 0.01 {
        v_right.0 < v_lo.0
    } else {
        v_right.1 < (v_lo.1 + ((v_right.0 - v_lo.0) / hypo_dx) * (v_hi.1 - v_lo.1))
    };
    let r90 = rot90(rot_deg);
    let steep = r90 == 1 || r90 == 3;
    c.slope_angle_deg = if steep { 90.0 - sd.angle } else { sd.angle };
    c.slope_dir = resolve_slope_dir(o);
    c.slope_flip_y = !c.slope_solid_below;
    c.slope_is_filled = sd.sq;
    c.slope_type = slope_type_of(o.flip_x, o.flip_y, o.rot);
    c.gd_solid_above = slope_solid_above(c.slope_type);
    c.gd_wall_on_left = slope_wall_on_left(c.slope_type);
    Some(c)
}

fn raw<'a>(o: &'a LevelObject, k: &str) -> Option<&'a String> { o.raw.get(k) }

fn portal_sub(id: i64) -> Option<&'static str> {
    Some(match id {
        10 => "gravity_flip", 11 => "gravity_normal", 12 => "cube", 13 => "fly", 45 => "mirrora", 46 => "mirrorb",
        47 => "ball", 660 => "wave", 111 => "ufo", 745 => "robot", 1331 => "spider", 1933 => "swing", 747 => "teleport",
        286 => "dual_on", 287 => "dual_off",
        _ => return None,
    })
}

fn portal_collider_type(sub: &str) -> Option<&'static str> {
    Some(match sub {
        "gravity_flip" => "portal_gravity_down", "gravity_normal" => "portal_gravity_up", "gravity_toggle" => "portal_gravity_toggle",
        "fly" => "portal_fly", "cube" => "portal_cube", "ball" => "portal_ball", "wave" => "portal_wave", "ufo" => "portal_ufo",
        "robot" => "portal_robot", "spider" => "portal_spider", "swing" => "portal_swing", "teleport" => "portal_teleport",
        "mirrora" => "portal_mirror_on", "mirrorb" => "portal_mirror_off", "shrink" => "portal_mini_on", "grow" => "portal_mini_off",
        "dual_on" => "portal_dual_on", "dual_off" => "portal_dual_off",
        _ => return None,
    })
}

fn teleport_y_offset(o: &LevelObject) -> f64 {
    let parsed = match raw(o, "54") { Some(s) => crate::js::parse_float(s), None => Some(90.0) };
    match parsed { Some(v) if v.is_finite() => v, _ => 90.0 }
}

/// The collision-entry half of `_spawnObject`. `None` = the object isn't spawned at all (no definition,
/// or a trigger). Otherwise the list of entries it adds to `objects` (a placeholder if it has no collider).
pub fn spawn_colliders(o: &LevelObject) -> Option<Vec<Collider>> {
    let id = o.id.unwrap_or(0);
    // Standalone exit portals (749) are only spawned through `_normalizeTeleportPortals`, which isn't ported yet.
    if id == 749 { return None; }
    let def: &ObjectDef = object_from_id(id)?;
    if id == 3643 {
        let flag = |k: &str| raw(o, k).map(|s| s.as_str()).unwrap_or("0") == "1";
        let sc = or0(o.scale);
        let size = 60.0 * (if sc == 0.0 { 1.0 } else { sc });
        let mut c = Collider::new("jump_ring", o.x * 2.0, o.y * 2.0, size, size, 0.0);
        c.orb_id = Some(3643);
        c.multi_activate = flag("99");
        c.claim_touch = flag("445");
        c.toggle = Some(ToggleBlock { group: raw(o, "51").and_then(|s| crate::js::parse_int(s)).unwrap_or(0), activate: flag("56"), claim_touch: flag("445"), spawn_only: flag("504") });
        let mut c = c.register();
        c.base_x = c.x; c.base_y = c.y;
        return Some(vec![c]);
    }
    if crate::objects::is_trigger_branch(id) { return None; }
    if def.ty == Some("trigger") && id != J_BLOCK_OBJECT_ID && !FORCE_BLOCK_IDS.contains(&id) { return None; }
    let (wx, wy) = (o.x * 2.0, o.y * 2.0);
    let no_touch = raw(o, "121").map(|s| s.as_str()) == Some("1");
    let (hx, hy) = { let (x, y) = gd_object_scale(o); (x.abs(), y.abs()) };
    let hmean = (hx + hy) / 2.0;
    let rot = or0(o.rot);
    let gw = def.grid_w.unwrap_or(f64::NAN);
    let gh = def.grid_h.unwrap_or(f64::NAN);
    let grid_ok = gw > 0.0 && gh > 0.0;
    let mut out: Vec<Collider> = Vec::new();

    let slope = if no_touch { None } else { create_slope_collider(o, wx, wy) };
    if let Some(c) = slope {
        out.push(c.register());
    } else if !no_touch && def.ty == Some("solid") && grid_ok {
        let mut c = Collider::new("solid", wx, wy, gw * A * hx, gh * A * hy, rot);
        c.objid = o.id;
        out.push(c.register());
    } else if !no_touch && def.ty == Some("dblock") && grid_ok {
        let mut c = Collider::new("dblock", wx, wy, gw * A * hx, gh * A * hy, rot);
        c.objid = o.id;
        out.push(c.register());
    } else if !no_touch && FORCE_BLOCK_IDS.contains(&id) {
        let fnum = |k: &str| match raw(o, k) { Some(s) => crate::js::parse_float(s).filter(|v| v.is_finite()).unwrap_or(0.0), None => 0.0 };
        let flag = |k: &str| raw(o, k).map(|s| s.as_str()) == Some("1");
        let mut c = Collider::new("forceblock", wx, wy, gw * A * hx, gh * A * hy, rot);
        c.objid = o.id;
        if let Some(r) = def.hitbox_radius { if r != 0.0 { c.hitbox_radius = Some(r * 2.0 * hmean); } }
        c.force = fnum("149");
        c.force_min = fnum("526");
        c.force_max = fnum("527");
        c.force_radial = flag("528");
        c.force_range = flag("529");
        c.force_id = raw(o, "530").and_then(|s| crate::js::parse_float(s)).map(|v| v.trunc() as i64).filter(|v| *v != 0).unwrap_or(0);
        c.force_flip_y = flag("5");
        out.push(c.register());
    } else if !no_touch && (id == S_BLOCK_OBJECT_ID || id == H_BLOCK_OBJECT_ID || id == F_BLOCK_OBJECT_ID || id == J_BLOCK_OBJECT_ID) && grid_ok {
        let mut c = Collider::new(if id == H_BLOCK_OBJECT_ID { "hblock" } else if id == F_BLOCK_OBJECT_ID { "fblock" } else if id == J_BLOCK_OBJECT_ID { "jblock" } else { "sblock" }, wx, wy, gw * A * hx, gh * A * hy, rot);
        c.objid = o.id;
        out.push(c.register());
    } else if !no_touch && def.ty == Some("hazard") {
        let (mut hit_w, mut hit_h) = (0.0, 0.0);
        let sw = def.sprite_w.unwrap_or(f64::NAN);
        let sh = def.sprite_h.unwrap_or(f64::NAN);
        if sw > 0.0 && sh > 0.0 && def.hitbox_scale_x.is_some() && def.hitbox_scale_y.is_some() {
            hit_w = sw * def.hitbox_scale_x.unwrap() * 2.0 * hx;
            hit_h = sh * def.hitbox_scale_y.unwrap() * 2.0 * hy;
        } else if grid_ok {
            hit_w = gw * 12.0 * hx;
            hit_h = gh * 24.0 * hy;
        }
        let world_radius = def.hitbox_radius.map(|r| r * 2.0 * hmean).unwrap_or(0.0);
        if def.hitbox_radius.is_some() && hit_w == 0.0 {
            hit_w = world_radius * 2.0;
            hit_h = world_radius * 2.0;
        }
        if hit_w > 0.0 && hit_h > 0.0 {
            let mut c = Collider::new("hazard", wx, wy, hit_w, hit_h, rot);
            if def.hitbox_radius.is_some() { c.hitbox_radius = Some(world_radius); }
            out.push(c.register());
        }
    } else if def.ty == Some("portal") {
        let (pw, ph) = (gw * A * hx, gh * A * hy);
        let sub = def.sub.or_else(|| portal_sub(id));
        if let Some(cty) = sub.and_then(portal_collider_type) {
            let prot = { let v = rot; if v.is_nan() { 0.0 } else { v } };
            let prad = prot * std::f64::consts::PI / 180.0;
            let is_tp = cty == "portal_teleport";
            let shift = if is_tp { -30.0 * hx } else { 0.0 };
            let cx = wx - m::cos(prad) * shift;
            let cy = wy + m::sin(prad) * shift;
            let mut c = Collider::new(cty, cx, cy, pw, ph, prot);
            c.particle_color = def.portal_particle_color;
            c.portal_x = Some(wx);
            c.portal_y = Some(wy);
            c.free_mode = raw(o, "111").map(|s| s.as_str()).unwrap_or("0") == "1";
            if raw(o, "112").map(|s| s.as_str()).unwrap_or("0") == "1" {
                let finite = |k: &str| raw(o, k).and_then(|s| crate::js::parse_float(s)).filter(|v| v.is_finite()).unwrap_or(0.0);
                c.camera_settings = Some((finite("113").min(40.0).max(1.0), finite("114").min(1.0).max(0.0)));
            }
            if is_tp {
                let y_off = teleport_y_offset(o);
                c.teleport_target_x = Some(wx);
                c.teleport_target_y = Some(wy + y_off * 2.0);
                c.teleport_y_offset = Some(y_off * 2.0);
            }
            out.push(c.register());
        }
    } else if def.ty == Some("pad") {
        let mut c = Collider::new("jump_pad", wx, wy, gw * A * hx, gh * A * hy, rot);
        c.pad_id = o.id;
        c.flip_y = o.flip_y;
        out.push(c.register());
    } else if def.ty == Some("ring") {
        let mut c = Collider::new("jump_ring", wx, wy, gw * A * hx, gh * A * hy, rot);
        c.orb_id = o.id;
        c.flip_y = o.flip_y;
        c.multi_activate = raw(o, "99").map(|s| s.as_str()) == Some("1");
        c.orb_rotation = Some(rot);
        out.push(c.register());
    } else if def.ty == Some("coin") {
        let g1 = if gw == 0.0 || gw.is_nan() { 1.0 } else { gw };
        let g2 = if gh == 0.0 || gh.is_nan() { 1.0 } else { gh };
        let mut c = Collider::new("coin", wx, wy, g1 * A * hx, g2 * A * hy, rot);
        c.coin_id = o.id;
        out.push(c.register());
    } else if def.ty == Some("speed") {
        let g1 = if gw == 0.0 || gw.is_nan() { 1.0 } else { gw };
        let g2 = if gh == 0.0 || gh.is_nan() { 1.0 } else { gh };
        let mut c = Collider::new("speed", wx, wy, g1 * A * hx, g2 * A * hy, rot);
        c.portal_y = Some(wy);
        c.speed_value = Some(match id {
            200 => sp::HALF, 201 => sp::ONE_TIMES, 202 => sp::TWO_TIMES, 203 => sp::THREE_TIMES, 1334 => sp::FOUR_TIMES,
            _ => sp::ONE_TIMES,
        });
        c.speed_id = o.id;
        out.push(c.register());
    }
    if out.is_empty() {
        // `objects.push({ type: objectDef.type || "deco", activated: false, x: 0, y: 0 })`
        let mut c = Collider::new(def.ty.unwrap_or("deco"), 0.0, 0.0, 0.0, 0.0, 0.0);
        c.placeholder = true;
        out.push(c);
    }
    Some(out)
}

/// All entries of a level, as JSON: `[[colliders of object 0], ...]` for objects that spawn, `null` otherwise.
pub fn level_colliders_json(level: &crate::level::Level) -> String {
    let items: Vec<String> = level.objects.iter().map(|o| match spawn_colliders(o) {
        None => "null".to_string(),
        Some(v) => format!("[{}]", v.iter().map(|c| c.to_json()).collect::<Vec<_>>().join(",")),
    }).collect();
    format!("[{}]", items.join(","))
}

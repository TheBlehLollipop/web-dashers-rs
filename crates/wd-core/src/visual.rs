//! The visual half of level.js `_spawnObject`: which sprites an object creates and how they are placed, tinted and registered.
//! A `Sprite` is a plain record of what the original would have put on a Phaser image (position, rotation, scale, flips, alpha,
//! tint, blend mode) plus the `_ee*` bookkeeping fields the rest of the game reads, kept in a map like the JS expandos.
//! Drawing is done elsewhere; this only decides what exists.
use crate::json::{num, str_lit};
use crate::jsmath as m;
use crate::jsonread::{parse, Json};
use crate::level::LevelObject;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

const PI: f64 = std::f64::consts::PI;
const ADD: i32 = 1; // Phaser.BlendModes.ADD
const T: f64 = 460.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Val { Num(f64), Bool(bool), Str(String), Arr(Vec<f64>), List(Vec<String>) }

#[derive(Clone, Debug)]
pub struct Sprite {
    pub atlas: String,
    pub frame: String,
    pub x: f64,
    pub y: f64,
    pub rotation: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub alpha: f64,
    pub visible: bool,
    pub active: bool,
    pub flip_x: bool,
    pub flip_y: bool,
    pub tint: Option<f64>,
    pub blend: i32,
    pub props: Props,
}

/// Property bag of a sprite. A sorted map of string keys spent a third of the frame in `memcmp`; this keeps a hash with each key, so a lookup
/// compares numbers and only touches the text of the one entry that matches. `iter` still goes in key order, as the map did.
#[derive(Clone, Debug, Default)]
pub struct Props(Vec<(u64, &'static str, Val)>);

#[inline]
fn key_hash(k: &str) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for b in k.bytes() { h = (h ^ b as u64).wrapping_mul(0x100000001b3); }
    h
}

impl Props {
    /// a sprite carries a few dozen properties: room for them up front saves the repeated regrowth while a level loads
    pub fn with_capacity(n: usize) -> Props { Props(Vec::with_capacity(n)) }
    #[inline]
    pub fn get(&self, k: &str) -> Option<&Val> {
        let h = key_hash(k);
        self.0.iter().find(|e| e.0 == h && e.1 == k).map(|e| &e.2)
    }
    pub fn contains_key(&self, k: &str) -> bool { self.get(k).is_some() }
    pub fn insert(&mut self, k: &'static str, v: Val) {
        let h = key_hash(k);
        match self.0.iter_mut().find(|e| e.0 == h && e.1 == k) { Some(e) => e.2 = v, None => self.0.push((h, k, v)) }
    }
    pub fn remove(&mut self, k: &str) {
        let h = key_hash(k);
        if let Some(i) = self.0.iter().position(|e| e.0 == h && e.1 == k) { self.0.remove(i); }
    }
    /// (key, value) in key order
    pub fn iter(&self) -> Vec<(&'static str, &Val)> {
        let mut v: Vec<(&'static str, &Val)> = self.0.iter().map(|e| (e.1, &e.2)).collect();
        v.sort_by(|a, b| a.0.cmp(b.0));
        v
    }
}

impl Sprite {
    pub fn new(atlas: &str, frame: &str, x: f64, y: f64) -> Sprite {
        Sprite { atlas: atlas.into(), frame: frame.into(), x, y, rotation: 0.0, scale_x: 1.0, scale_y: 1.0, alpha: 1.0, visible: true, active: true, flip_x: false, flip_y: false, tint: None, blend: 0, props: Props::default() }
    }
    pub fn num(&self, k: &str) -> Option<f64> { match self.props.get(k) { Some(Val::Num(n)) => Some(*n), _ => None } }
    pub fn flag(&self, k: &str) -> bool { matches!(self.props.get(k), Some(Val::Bool(true))) }
    pub fn set_num(&mut self, k: &'static str, v: f64) { self.props.insert(k, Val::Num(v)); }
    pub fn set_flag(&mut self, k: &'static str, v: bool) { self.props.insert(k, Val::Bool(v)); }
    /// `setAngle(deg)`: Phaser wraps the angle into [-180, 180)
    pub fn set_angle(&mut self, deg: f64) {
        let wrapped = -180.0 + ((((deg + 180.0) % 360.0) + 360.0) % 360.0);
        self.rotation = wrapped * (PI / 180.0);
    }
    pub fn set_alpha(&mut self, a: f64) { self.alpha = a.max(0.0).min(1.0); }
    /// `tintTopLeft`
    pub fn tint_top_left(&self) -> f64 { self.tint.unwrap_or(16777215.0) }
}

// ---- object definitions ----
static DEFS_JSON: &str = include_str!("../data/objects_full.json");
pub struct Defs { root: Json, index: HashMap<i64, usize> }
fn defs() -> &'static Defs {
    static D: OnceLock<Defs> = OnceLock::new();
    D.get_or_init(|| {
        let root = parse(DEFS_JSON).expect("object definitions");
        let mut index = HashMap::new();
        if let Json::Obj(e) = &root { for (i, (k, _)) in e.iter().enumerate() { if let Ok(id) = k.parse::<i64>() { index.insert(id, i); } } }
        Defs { root, index }
    })
}
pub fn def_of(id: i64) -> Option<&'static Json> {
    let d = defs();
    d.index.get(&id).and_then(|&i| match &d.root { Json::Obj(e) => e.get(i).map(|(_, v)| v), _ => None })
}

// ---- atlases ----
const ATLAS_LIST: &[&str] = &["GJ_WebSheet", "GJ_GameSheet", "GJ_GameSheet02", "GJ_GameSheet03", "GJ_GameSheet04", "GJ_GameSheetEditor", "GJ_GameSheetGlow", "GJ_GameSheetIcons",
    "GJ_LaunchSheet", "player_ball_00", "player_dart_00", "GJ_ParticleSheet-uhd", "GJ_ParticleSheet", "PixelSheet_01-hd", "FireSheet_01-hd", "Wavesheet"];

#[derive(Clone, Copy, Debug, Default)]
pub struct AtlasFrame { pub width: f64, pub height: f64, pub real_w: f64, pub real_h: f64, pub src_x: f64, pub src_y: f64, pub has_sprite_source: bool }

/// The sprite lists the per-frame animation code walks (level.js `_animatedSprites`, `_sawSprites`, `_orbSprites`, `_audioScaleSprites`, `_audioPulseSprites`).
#[derive(Default, Clone, Debug)]
pub struct SpriteLists { pub animated: Vec<usize>, pub saws: Vec<usize>, pub orbs: Vec<usize>, pub audio_scale: Vec<usize>, pub audio_pulse: Vec<usize>, pub coins: Vec<usize> }

#[derive(Default)]
pub struct Atlases { pub sheets: HashMap<String, HashMap<String, AtlasFrame>> }

impl Atlases {
    /// Adds a Phaser JSON atlas (`{"frames": {name: {frame, spriteSourceSize, sourceSize, ...}}}`).
    pub fn add(&mut self, name: &str, json: &str) -> Result<usize, String> {
        let j = parse(json)?;
        let mut frames = HashMap::new();
        // TexturePacker multi-atlas files keep their frames under `textures[0].frames`
        let f = match j.get("frames") { Some(f) => f, None => j.get("textures").and_then(|t| t.arr()).and_then(|a| a.first()).and_then(|t| t.get("frames")).ok_or("no frames")? };
        let mut add = |name: &str, d: &Json| {
            let fr = d.get("frame");
            let (w, h) = (fr.and_then(|f| f.get_num("w")).unwrap_or(0.0), fr.and_then(|f| f.get_num("h")).unwrap_or(0.0));
            let ss = d.get("spriteSourceSize");
            let sz = d.get("sourceSize");
            frames.insert(name.to_string(), AtlasFrame {
                width: w, height: h, real_w: sz.and_then(|s| s.get_num("w")).unwrap_or(w), real_h: sz.and_then(|s| s.get_num("h")).unwrap_or(h),
                src_x: ss.and_then(|s| s.get_num("x")).unwrap_or(0.0), src_y: ss.and_then(|s| s.get_num("y")).unwrap_or(0.0), has_sprite_source: ss.is_some(),
            });
        };
        match f {
            Json::Obj(e) => for (k, v) in e { add(k, v); },
            Json::Arr(a) => for v in a { if let Some(n) = v.get_str("filename") { add(n, v); } },
            _ => return Err("bad frames".into()),
        }
        let n = frames.len();
        self.sheets.insert(name.to_string(), frames);
        Ok(n)
    }
    /// `getAtlasFrame` (the icon atlases are not part of object visuals)
    pub fn find(&self, frame: &str) -> Option<(&str, AtlasFrame)> {
        if frame.starts_with("player_") {
            for a in ["GJ_GameSheet03", "GJ_GameSheet", "GJ_GameSheet02", "GJ_GameSheet04", "GJ_GameSheetEditor", "GJ_GameSheetGlow", "GJ_GameSheetIcons", "GJ_WebSheet", "GJ_LaunchSheet", "player_ball_00", "player_dart_00"] {
                if let Some(f) = self.sheets.get(a).and_then(|s| s.get(frame)) { return Some((a, *f)); }
            }
        }
        for a in ATLAS_LIST {
            if let Some(f) = self.sheets.get(*a).and_then(|s| s.get(frame)) { return Some((a, *f)); }
        }
        None
    }
}

// ---- geometry helpers (level.js) ----
fn finite_or(v: f64, d: f64) -> f64 { if v.is_finite() { v } else { d } }
pub fn gd_object_scale(o: &LevelObject) -> (f64, f64) {
    let fallback = finite_or(o.scale, 1.0);
    (finite_or(o.scale_x, fallback), finite_or(o.scale_y, fallback))
}
pub fn gd_object_warp(o: &LevelObject) -> f64 { finite_or(o.warp, 0.0) }
fn warp_axes(rot_deg: f64, warp_deg: f64) -> (f64, f64) { let to_rad = PI / 180.0; ((rot_deg - warp_deg / 2.0) * to_rad, (rot_deg + warp_deg / 2.0) * to_rad) }
pub fn transform_local_point(x: f64, y: f64, rot_deg: f64, sx: f64, sy: f64, warp_deg: f64) -> (f64, f64) {
    let (rx, ry) = warp_axes(rot_deg, warp_deg);
    (m::cos(ry) * sx * x - m::sin(rx) * sy * y, m::sin(ry) * sx * x + m::cos(rx) * sy * y)
}
fn or0(x: f64) -> f64 { if x == 0.0 || x.is_nan() { 0.0 } else { x } }

/// The per-level state `_spawnObject` writes to.
pub struct Builder<'a> {
    pub atlases: &'a Atlases,
    pub sprites: Vec<Sprite>,
    pub spawn_order: f64,
    enter_channel: f64,
    parent_groups: Vec<f64>,
    pub secret_slots: i64,
    pub user_slots: i64,
    /// saved coin progress (linked object ids), for the collected look of coins
    pub secret_done: Vec<i64>,
    pub user_done: Vec<i64>,
    pub beasts: Vec<crate::beast::Beast>,
    glow_keys: HashSet<String>,
    pub next_object_id: i64,
    pub legacy_color_level: bool,
    pub is_editor: bool,
    pub ldm: bool,
    /// `_groupSprites`: sprite indices per group, in registration order (may repeat)
    pub group_sprites: BTreeMap<i64, Vec<usize>>,
    /// pad particle emitters (`_cullableEmitters`), as indices into `sprites`
    pub emitters: Vec<usize>,
    /// whether pads get their emitter as a pseudo-sprite (off for the plain sprite comparison)
    pub pad_emitters: bool,
    pub lists: SpriteLists,
    /// the emitter of the last collider created before the current object (`this.objects[length - 1]._padParticleEmitter`)
    pub prev_pad_emitter: Option<usize>,
    /// `_colorChannelSprites`
    pub color_channel_sprites: BTreeMap<i64, Vec<usize>>,
    /// `_channelBlending` as `_setUpSettings` left it
    pub channel_blending: BTreeMap<i64, bool>,
    persp_cache: HashMap<i64, Option<BTreeMap<usize, f64>>>,
    pub rng: &'a mut dyn FnMut() -> f64,
}

fn child_vec(d: &Json) -> &[Json] { d.get("children").and_then(Json::arr).unwrap_or(&[]) }
fn truthy_str(s: Option<&str>) -> bool { matches!(s, Some(v) if !v.is_empty()) }
fn re_persp(f: &str) -> bool { f.starts_with("persp_") }
fn re_persp_outline(f: &str) -> bool { f.starts_with("persp_outline_") }

impl<'a> Builder<'a> {
    pub fn new(atlases: &'a Atlases, rng: &'a mut dyn FnMut() -> f64) -> Builder<'a> {
        Builder { atlases, sprites: Vec::new(), spawn_order: 0.0, enter_channel: 0.0, parent_groups: Vec::new(), secret_slots: 0, user_slots: 0, secret_done: Vec::new(), user_done: Vec::new(), beasts: Vec::new(), glow_keys: HashSet::new(), next_object_id: 0, legacy_color_level: false, is_editor: false, ldm: false, group_sprites: BTreeMap::new(), emitters: Vec::new(), pad_emitters: false, lists: SpriteLists::default(), prev_pad_emitter: None, color_channel_sprites: BTreeMap::new(), channel_blending: [(1007i64, true)].into_iter().collect(), persp_cache: HashMap::new(), rng }
    }

    /// The particle emitter of the pads that have one (`_spawnObject`'s pad branch): it only matters to the sprite world as a
    /// group member with a position that follows the culling (`_cullEmitters`).
    fn spawn_pad_emitter(&mut self, o: &LevelObject, def: &Json, id: i64) {
        if def.get_str("type") != Some("pad") { return; }
        if !self.pad_emitters || !matches!(id, 35 | 67 | 140 | 1332 | 3005) || self.ldm || self.is_editor { return; }
        let rot_deg = (((o.rot % 360.0) + 360.0) % 360.0);
        let upside_down = rot_deg == 180.0 || (o.flip_y && rot_deg == 0.0);
        let (mut off_x, mut off_y) = (0.0, -2.0);
        if upside_down { off_y = 2.0; }
        else if rot_deg == 90.0 { off_x = 2.0; }
        else if rot_deg == 270.0 { off_x = -2.0; }
        let (wx, wy) = (o.x * 2.0, o.y * 2.0);
        let mut e = Sprite::new("", "__pad_emitter__", wx + off_x, T - wy + off_y);
        e.set_flag("_isEmitter", true);
        let gids: Vec<f64> = if o.groups.is_empty() { Vec::new() } else { o.groups.split('.').map(crate::js::to_number).filter(|n| *n > 0.0).collect() };
        let si = self.sprites.len();
        if !gids.is_empty() {
            e.props.insert("_eeGroups", Val::Arr(gids.clone()));
        }
        self.sprites.push(e);
        self.emitters.push(si);
        for g in gids { self.group_sprites.entry(g as i64).or_default().push(si); }
    }

    fn reg_color(&mut self, reg: &Reg, si: usize, ch: f64, force_parent: bool, col1: f64) {
        if let Some(c) = reg.color(&mut self.sprites[si], ch, force_parent, col1, &self.channel_blending) {
            self.color_channel_sprites.entry(c as i64).or_default().push(si);
        }
    }

    fn reg_groups(&mut self, reg: &Reg, si: usize, bx: f64, by: f64) {
        reg.groups(&mut self.sprites[si], bx, by);
        if let Some(g) = &reg.obj_gids {
            let mut uniq: Vec<i64> = Vec::new();
            for v in g { if *v > 0 && !uniq.contains(v) { uniq.push(*v); } }
            for gid in uniq { self.group_sprites.entry(gid).or_default().push(si); }
        }
    }

    fn add_image(&mut self, x: f64, y: f64, frame: &str) -> Option<usize> {
        let (atlas, _) = self.atlases.find(frame)?;
        let s = Sprite::new(atlas, frame, x, y);
        self.sprites.push(s);
        Some(self.sprites.len() - 1)
    }

    /// `_applyVisualProps(scene, sprite, frameName, objectData, colorData)`; `rot` is `objectData.rot` (children pass their own).
    fn apply_visual_props(&mut self, si: usize, frame_name: &str, o: &LevelObject, rot: f64, color: Option<&Json>) {
        let info = self.atlases.find(frame_name);
        let pixel_sheet_scale = if matches!(info, Some(("PixelSheet_01-hd", _))) { 0.5 } else { 1.0 };
        let (mut ox, mut oy) = match info {
            Some((_, f)) => {
                let (sx, sy) = if f.has_sprite_source { (f.src_x, f.src_y) } else { (0.0, 0.0) };
                (f.real_w / 2.0 - (sx + f.width / 2.0), f.real_h / 2.0 - (sy + f.height / 2.0))
            }
            None => (0.0, 0.0),
        };
        let line_key = frame_name.strip_suffix("_glow_001.png").or_else(|| frame_name.strip_suffix("_001.png")).unwrap_or(frame_name);
        let mut is_persp_line = false;
        if line_key == "block008_topcolor_15" {
            oy = -9.5;
            is_persp_line = true;
        } else if (frame_name == "persp_outline_03_001.png" || frame_name == "persp_outline_02_001.png") && matches!(o.id, Some(507) | Some(508)) {
            oy = 0.0;
            is_persp_line = true;
        }
        if o.flip_x { self.sprites[si].flip_x = true; ox = -ox; }
        if o.flip_y { self.sprites[si].flip_y = true; oy = -oy; }
        let total_rotation = 0.0 + rot; // `getData("gjBaseRotationDeg")` is never set
        let (mut sx, mut sy) = gd_object_scale(o);
        sx *= pixel_sheet_scale;
        sy *= pixel_sheet_scale;
        let warp = gd_object_warp(o);
        if total_rotation != 0.0 || sx != 1.0 || sy != 1.0 || warp != 0.0 {
            let (px, py) = transform_local_point(ox, oy, total_rotation, sx, sy, warp);
            ox = px; oy = py;
        }
        let s = &mut self.sprites[si];
        if is_persp_line { s.set_num("_eeVisOffX", ox); s.set_num("_eeVisOffY", oy); }
        s.x += ox;
        s.y += oy;
        // applyGDObjectTransform (the warped branch only records the warp; drawing applies it)
        if warp == 0.0 || warp.is_nan() {
            if total_rotation != 0.0 && !total_rotation.is_nan() { s.set_angle(total_rotation); }
            if sx != 1.0 || sy != 1.0 { s.scale_x = sx; s.scale_y = sy; }
            s.set_num("_eeBaseScaleX", sx);
            s.set_num("_eeBaseScaleY", sy);
        } else {
            s.set_num("_eeBaseScaleX", 1.0);
            s.set_num("_eeBaseScaleY", 1.0);
            let (rx, ry) = warp_axes(total_rotation, warp);
            s.props.insert("_gdWarp", Val::Arr(vec![rx, ry, sx, sy]));
        }
        let bx = s.num("_eeBaseScaleX");
        let by = s.num("_eeBaseScaleY");
        if let Some(v) = bx { s.set_num("_eeObjScaleX", v); }
        if let Some(v) = by { s.set_num("_eeObjScaleY", v); }
        if let Some(c) = color {
            let black_default = c.get_bool("black") == Some(true) || c.get_num("tint") == Some(0.0);
            if let Some(t) = c.get_num("tint") { s.tint = Some(t); }
            if black_default {
                s.tint = Some(0.0);
                s.set_flag("_isBlack", true);
                let can = c.get_bool("can_color") == Some(true);
                s.set_flag("_canColor", can);
                s.set_flag("_blackDefault", can && !(o.color1.map_or(false, |c| c > 0)));
            }
        }
    }

    fn add_visual_sprite(&mut self, si: usize, def: Option<&Json>, portal_front: bool) {
        let s = &mut self.sprites[si];
        if let Some(d) = def {
            if d.get_str("blend") == Some("additive") { s.blend = ADD; s.set_num("_eeLayer", 0.0); return; }
        }
        if portal_front { s.set_num("_eeLayer", 2.0); }
        else if def.map_or(false, |d| d.get_num("z").map_or(false, |z| z < 0.0)) { s.set_num("_eeLayer", 0.0); }
        else { s.set_num("_eeLayer", 1.0); }
    }

    fn has_glow(def: Option<&Json>, o: Option<&LevelObject>) -> bool {
        if let Some(o) = o { if o.raw.get("96").map(|s| s.as_str()).unwrap_or("0") == "1" { return false; } }
        match def {
            Some(d) => d.get_bool("glow") == Some(true) || truthy_str(d.get_str("glow_frame")) && d.get_str("glow_frame") != Some("none"),
            None => false,
        }
    }

    fn glow_frame_name(frame: &str, def: Option<&Json>) -> Option<String> {
        if let Some(d) = def { if let Some(g) = d.get_str("glow_frame") { if !g.is_empty() && g != "none" { return Some(g.to_string()); } } }
        if frame.ends_with("_001.png") { return Some(frame.replace("_001.png", "_glow_001.png")); }
        None
    }

    fn add_glow_sprite(&mut self, x: f64, y: f64, frame: &str, o: &LevelObject, rot: f64, world_x: Option<f64>, def: Option<&Json>) -> Option<usize> {
        let glow_frame = Self::glow_frame_name(frame, def)?;
        if glow_frame == frame { return None; }
        let r = if rot.is_finite() { rot } else { 0.0 };
        let key = format!("{}:{}:{}:{}", num(world_x.unwrap_or(x)), num(y), glow_frame, crate::physics::js_round(r * 1000.0) as i64);
        if self.glow_keys.contains(&key) { return None; }
        let gi = self.add_image(x, y, &glow_frame)?;
        self.glow_keys.insert(key);
        self.apply_visual_props(gi, &glow_frame, o, rot, None);
        let black = def.map_or(false, |d| d.get_bool("black") == Some(true) || d.get_num("tint") == Some(0.0));
        let s = &mut self.sprites[gi];
        if let Some(t) = def.and_then(|d| d.get_num("tint")) { s.tint = Some(t); }
        if black {
            s.tint = Some(0.0);
            s.set_flag("_isBlack", true);
            let can = def.map_or(false, |d| d.get_bool("can_color") == Some(true));
            s.set_flag("_canColor", can);
            s.set_flag("_blackDefault", can && !(o.color1.map_or(false, |c| c > 0)));
        }
        s.blend = ADD;
        s.set_alpha(0.425);
        let a = s.alpha;
        s.set_num("_eeOrigAlpha", a);
        s.set_num("_eeLayer", 0.0);
        s.set_flag("_eeIsGlowSprite", true);
        s.visible = !self.ldm; // `_isGlowVisible()`
        if let Some(wx) = world_x { s.set_num("_eeWorldX", wx); s.set_num("_eeBaseY", y); }
        Some(gi)
    }

    fn sync_glow_color(&mut self, gi: usize, si: usize) {
        let (tint, black, can, bd, saw_color, is_saw) = { let o = &self.sprites[si]; (o.tint_top_left(), o.flag("_isBlack"), o.flag("_canColor"), o.flag("_blackDefault"), o.num("_SawColor"), o.flag("_isSaw")) };
        let g = &mut self.sprites[gi];
        g.tint = Some(tint);
        g.set_flag("_isBlack", black);
        g.set_flag("_canColor", can);
        g.set_flag("_blackDefault", bd);
        match saw_color { Some(v) => g.set_num("_SawColor", v), None => { g.props.remove("_SawColor"); } }
        g.set_flag("_isSaw", is_saw);
    }

    fn persp_child_offsets(&mut self, id: i64, d: &Json) -> Option<BTreeMap<usize, f64>> {
        if let Some(c) = self.persp_cache.get(&id) { return c.clone(); }
        let children = child_vec(d);
        let persp: Vec<usize> = children.iter().enumerate().filter(|(_, c)| c.get_str("frame").map_or(false, re_persp) && c.get("localDx").is_none()).map(|(i, _)| i).collect();
        let fill: Vec<usize> = persp.iter().copied().filter(|&i| !children[i].get_str("frame").map_or(false, re_persp_outline) && children[i].get("localDy").is_some()).collect();
        let mut result: Option<BTreeMap<usize, f64>> = None;
        if persp.len() != 2 && (fill.len() == 3 || fill.len() == 4) {
            let run = if fill.len() == 4 { 2.0 } else { 1.0 };
            let mut r = BTreeMap::new();
            for &i in &fill { r.insert(i, children[i].get_num("localDy").unwrap_or(0.0) * run); }
            result = Some(r);
        }
        if persp.len() == 2 {
            let width_of = |fr: Option<&str>| -> f64 { match fr { Some(f) if f != "none" && !f.is_empty() => self.atlases.find(f).map_or(0.0, |(_, a)| a.real_w), _ => 0.0 } };
            let lw = width_of(children[persp[0]].get_str("frame"));
            let rw = width_of(children[persp[1]].get_str("frame"));
            let pw = width_of(d.get_str("frame"));
            if lw > 0.0 && rw > 0.0 && (pw == 0.0 || (lw + rw - pw).abs() <= 2.0) {
                let mut r = BTreeMap::new();
                r.insert(persp[0], -lw / 2.0);
                r.insert(persp[1], rw / 2.0);
                result = Some(r);
            }
        }
        self.persp_cache.insert(id, result.clone());
        result
    }

    fn add_to_section(&mut self, si: usize) {
        let s = &mut self.sprites[si];
        s.set_num("_eeEnterChannel", self.enter_channel);
        if !self.parent_groups.is_empty() { s.props.insert("_eeParentGroups", Val::Arr(self.parent_groups.clone())); }
        let wx = s.num("_eeWorldX").unwrap_or(f64::NAN);
        let idx = 0f64.max((or0(wx) / 400.0).floor());
        s.set_num("_eeSectionIndex", idx);
        s.set_num("_eeSpawnOrder", self.spawn_order);
        self.spawn_order += 1.0;
    }

    fn register_animated(&mut self, si: usize, d: &Json) {
        let Some(frames) = d.get("animFrames").and_then(Json::arr) else { return };
        if frames.is_empty() { return; }
        let interval = match d.get_num("animInterval") { Some(v) if v != 0.0 => v, _ => 100.0 };
        let r = (self.rng)();
        self.lists.animated.push(si);
        let s = &mut self.sprites[si];
        s.props.insert("_animFrames", Val::List(frames.iter().filter_map(|f| f.str().map(String::from)).collect()));
        s.set_num("_animInterval", interval);
        s.set_num("_animIdx", 0.0);
        s.set_num("_lastAnimSwap", 0.0 - r * interval);
    }

    fn saw_registered(&mut self, si: usize) {
        if !self.sprites[si].flag("_eeSawRegistered") { self.lists.saws.push(si); }
        let s = &mut self.sprites[si];
        s.set_flag("_eeSawRegistered", true);
        if s.num("_SawRandom1").is_some() || s.num("_SawRandom2").is_some() { s.set_flag("_animatedsaw", true); }
    }

    /// Spawns the sprites of one (non-trigger) level object.
    pub fn spawn(&mut self, o: &LevelObject) -> std::ops::Range<usize> {
        let start = self.sprites.len();
        self.enter_channel = o.raw.get("343").and_then(|v| crate::js::parse_int(v)).unwrap_or(0) as f64;
        self.parent_groups = o.raw.get("274").map(|v| v.split('.').map(crate::js::to_number).filter(|n| *n > 0.0).collect()).unwrap_or_default();
        let id = o.id.unwrap_or(0);
        let def = match def_of(id) { Some(d) => d, None => { self.next_object_id += 1; return start..start; } };
        let linked = self.next_object_id;
        self.next_object_id += 1;
        let world_x = o.x * 2.0;
        let world_y = o.y * 2.0;
        let mut frame_name: Option<String> = def.get_str("frame").map(String::from);
        if let Some(rf) = def.get("randomFrames").and_then(Json::arr) {
            if !rf.is_empty() { let k = ((self.rng)() * rf.len() as f64).floor() as usize; frame_name = rf.get(k).and_then(|v| v.str()).map(String::from); }
        }
        let secret_slot = if id == 142 { let v = self.secret_slots; self.secret_slots += 1; Some(v) } else { None };
        let user_slot = if id == 1329 { let v = self.user_slots; self.user_slots += 1; Some(v) } else { None };
        let Some(frame_name) = frame_name.filter(|f| !f.is_empty()) else { return start..start; };
        let ty = def.get_str("type").unwrap_or("");
        const SAW: &[i64] = &[183, 184, 185, 186, 187, 188, 397, 398, 399, 678, 679, 680, 740, 741, 742, 1619, 1620, 1708, 1709, 1710, 1734, 1735, 1736, 85, 86, 87, 97, 137, 138, 139, 154, 155, 156, 180, 181, 182, 1019, 1020, 1021, 1058, 1059, 1060, 1061, 1752, 396, 395, 222, 223, 224, 997, 998, 999, 1000, 1055, 1056, 1057, 394, 375, 376, 377, 378, 677, 676, 675];
        const FAST: &[i64] = &[186, 187, 188, 89, 88, 98, 183, 185, 184, 397, 398, 399, 678, 679, 680, 740, 741, 742, 1619, 1620, 1705, 1706, 1707, 1708, 1709, 1710, 1734, 1735, 1736];
        let is_saw_id = SAW.contains(&id);
        let is_fast_saw_id = FAST.contains(&id);

        let sprite_world_x = world_x;
        let base_y = T - world_y;
        let is_portal_front = (ty == "portal" || ty == "speed") && frame_name.contains("_front_");
        let z_layer = match o.z_layer { Some(z) if z != 0 => z, _ => def.get_num("default_z_layer").map_or(0, |v| v as i64) };
        let z_ord = match o.z_order { Some(z) if z != 0 => z, _ => def.get_num("default_z_order").map_or(0, |v| v as i64) };
        let depth_base = match z_layer { -5 => -12.0, -3 => -9.0, -1 => -6.0, 0 => 0.0, 1 => 3.0, 3 => 6.0, 5 => 9.0, 7 => 10.5, 9 => 12.0, 11 => 13.5, _ => 0.0 };
        let obj_z_depth = depth_base + z_ord as f64 * 0.01;

        let mut col1 = match o.color1 { Some(c) if c != 0 => c as f64, _ => def.get_num("default_base_color_channel").unwrap_or(0.0) };
        if !o.color1.map_or(false, |c| c != 0) && !self.legacy_color_level { if let Some(c) = def.get_num("modern_base_color_channel") { col1 = c; } }
        if col1 == 0.0 && (ty == "solid" || ty == "hazard") { col1 = 1004.0; }
        let col2 = match o.color2 { Some(c) if c != 0 => c as f64, _ => def.get_num("default_detail_color_channel").unwrap_or(-1.0) };
        let has_detail_channel = def.get_num("default_detail_color_channel").map_or(false, |v| v > 0.0) && col2 > 0.0;
        let is_detail_child = |c: &Json| has_detail_channel && c.get_bool("black") != Some(true) && matches!(c.get_num("tint"), Some(t) if t != 0.0);
        let explicit_col1 = o.color1.map_or(false, |c| c > 0);
        let can_color = def.get_bool("can_color") != Some(false) || explicit_col1;
        let obj_gids: Option<Vec<i64>> = if o.groups.is_empty() { None } else { Some(o.groups.split('.').map(crate::js::to_number).filter(|n| *n > 0.0).map(|n| n as i64).collect()) };
        let invisible_frame = |f: Option<&str>| f.map_or(false, |f| f.starts_with("invis_") || f.starts_with("invisibleOutline_"));
        let has_invisible = invisible_frame(def.get_str("frame")) || invisible_frame(def.get_str("glow_frame")) || child_vec(def).iter().any(|c| invisible_frame(c.get_str("frame")));
        let raw = |k: &str| o.raw.get(k).map(|s| s.as_str());
        let hsv_for = |flag: &str, val: &str| if raw(flag).unwrap_or("0") == "1" && raw(val).map_or(false, |v| !v.is_empty()) { crate::colors::parse_channel_hsv(raw(val)) } else { None };
        let reg = Reg {
            has_detail: has_detail_channel, col2, hsv1: hsv_for("41", "43"), hsv2: hsv_for("42", "44"),
            invisible: has_invisible, can_color, depth_base, obj_gids, linked,
            bits: (
                o.raw.get("64").map(|s| s.as_str()).unwrap_or("0") == "1",
                o.raw.get("67").map(|s| s.as_str()).unwrap_or("0") == "1",
                o.editor_layer.unwrap_or_else(|| crate::js::parse_int(o.raw.get("20").map(|s| s.as_str()).unwrap_or("0")).unwrap_or(0)),
                o.editor_layer2.unwrap_or_else(|| crate::js::parse_int(o.raw.get("61").map(|s| s.as_str()).unwrap_or("0")).unwrap_or(0)),
            ),
        };
        let level_scale = { let v = or0(o.scale); if v == 0.0 { 1.0 } else { v } };

        // ---- portal back sprite
        let mut portal_back: Option<usize> = None;
        if is_portal_front {
            let back = frame_name.replace("_front_", "_back_");
            if let Some(bi) = self.add_image(sprite_world_x, base_y, &back) {
                self.apply_visual_props(bi, &back, o, o.rot, None);
                let s = &mut self.sprites[bi];
                s.set_num("_eeLayer", 1.0);
                s.set_num("_eeWorldX", world_x);
                s.set_num("_eeBaseY", base_y);
                s.set_num("_eeZDepth", obj_z_depth - 0.005);
                s.set_num("_eeOrigAlpha", 1.0);
                self.add_to_section(bi);
                self.reg_groups(&reg, bi, world_x, base_y);
                self.reg_color(&reg, bi, col1, false, col1);
                reg.object(&mut self.sprites[bi]);
                portal_back = Some(bi);
            }
        }

        // ---- glow of the main sprite
        let mut orb_glow: Option<usize> = None;
        if Self::has_glow(Some(def), Some(o)) {
            if let Some(gi) = self.add_glow_sprite(sprite_world_x, base_y, &frame_name, o, o.rot, Some(world_x), Some(def)) {
                self.add_to_section(gi);
                let s = &mut self.sprites[gi];
                s.set_num("_eeZDepth", obj_z_depth - 0.003);
                let a = s.alpha;
                s.set_num("_eeOrigAlpha", a);
                self.reg_color(&reg, gi, col1, is_saw_id, col1);
                self.reg_groups(&reg, gi, world_x, base_y);
                reg.object(&mut self.sprites[gi]);
                orb_glow = Some(gi);
            }
        }

        // ---- the main sprite
        let main = self.add_image(sprite_world_x, base_y, &frame_name);
        'main: {
        let Some(si) = main else { break 'main };
        self.apply_visual_props(si, &frame_name, o, o.rot, Some(def));
        if let Some(pb) = portal_back { let (x, y) = (self.sprites[si].x, self.sprites[si].y); self.sprites[pb].x = x; self.sprites[pb].y = y; }
        self.add_visual_sprite(si, Some(def), is_portal_front);
        {
            let s = &mut self.sprites[si];
            s.set_num("_eeWorldX", world_x);
            s.set_num("_eeBaseY", base_y);
            s.set_num("_eeZDepth", obj_z_depth);
            let orig = def.get_num("alpha").unwrap_or(1.0);
            s.set_num("_eeOrigAlpha", orig);
            if orig != 1.0 { s.set_alpha(orig); }
            if ty == "dblock" || matches!(id, 1813 | 1829 | 1859) { s.set_flag("_eeEditorOnly", true); s.visible = self.is_editor; }
        }
        if is_saw_id {
            let decorative = ty == "deco" && frame_name.contains("sawblade");
            self.sprites[si].set_flag("_isSaw", true);
            if self.sprites[si].num("_Sawrotationspeed").is_none() {
                let base = if is_fast_saw_id { 0.0063 } else { 0.0032 };
                let boosted = if decorative { base * 1.15 } else { base };
                let sign = if (self.rng)() < 0.5 { -1.0 } else { 1.0 };
                let spd = sign * (boosted + ((self.rng)() * boosted * 0.2 - boosted * 0.1));
                self.sprites[si].set_num("_Sawrotationspeed", spd);
            }
            let r1 = (self.rng)() * PI * 2.0;
            let r2 = if decorative { if is_fast_saw_id { 0.00075 } else { 0.00045 } } else if is_fast_saw_id { 0.00065 } else { 0.0004 };
            self.sprites[si].set_num("_SawRandom1", r1);
            self.sprites[si].set_num("_SawRandom2", r2);
            self.sprites[si].set_num("_Sawoffset", 0.0);
            self.saw_registered(si);
        }
        self.reg_color(&reg, si, col1, is_saw_id, col1);
        self.add_to_section(si);
        reg.object(&mut self.sprites[si]);
        if let Some(g) = &reg.obj_gids { if !g.is_empty() { self.sprites[si].props.insert("_eeGroups", Val::Arr(g.iter().map(|v| *v as f64).collect())); } }
        { let (wx, by) = (self.sprites[si].num("_eeWorldX").unwrap_or(f64::NAN), self.sprites[si].num("_eeBaseY").unwrap_or(f64::NAN)); if reg.obj_gids.as_ref().map_or(false, |g| !g.is_empty()) { self.reg_groups(&reg, si, wx, by); } }
        self.register_animated(si, def);
        // the pad sprite links to the emitter of the most recent collider (not necessarily its own: the original creates sprites before colliders)
        if ty == "pad" { if let Some(e) = self.prev_pad_emitter { self.sprites[si].set_num("_padEmitter", e as f64); } }

        if ty == "ring" {
            let sc = 0.75 * level_scale;
            self.lists.orbs.push(si);
            let s = &mut self.sprites[si];
            s.scale_x = sc; s.scale_y = sc;
            s.set_num("_eeBaseScale", sc);
            s.set_flag("_eeAudioScale", true);
            s.set_num("_orbId", id as f64);
            if frame_name.contains("dropRing") || frame_name.contains("gravJumpRing") {
                s.set_flag("_isSaw", true);
                if is_fast_saw_id { s.set_num("_Sawrotationspeed", 0.0065); }
                self.saw_registered(si);
            }
            if let Some(gi) = orb_glow {
                let g = &mut self.sprites[gi];
                g.scale_x = sc; g.scale_y = sc;
                g.set_num("_eeBaseScale", sc);
                g.set_flag("_eeAudioScale", true);
                g.set_num("_orbId", id as f64);
                self.lists.orbs.push(gi);
            }
        }
        if def.get_bool("audioPulse") == Some(true) {
            self.lists.audio_pulse.push(si);
            if let Some(gi) = orb_glow { self.lists.audio_pulse.push(gi); }
        }
        let _ = (secret_slot, user_slot);
        if ty == "coin" {
            let (sx, ) = (self.sprites[si].scale_x, );
            let s = &mut self.sprites[si];
            s.set_num("_coinWorldX", world_x);
            s.set_num("_coinWorldY", world_y);
            s.set_num("_coinBaseScale", if sx == 0.0 { 1.0 } else { sx });
            self.lists.coins.push(si);
            const SECRET: [&str; 4] = ["secretCoin_01_001.png", "secretCoin_01_002.png", "secretCoin_01_003.png", "secretCoin_01_004.png"];
            const USER: [&str; 4] = ["secretCoin_2_01_001.png", "secretCoin_2_01_002.png", "secretCoin_2_01_003.png", "secretCoin_2_01_004.png"];
            let anim = if id == 142 { s.set_num("_secretCoinId", linked as f64); s.set_num("_secretCoinSlot", secret_slot.unwrap_or(0) as f64); Some((SECRET, "_secretCoinCompleted")) }
                       else if id == 1329 { s.set_num("_userCoinId", linked as f64); s.set_num("_userCoinSlot", user_slot.unwrap_or(0) as f64); Some((USER, "_userCoinCompleted")) } else { None };
            if let Some((frames, flag)) = anim {
                let collected = if id == 142 { self.secret_done.contains(&linked) } else { self.user_done.contains(&linked) };
                const SECRET_B: [&str; 4] = ["secretCoin_b_01_001.png", "secretCoin_b_01_002.png", "secretCoin_b_01_003.png", "secretCoin_b_01_004.png"];
                const USER_B: [&str; 4] = ["secretCoin_2_b_01_001.png", "secretCoin_2_b_01_002.png", "secretCoin_2_b_01_003.png", "secretCoin_2_b_01_004.png"];
                let frames: [&str; 4] = if collected { if id == 142 { SECRET_B } else { USER_B } } else { frames };
                s.props.insert("_animFrames", Val::List(frames.iter().map(|f| f.to_string()).collect()));
                s.set_num("_animIdx", 0.0);
                s.set_flag(flag, collected);
                let first = frames[0];
                if let Some((atlas, _)) = self.atlases.find(first) { let a = atlas.to_string(); let s = &mut self.sprites[si]; s.atlas = a; s.frame = first.to_string(); }
            }
        }
        let is_deco_saw_frame = frame_name.contains("sawblade");
        if let Some(gi) = orb_glow {
            if is_saw_id || is_deco_saw_frame {
                let (rs, r1, r2) = { let s = &self.sprites[si]; (s.num("_Sawrotationspeed"), s.num("_SawRandom1"), s.num("_SawRandom2")) };
                self.sprites[gi].set_flag("_isSaw", true);
                if let Some(rs) = rs {
                    let g = &mut self.sprites[gi];
                    g.set_num("_Sawrotationspeed", rs);
                    if let Some(v) = r1 { g.set_num("_SawRandom1", v); }
                    if let Some(v) = r2 { g.set_num("_SawRandom2", v); }
                } else {
                    let decorative = ty == "deco" && is_deco_saw_frame;
                    let base = if is_fast_saw_id { 0.0065 } else { 0.0034 };
                    let boosted = if decorative { base * 1.15 } else { base };
                    let sign = if (self.rng)() < 0.5 { -1.0 } else { 1.0 };
                    let spd = sign * (boosted + ((self.rng)() * boosted * 0.2 - boosted * 0.1));
                    let r1 = (self.rng)() * PI * 2.0;
                    let r2 = if decorative { if is_fast_saw_id { 0.00075 } else { 0.00045 } } else if is_fast_saw_id { 0.00065 } else { 0.0004 };
                    let g = &mut self.sprites[gi];
                    g.set_num("_Sawrotationspeed", spd); g.set_num("_SawRandom1", r1); g.set_num("_SawRandom2", r2);
                }
                let off = self.sprites[gi].rotation - self.sprites[si].rotation;
                self.sprites[gi].set_num("_Sawoffset", off);
                self.saw_registered(gi);
            }
        }
        if is_deco_saw_frame {
            self.sprites[si].tint = Some(0.0);
            self.sprites[si].set_flag("_isBlack", true);
            self.sprites[si].set_flag("_isSaw", true);
            let decorative = ty == "deco";
            if is_fast_saw_id || decorative {
                let target = (if is_fast_saw_id { 0.0065 } else { 0.0034 }) * if decorative { 1.15 } else { 1.0 };
                self.sprites[si].set_num("_Sawrotationspeed", target);
            }
            self.saw_registered(si);
            if let Some(mi) = self.add_image(sprite_world_x, base_y, &frame_name) {
                self.apply_visual_props(mi, &frame_name, o, o.rot, Some(def));
                let (srot, szd, slayer, srs, sr1, sr2) = { let s = &self.sprites[si]; (s.rotation, s.num("_eeZDepth"), s.num("_eeLayer"), s.num("_Sawrotationspeed"), s.num("_SawRandom1"), s.num("_SawRandom2")) };
                let mir = &mut self.sprites[mi];
                mir.tint = Some(0.0);
                mir.set_flag("_isBlack", true);
                mir.rotation = srot + PI;
                mir.set_flag("_isSaw", true);
                if let Some(z) = szd { mir.set_num("_eeZDepth", z); }
                mir.set_num("_eeLayer", slayer.unwrap_or(1.0));
                mir.set_flag("_eeBehindParent", true);
                if let Some(rs) = srs {
                    mir.set_num("_Sawrotationspeed", rs);
                    if let Some(v) = sr1 { mir.set_num("_SawRandom1", v); }
                    if let Some(v) = sr2 { mir.set_num("_SawRandom2", v); }
                    let off = mir.rotation - srot;
                    mir.set_num("_Sawoffset", off);
                } else {
                    let decorative_s = ty == "deco";
                    let base = if is_fast_saw_id { 0.0065 } else { 0.0034 };
                    let boosted = if decorative_s { base * 1.15 } else { base };
                    let sign = if (self.rng)() < 0.5 { -1.0 } else { 1.0 };
                    let spd = sign * (boosted + ((self.rng)() * boosted * 0.2 - boosted * 0.1));
                    let r1 = (self.rng)() * PI * 2.0;
                    let r2 = if decorative_s { if is_fast_saw_id { 0.00075 } else { 0.00045 } } else if is_fast_saw_id { 0.0007 } else { 0.0004 };
                    let mir = &mut self.sprites[mi];
                    mir.set_num("_Sawrotationspeed", spd); mir.set_num("_SawRandom1", r1); mir.set_num("_SawRandom2", r2);
                }
                self.reg_color(&reg, mi, col1, true, col1);
                self.sprites[mi].set_num("_eeWorldX", world_x);
                self.sprites[mi].set_num("_eeBaseY", base_y);
                self.add_to_section(mi);
                self.add_visual_sprite(mi, None, false);
                self.saw_registered(mi);
                self.reg_groups(&reg, mi, world_x, base_y);
                reg.object(&mut self.sprites[mi]);
            }
        }
        if let Some(gi) = orb_glow { self.sync_glow_color(gi, si); }
        }

        // ---- overlay sprite of solids and hazards
        if ty == "solid" || ty == "hazard" {
            let overlay = frame_name.replace("_001.png", "_2_001.png");
            if self.atlases.find(&overlay).is_some() {
                if let Some(oi) = self.add_image(sprite_world_x, base_y, &overlay) {
                    self.apply_visual_props(oi, &overlay, o, o.rot, None);
                    self.add_visual_sprite(oi, None, false);
                    let s = &mut self.sprites[oi];
                    s.set_num("_eeWorldX", world_x);
                    s.set_num("_eeBaseY", base_y);
                    s.set_num("_eeZDepth", obj_z_depth + 0.002);
                    s.set_num("_eeOrigAlpha", 1.0);
                    let oc2 = if col2 <= 0.0 { 2.0 } else { col2 };
                    self.reg_color(&reg, oi, oc2, false, col1);
                    self.add_to_section(oi);
                    self.reg_groups(&reg, oi, world_x, base_y);
                    reg.object(&mut self.sprites[oi]);
                }
            }
        }
        // children are added by `spawn_children` (below)
        let coin_animation = ty == "coin";
        self.spawn_children(o, def, id, &frame_name, main, world_x, base_y, sprite_world_x, obj_z_depth, col1, col2, is_saw_id, is_fast_saw_id, &reg, &is_detail_child, level_scale);
        if crate::beast::IDS.contains(&id) { self.spawn_beast(o, def, &reg, start, world_x, base_y, obj_z_depth, col1, col2); }
        if id == 747 { self.spawn_teleport_exit(o, def, &reg, obj_z_depth, col1); }
        // portal particles are added to a section too (they take a spawn-order number)
        if def.get_bool("portalParticle") == Some(true) && !self.ldm && !self.is_editor { self.spawn_order += 1.0; }
        let _ = coin_animation;
        self.spawn_pad_emitter(o, def, id);
        start..self.sprites.len()
    }
}

/// `registerColor` / `registerToGroups` / `registerObjectSprite` of `_spawnObject`.
struct Reg { has_detail: bool, col2: f64, hsv1: Option<crate::colors::Hsv>, hsv2: Option<crate::colors::Hsv>, invisible: bool, can_color: bool, depth_base: f64, obj_gids: Option<Vec<i64>>, linked: i64, bits: (bool, bool, i64, i64) }
impl Reg {
    fn color(&self, s: &mut Sprite, ch: f64, force_parent: bool, col1: f64, blending: &BTreeMap<i64, bool>) -> Option<f64> {
        if s.flag("_cantColor") || (s.flag("_isBlack") && !s.flag("_canColor")) { return None; }
        if ch > 0.0 && (self.can_color || s.flag("_canColor")) {
            let mut ch = ch;
            let spr_hsv = if self.has_detail && ch == self.col2 && self.col2 != col1 { self.hsv2 } else if ch == col1 { self.hsv1 } else { None };
            if ch == 1012.0 && col1 > 0.0 && col1 != 1012.0 { s.set_flag("_eeLighter", true); ch = col1; }
            s.set_num("_eeColorChannel", ch);
            s.set_flag("_eeIsDetail", self.has_detail && ch == self.col2 && self.col2 != col1);
            if let Some(h) = spr_hsv { s.props.insert("_eeHsv", Val::Arr(vec![h.h, h.s, h.v, h.s_add as u8 as f64, h.v_add as u8 as f64])); }
            if s.num("_eeBaseBlend").is_none() { let b = s.blend as f64; s.set_num("_eeBaseBlend", b); }
            if s.num("_eeLayerBase").is_none() { s.set_num("_eeLayerBase", self.depth_base); }
            if blending.get(&(ch as i64)).copied().unwrap_or(false) || (ch >= 1005.0 && ch <= 1007.0) { s.blend = ADD; s.set_flag("_eeChannelAdditive", true); }
            if force_parent && s.num("_SawColor").is_none() { s.set_num("_SawColor", ch); }
            return Some(ch);
        }
        None
    }
    fn groups(&self, s: &mut Sprite, bx: f64, by: f64) {
        let Some(g) = &self.obj_gids else { return };
        if g.is_empty() { return; }
        let mut uniq: Vec<f64> = Vec::new();
        for v in g { let f = *v as f64; if f > 0.0 && !uniq.contains(&f) { uniq.push(f); } }
        s.props.insert("_eeGroups", Val::Arr(uniq));
        s.set_num("_origWorldX", bx); s.set_num("_origBaseY", by);
        s.set_num("_eeMoveBaseWorldX", bx); s.set_num("_eeMoveBaseBaseY", by);
        if s.num("_eeInitialWorldX").is_none() { s.set_num("_eeInitialWorldX", bx); }
        if s.num("_eeInitialBaseY").is_none() { s.set_num("_eeInitialBaseY", by); }
        if s.num("_eeInitialRotationRad").is_none() { let r = s.rotation; s.set_num("_eeInitialRotationRad", r); }
        let _ = ADD;
    }
    fn object(&self, s: &mut Sprite) {
        s.set_num("_eeObjectId", self.linked as f64);
        if self.invisible { s.set_num("_eeApproachAlpha", 0.0); s.set_alpha(0.0); }
        s.set_flag("_eeDontFade", self.bits.0);
        s.set_flag("_eeDontEnter", self.bits.1);
        s.set_num("_eeEditorLayer", self.bits.2 as f64);
        s.set_num("_eeEditorLayer2", self.bits.3 as f64);
    }
}

impl<'a> Builder<'a> {
    /// `_spawnTeleportExitPortalVisual`: the exit half of a teleport portal (object 747) is drawn where the entry's Y offset says.
    fn spawn_teleport_exit(&mut self, enter: &LevelObject, enter_def: &Json, reg: &Reg, obj_z_depth: f64, col1: f64) {
        let offset_y = match enter.raw.get("54") { Some(s) => crate::js::parse_float(s).filter(|v| v.is_finite()).unwrap_or(90.0), None => 90.0 };
        let exit_def = def_of(749).unwrap_or(enter_def);
        let Some(frame_name) = exit_def.get_str("frame").or(enter_def.get_str("frame")).filter(|f| !f.is_empty()).map(String::from) else { return };
        let enter_x = or0(enter.x);
        let enter_y = or0(enter.y);
        let exit_y = enter_y + offset_y;
        let exit_rot = or0(enter.rot);
        let world_x = enter_x * 2.0;
        let world_y = exit_y * 2.0;
        let base_y = T - world_y;
        let mut exit_obj = enter.clone();
        exit_obj.id = Some(749);
        exit_obj.y = exit_y;
        exit_obj.rot = exit_rot;
        exit_obj.flip_x = true;
        let mut back: Option<usize> = None;
        if frame_name.contains("_front_") {
            let back_frame = frame_name.replace("_front_", "_back_");
            if let Some(bi) = self.add_image(world_x, base_y, &back_frame) {
                self.apply_visual_props(bi, &back_frame, &exit_obj, exit_rot, Some(exit_def));
                let s = &mut self.sprites[bi];
                s.set_num("_eeLayer", 1.0); s.set_num("_eeWorldX", world_x); s.set_num("_eeBaseY", base_y);
                s.set_num("_eeZDepth", obj_z_depth - 0.004); s.set_num("_eeOrigAlpha", 1.0); s.set_flag("_eeGeneratedTeleportExit", true);
                self.add_to_section(bi);
                self.reg_groups(&reg, bi, world_x, base_y);
                self.reg_color(&reg, bi, col1, false, col1);
                reg.object(&mut self.sprites[bi]);
                back = Some(bi);
            }
        }
        let Some(si) = self.add_image(world_x, base_y, &frame_name) else { return };
        self.apply_visual_props(si, &frame_name, &exit_obj, exit_rot, Some(exit_def));
        if let Some(bi) = back { let (x, y) = (self.sprites[si].x, self.sprites[si].y); self.sprites[bi].x = x; self.sprites[bi].y = y; }
        self.add_visual_sprite(si, Some(exit_def), true);
        let s = &mut self.sprites[si];
        s.set_num("_eeWorldX", world_x); s.set_num("_eeBaseY", base_y); s.set_num("_eeZDepth", obj_z_depth + 0.004); s.set_num("_eeOrigAlpha", 1.0);
        s.set_flag("_eeGeneratedTeleportExit", true);
        self.reg_color(&reg, si, col1, false, col1);
        self.add_to_section(si);
        self.reg_groups(&reg, si, world_x, base_y);
        reg.object(&mut self.sprites[si]);
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_children(&mut self, o: &LevelObject, def: &Json, id: i64, frame_name: &str, si: Option<usize>, world_x: f64, base_y: f64, sprite_world_x: f64, obj_z_depth: f64,
        col1: f64, col2: f64, is_saw_id: bool, is_fast_saw_id: bool, reg: &Reg, is_detail_child: &dyn Fn(&Json) -> bool, level_scale: f64) {
        let children = child_vec(def);
        if children.is_empty() { return; }
        let persp = self.persp_child_offsets(id, def);
        let ty = def.get_str("type").unwrap_or("");
        for (ci, cd) in children.iter().enumerate() {
            let is_editor_guide = self.is_editor && (cd.get_bool("portalGuide") == Some(true) || cd.get_bool("orbGuide") == Some(true));
            if is_editor_guide { continue; }
            let psx = if o.scale_x.is_finite() { o.scale_x } else { level_scale };
            let psy = if o.scale_y.is_finite() { o.scale_y } else { level_scale };
            let mut cdx = cd.get_num("dx").unwrap_or(0.0) * psx;
            let mut cdy = cd.get_num("dy").unwrap_or(0.0) * psy;
            let auto_dx = persp.as_ref().and_then(|p| p.get(&ci)).copied();
            if cd.get("localDx").is_some() || cd.get("localDy").is_some() || auto_dx.is_some() {
                let mut ldx = auto_dx.unwrap_or_else(|| or0(cd.get_num("localDx").unwrap_or(0.0)));
                let mut ldy = or0(cd.get_num("localDy").unwrap_or(0.0));
                if o.flip_x { ldx = -ldx; }
                if o.flip_y { ldy = -ldy; }
                let (cs_x, cs_y) = gd_object_scale(o);
                let (px, py) = transform_local_point(ldx, ldy, or0(o.rot), cs_x, cs_y, gd_object_warp(o));
                cdx = px; cdy = py;
            }
            let child_world_x = world_x + cdx;
            let child_base_y = base_y + cdy;
            let Some(cframe) = cd.get_str("frame").map(String::from) else { continue };
            let Some(ci_s) = self.add_image(sprite_world_x + cdx, base_y + cdy, &cframe) else { continue };
            let (fx, fy) = (o.flip_x, o.flip_y);
            let crot_def = cd.get_num("rot").unwrap_or(0.0);
            let mut crot = or0(o.rot) + crot_def;
            if fx != fy { crot = or0(o.rot) - crot_def; }
            if cframe == "portal_01_extra_2_001.png" || cframe == "portal_02_extra_2_001.png" { crot = 0.0; }
            else if cframe == "blockOutline_14new_001.png" || cframe == "blockOutline_15new_001.png" {
                let mut off = if cframe == "blockOutline_14new_001.png" { -45.0 } else { -26.565 };
                if fx { off = -off; }
                if fy { off = -off; }
                crot += off;
            }
            // `childVisualDef`: the child's own flags, with `can_color` falling back to the parent's for black children
            let black_child = cd.get_bool("black") == Some(true) || cd.get_num("tint") == Some(0.0);
            let child_can_color: Option<bool> = if black_child { cd.get_bool("can_color").or(def.get_bool("can_color")) } else { cd.get_bool("can_color").or(if def.get_str("frame") == Some("none") && cd.get_bool("audioScale") == Some(true) { def.get_bool("can_color") } else { None }) };
            let child_rot_def = or0(cd.get_num("rot").unwrap_or(0.0));
            let quarter = m_round(child_rot_def / 90.0);
            let swapped;
            let o_child = if quarter % 2.0 != 0.0 && (child_rot_def - quarter * 90.0).abs() < 0.01 && gd_object_warp(o) == 0.0 {
                let (psx2, psy2) = gd_object_scale(o);
                if psx2 != psy2 { let mut c = o.clone(); c.scale_x = psy2; c.scale_y = psx2; swapped = c; &swapped } else { o }
            } else { o };
            self.apply_visual_props_child(ci_s, &cframe, o_child, crot, cd, child_can_color);
            self.sprites[ci_s].set_flag("_cantColor", cd.get_bool("Cant_Color") == Some(true));
            self.register_animated(ci_s, cd);
            if child_can_color == Some(true) { self.sprites[ci_s].set_flag("_canColor", true); }
            let show_guide = if cd.get_bool("portalGuide") == Some(true) { !self.is_editor } else { true };
            let show_guide2 = if cd.get_bool("orbGuide") == Some(true) { !self.is_editor } else { true };
            self.sprites[ci_s].visible = show_guide && show_guide2;
            if cd.get_bool("portalGuide") == Some(true) { self.sprites[ci_s].set_flag("_eePortalGuide", true); }
            if cd.get_bool("orbGuide") == Some(true) { self.sprites[ci_s].set_flag("_eeOrbGuide", true); }
            if cd.get_bool("audioScale") == Some(true) {
                let s = &mut self.sprites[ci_s];
                s.scale_x = 0.1; s.scale_y = 0.1;
                s.set_alpha(0.9);
                s.set_flag("_eeAudioScale", true);
                if child_can_color == Some(true) { s.set_flag("_eeAudioTintable", true); }
                self.lists.audio_scale.push(ci_s);
            }
            if cd.get_bool("audioPulse") == Some(true) { self.lists.audio_pulse.push(ci_s); }
            let child_blend: Option<&str> = cd.get_str("blend").or(def.get_str("blend"));
            if matches!(id, 15 | 16 | 17) { self.sprites[ci_s].set_flag("_rodballchild", true); self.sprites[ci_s].set_num("_Rodobjectid", id as f64); }
            let child_z = cd.get_num("z").unwrap_or(-1.0);
            if child_z < 0.0 {
                let s = &mut self.sprites[ci_s];
                s.set_num("_eeLayer", 1.0);
                s.set_flag("_eeBehindParent", true);
                if child_blend == Some("additive") { s.blend = ADD; }
            } else {
                // `_addVisualSprite(childSprite, childVisualSource)`: the source is the child's flags + the fallback blend
                let s = &mut self.sprites[ci_s];
                if child_blend == Some("additive") { s.blend = ADD; s.set_num("_eeLayer", 0.0); }
                else if cd.get_bool("portalGuide") == Some(true) { s.set_num("_eeLayer", 2.0); }
                else if child_z < 0.0 { s.set_num("_eeLayer", 0.0); }
                else { s.set_num("_eeLayer", 1.0); }
            }
            {
                let s = &mut self.sprites[ci_s];
                s.set_num("_eeWorldX", child_world_x);
                s.set_num("_eeBaseY", child_base_y);
                let guide_layer = if cd.get_bool("portalGuide") == Some(true) { 0.0 } else { cd.get_num("z").unwrap_or(-1.0) };
                s.set_num("_eeZDepth", obj_z_depth + (-10f64).max(10f64.min(guide_layer)) * 0.0008);
                let orig = def.get_num("alpha").unwrap_or(1.0);
                s.set_num("_eeOrigAlpha", orig);
                if orig != 1.0 { s.set_alpha(orig); }
            }
            let parent_saw = match si { Some(p) => (self.sprites[p].num("_Sawrotationspeed"), self.sprites[p].num("_SawRandom1"), self.sprites[p].num("_SawRandom2"), self.sprites[p].rotation), None => (None, None, None, 0.0) };
            if is_saw_id {
                let s = &mut self.sprites[ci_s];
                s.set_flag("_isSaw", true);
                if let Some(rs) = parent_saw.0 {
                    s.set_num("_Sawrotationspeed", rs);
                    if let Some(v) = parent_saw.1 { s.set_num("_SawRandom1", v); }
                    if let Some(v) = parent_saw.2 { s.set_num("_SawRandom2", v); }
                }
                let off = s.rotation - parent_saw.3;
                s.set_num("_Sawoffset", off);
                self.saw_registered(ci_s);
            }
            let child_channel = match cd.get("color_channel") { Some(c) => c.num().unwrap_or(f64::NAN), None => if is_detail_child(cd) { col2 } else { col1 } };
            self.reg_color(&reg, ci_s, child_channel, is_saw_id, col1);
            self.add_to_section(ci_s);
            self.reg_groups(&reg, ci_s, child_world_x, child_base_y);
            reg.object(&mut self.sprites[ci_s]);
            let child_glow = Self::has_glow(Some(cd), Some(o)) || Self::has_glow(Some(def), Some(o));
            if child_glow {
                let gdef_for_frame = cd;
                if let Some(gi) = self.add_glow_sprite_child(sprite_world_x + cdx, base_y + cdy, &cframe, o, crot, child_world_x, gdef_for_frame, child_can_color) {
                    let (crot_s, cx, cy, czd, clayer) = { let c = &self.sprites[ci_s]; (c.rotation, c.x, c.y, c.num("_eeZDepth").unwrap_or(f64::NAN), c.num("_eeLayer").unwrap_or(1.0)) };
                    {
                        let g = &mut self.sprites[gi];
                        g.rotation = crot_s; g.x = cx; g.y = cy;
                        g.set_num("_eeZDepth", czd - 0.003);
                        let a = g.alpha;
                        g.set_num("_eeOrigAlpha", a);
                        g.set_num("_eeWorldX", child_world_x);
                        g.set_num("_eeBaseY", child_base_y);
                        g.set_num("_eeLayer", clayer);
                        g.set_flag("_eeBehindParent", true);
                        g.set_flag("_cantColor", cd.get_bool("Cant_Color") == Some(true));
                    }
                    if is_saw_id {
                        self.sprites[gi].set_flag("_isSaw", true);
                        if let Some(rs) = parent_saw.0 {
                            let g = &mut self.sprites[gi];
                            g.set_num("_Sawrotationspeed", rs);
                            if let Some(v) = parent_saw.1 { g.set_num("_SawRandom1", v); }
                            if let Some(v) = parent_saw.2 { g.set_num("_SawRandom2", v); }
                        } else {
                            let base = if is_fast_saw_id { 0.0065 } else { 0.0034 };
                            let sign = if (self.rng)() < 0.5 { -1.0 } else { 1.0 };
                            let spd = sign * (base + ((self.rng)() * base * 0.2 - base * 0.1));
                            let r1 = (self.rng)() * PI * 2.0;
                            let r2 = if is_fast_saw_id { 0.00065 } else { 0.0004 };
                            let g = &mut self.sprites[gi];
                            g.set_num("_Sawrotationspeed", spd); g.set_num("_SawRandom1", r1); g.set_num("_SawRandom2", r2);
                        }
                        let off = self.sprites[gi].rotation - parent_saw.3;
                        self.sprites[gi].set_num("_Sawoffset", off);
                        self.saw_registered(gi);
                    }
                    self.sync_glow_color(gi, ci_s);
                    self.reg_color(&reg, gi, child_channel, is_saw_id, col1);
                    self.reg_groups(&reg, gi, child_world_x, child_base_y);
                    reg.object(&mut self.sprites[gi]);
                    if cd.get_bool("audioPulse") == Some(true) { self.lists.audio_pulse.push(gi); }
                }
            }
            if ty == "ring" && cd.get_bool("orbGuide") == Some(true) {
                let sc = 0.75 * level_scale;
                let s = &mut self.sprites[ci_s];
                s.scale_x = sc; s.scale_y = sc;
                s.set_num("_eeBaseScale", sc);
                s.set_num("_orbId", id as f64);
                s.set_flag("_eeOrbGuide", true);
                let grav = cd.get_str("frame").map_or(false, |f| f.contains("gravring") || f.contains("gravJumpRing") || f.contains("gravRing") || f.contains("gravjumpring"));
                s.set_flag("_OrbGuideGrav", grav);
                let r = s.rotation;
                s.set_num("_OrbGuideRotation", r);
                self.lists.orbs.push(ci_s);
            }
            if frame_name.contains("sawblade") {
                self.sprites[ci_s].tint = Some(0.0);
                self.sprites[ci_s].set_flag("_isBlack", true);
                self.sprites[ci_s].set_flag("_isSaw", true);
                if let Some(rs) = parent_saw.0 {
                    let s = &mut self.sprites[ci_s];
                    s.set_num("_Sawrotationspeed", rs);
                    if let Some(v) = parent_saw.1 { s.set_num("_SawRandom1", v); }
                    if let Some(v) = parent_saw.2 { s.set_num("_SawRandom2", v); }
                }
                self.saw_registered(ci_s);
                if let Some(mi) = self.add_image(sprite_world_x + cdx, base_y + cdy, &cframe) {
                    self.apply_visual_props_child(mi, &cframe, o, crot, cd, child_can_color);
                    let (crs, crot_s, czd, clay, c_rs, c_r1, c_r2) = { let c = &self.sprites[ci_s]; (c.rotation, c.rotation, c.num("_eeZDepth"), c.num("_eeLayer"), c.num("_Sawrotationspeed"), c.num("_SawRandom1"), c.num("_SawRandom2")) };
                    let _ = crot_s;
                    let m_ = &mut self.sprites[mi];
                    m_.tint = Some(0.0);
                    m_.set_flag("_isBlack", true);
                    m_.rotation = crs + PI;
                    m_.set_flag("_isSaw", true);
                    if let Some(z) = czd { m_.set_num("_eeZDepth", z); }
                    m_.set_num("_eeLayer", clay.unwrap_or(1.0));
                    m_.set_flag("_eeBehindParent", true);
                    if let Some(rs) = c_rs {
                        m_.set_num("_Sawrotationspeed", rs);
                        if let Some(v) = c_r1 { m_.set_num("_SawRandom1", v); }
                        if let Some(v) = c_r2 { m_.set_num("_SawRandom2", v); }
                    }
                    let off = m_.rotation - crs;
                    m_.set_num("_Sawoffset", off);
                    self.reg_color(&reg, mi, col1, true, col1);
                    self.sprites[mi].set_num("_eeWorldX", child_world_x);
                    self.sprites[mi].set_num("_eeBaseY", child_base_y);
                    self.add_to_section(mi);
                    self.saw_registered(mi);
                    self.reg_groups(&reg, mi, child_world_x, child_base_y);
                    reg.object(&mut self.sprites[mi]);
                }
            }
        }
    }

    /// `_applyVisualProps` with a child's definition as colour data (`can_color` possibly taken from the parent).
    fn apply_visual_props_child(&mut self, si: usize, frame: &str, o: &LevelObject, rot: f64, cd: &Json, can_color: Option<bool>) {
        self.apply_visual_props(si, frame, o, rot, None);
        let black = cd.get_bool("black") == Some(true) || cd.get_num("tint") == Some(0.0);
        let s = &mut self.sprites[si];
        if let Some(t) = cd.get_num("tint") { s.tint = Some(t); }
        if black {
            s.tint = Some(0.0);
            s.set_flag("_isBlack", true);
            let can = can_color == Some(true);
            s.set_flag("_canColor", can);
            s.set_flag("_blackDefault", can && !o.color1.map_or(false, |c| c > 0));
        }
    }

    fn add_glow_sprite_child(&mut self, x: f64, y: f64, frame: &str, o: &LevelObject, rot: f64, world_x: f64, cd: &Json, can_color: Option<bool>) -> Option<usize> {
        let glow_frame = Self::glow_frame_name(frame, Some(cd))?;
        if glow_frame == frame { return None; }
        let r = if rot.is_finite() { rot } else { 0.0 };
        let key = format!("{}:{}:{}:{}", num(world_x), num(y), glow_frame, crate::physics::js_round(r * 1000.0) as i64);
        if self.glow_keys.contains(&key) { return None; }
        let gi = self.add_image(x, y, &glow_frame)?;
        self.glow_keys.insert(key);
        self.apply_visual_props(gi, &glow_frame, o, rot, None);
        let black = cd.get_bool("black") == Some(true) || cd.get_num("tint") == Some(0.0);
        let s = &mut self.sprites[gi];
        if let Some(t) = cd.get_num("tint") { s.tint = Some(t); }
        if black {
            s.tint = Some(0.0);
            s.set_flag("_isBlack", true);
            let can = can_color == Some(true);
            s.set_flag("_canColor", can);
            s.set_flag("_blackDefault", can && !o.color1.map_or(false, |c| c > 0));
        }
        s.blend = ADD;
        s.set_alpha(0.425);
        let a = s.alpha;
        s.set_num("_eeOrigAlpha", a);
        s.set_num("_eeLayer", 0.0);
        s.set_flag("_eeIsGlowSprite", true);
        s.visible = !self.ldm;
        s.set_num("_eeWorldX", world_x);
        s.set_num("_eeBaseY", y);
        self.add_to_section(gi);
        Some(gi)
    }
}

pub fn sprites_json(sprites: &[Sprite]) -> String {
    let items: Vec<String> = sprites.iter().map(|s| {
        let mut props = String::new();
        for (k, v) in s.props.iter() {
            let val = match v { Val::Num(n) => num(*n), Val::Bool(b) => b.to_string(), Val::Str(t) => str_lit(t), Val::Arr(a) => format!("[{}]", a.iter().map(|x| num(*x)).collect::<Vec<_>>().join(",")), Val::List(l) => format!("[{}]", l.iter().map(|x| str_lit(x)).collect::<Vec<_>>().join(",")) };
            props.push_str(&format!(",{}:{}", str_lit(k), val));
        }
        format!("{{\"atlas\":{},\"frame\":{},\"x\":{},\"y\":{},\"rotation\":{},\"scaleX\":{},\"scaleY\":{},\"alpha\":{},\"visible\":{},\"flipX\":{},\"flipY\":{},\"tint\":{},\"blend\":{}{}}}",
            str_lit(&s.atlas), str_lit(&s.frame), num(s.x), num(s.y), num(s.rotation), num(s.scale_x), num(s.scale_y), num(s.alpha), s.visible, s.flip_x, s.flip_y, s.tint.map_or("null".to_string(), num), s.blend, props)
    }).collect();
    format!("[{}]", items.join(","))
}


/// `_lighterHex`
fn lighter_hex(hex: f64) -> f64 {
    let h32 = hex as i64;
    let (r, g, b) = (((h32 >> 16) & 255) as f64, ((h32 >> 8) & 255) as f64, (h32 & 255) as f64);
    let max = r.max(g).max(b) / 255.0;
    let min = r.min(g).min(b) / 255.0;
    let h = crate::colors::rgb_to_hsv(r, g, b).0;
    let sat = 0.0001f64.max(1f64.min((if max == 0.0 { 0.0 } else { (max - min) / max }) * 0.65));
    let val = 0.0001f64.max(1f64.min(max + 0.15));
    let c = val * sat;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m_ = val - c;
    let (rr, gg, bb) = if h < 60.0 { (c, x, 0.0) } else if h < 120.0 { (x, c, 0.0) } else if h < 180.0 { (0.0, c, x) } else if h < 240.0 { (0.0, x, c) } else if h < 300.0 { (x, 0.0, c) } else { (c, 0.0, x) };
    ((((rr + m_) * 255.0).trunc() as i64) << 16 | (((gg + m_) * 255.0).trunc() as i64) << 8 | ((bb + m_) * 255.0).trunc() as i64) as i32 as f64
}

impl Sprite {
    /// `_applyChannelTint(spr, chId, hex, colorManager)`: the tint it would set, or `None` when it leaves the tint alone.
    pub fn channel_tint(&self, ch: i64, hex: f64, cm: &crate::colors::ColorManager) -> Option<f64> {
        if self.flag("_cantColor") { return None; }
        if self.flag("_eePulsed") { return None; }
        if self.flag("_eeAudioScale") && !self.flag("_eeAudioTintable") { return None; }
        if self.flag("_isSaw") {
            return self.num("_SawColor").map(|c| cm.get_hex(c as i64));
        }
        if self.flag("_isBlack") && !self.flag("_canColor") { return None; }
        if self.flag("_blackDefault") && !cm.has_color(ch) { return None; }
        let mut hex = hex;
        if self.flag("_eeLighter") { hex = lighter_hex(hex); }
        if let Some(Val::Arr(h)) = self.props.get("_eeHsv") {
            let hsv = crate::colors::Hsv { h: h[0], s: h[1], v: h[2], s_add: h[3] != 0.0, v_add: h[4] != 0.0 };
            let hx = hex as i64;
            let shifted = crate::colors::apply_channel_hsv(crate::colors::Rgb { r: ((hx >> 16) & 255) as f64, g: ((hx >> 8) & 255) as f64, b: (hx & 255) as f64 }, Some(hsv));
            let clamp = |v: f64| 0f64.max(255f64.min(crate::physics::js_round(v))) as i64;
            hex = ((clamp(shifted.r) << 16) | (clamp(shifted.g) << 8) | clamp(shifted.b)) as f64;
        }
        Some(hex)
    }
}

/// The sections (400 wide) that `updateVisibility` shows for a camera: `(first, last)`, or `None` when culling hides everything.
/// `cull_distance` is the `window.cullDistance` setting (0..3, default 3).
pub fn visible_sections(camera_x: f64, last_section: i64, screen_width: f64, cull_distance: f64) -> Option<(i64, i64)> {
    if last_section < 0 { return None; }
    let cd = if cull_distance.is_finite() { 0f64.max(3f64.min(cull_distance.floor())) } else { 3.0 };
    let (lo, hi) = if cd == 3.0 {
        (0f64.max(((camera_x - 200.0) / 400.0).floor()), (last_section as f64).min(((camera_x + screen_width + 200.0) / 400.0).floor()))
    } else if cd == 0.0 {
        ((last_section + 1) as f64, -1.0)
    } else {
        let margin = if cd == 2.0 { 100.0 } else { 0.0 };
        (0f64.max(((camera_x - margin) / 400.0).floor()), (last_section as f64).min(((camera_x + screen_width + margin) / 400.0).floor()))
    };
    if lo > hi { None } else { Some((lo as i64, hi as i64)) }
}

// ---- text objects (level.js `_spawnTextObject`, object 914) ----
impl<'a> Builder<'a> {
    /// The frame-table entry a text sprite draws: atlas `@text`, frame `size|text`.
    pub fn text_frame(size: f64, text: &str) -> (String, String) { ("@text".to_string(), format!("{}|{}", num(size), text)) }

    /// Spawns the bitmap text sprite of a text object. `linked` is the object's linked id (already counted).
    pub fn spawn_text(&mut self, o: &LevelObject, def: &Json, linked: i64) -> usize {
        let world_x = o.x * 2.0;
        let world_y = T - o.y * 2.0;
        // (`levelObj.text` is always a string once parsed, so the definition's default text never shows)
        let text = o.text.clone();
        let base_size = def.get_num("textSize").filter(|v| v.is_finite()).unwrap_or(36.0);
        let size = 1f64.max(m_round(base_size));
        let (atlas, frame) = Self::text_frame(size, &text);
        let mut s = Sprite::new(&atlas, &frame, world_x, world_y);
        let (tsx, tsy) = gd_object_scale(o);
        let warp = gd_object_warp(o);
        let (sx, sy) = (tsx * if o.flip_x { -1.0 } else { 1.0 }, tsy * if o.flip_y { -1.0 } else { 1.0 });
        let rot = or0(o.rot);
        // applyGDObjectTransform
        if warp == 0.0 || warp.is_nan() {
            if rot != 0.0 { s.set_angle(rot); }
            if sx != 1.0 || sy != 1.0 { s.scale_x = sx; s.scale_y = sy; }
            s.set_num("_eeBaseScaleX", sx);
            s.set_num("_eeBaseScaleY", sy);
        } else {
            s.set_num("_eeBaseScaleX", 1.0);
            s.set_num("_eeBaseScaleY", 1.0);
            let (rx, ry) = warp_axes(rot, warp);
            s.props.insert("_gdWarp", Val::Arr(vec![rx, ry, sx, sy]));
        }
        let (bx, by) = (s.num("_eeBaseScaleX").unwrap_or(1.0), s.num("_eeBaseScaleY").unwrap_or(1.0));
        s.set_num("_eeObjScaleX", bx);
        s.set_num("_eeObjScaleY", by);
        let depth_base = |z: i64| -> f64 { match z { -5 => -12.0, -3 => -9.0, -1 => -6.0, 0 => 0.0, 1 => 3.0, 3 => 6.0, 5 => 9.0, 7 => 10.5, 9 => 12.0, 11 => 13.5, _ => 0.0 } };
        let z_layer = o.z_layer.unwrap_or_else(|| def.get_num("default_z_layer").map_or(3, |v| v as i64));
        let z_order = o.z_order.unwrap_or_else(|| def.get_num("default_z_order").map_or(0, |v| v as i64));
        let z_depth = depth_base(z_layer) + z_order as f64 * 0.001;
        s.set_num("_eeLayer", 1.0);
        s.set_num("_eeWorldX", world_x);
        s.set_num("_eeBaseY", world_y);
        s.set_num("_eeOrigWorldX", world_x);
        s.set_num("_eeOrigBaseY", world_y);
        s.set_num("_eeZDepth", z_depth);
        s.set_num("_eeOrigAlpha", 1.0);
        s.set_flag("_eeTextObject", true);
        s.set_num("_eeObjectId", linked as f64);
        s.set_num("_eeEditorLayer", o.editor_layer.unwrap_or_else(|| crate::js::parse_int(o.raw.get("20").map(|v| v.as_str()).unwrap_or("0")).unwrap_or(0)) as f64);
        s.set_num("_eeEditorLayer2", o.editor_layer2.unwrap_or_else(|| crate::js::parse_int(o.raw.get("61").map(|v| v.as_str()).unwrap_or("0")).unwrap_or(0)) as f64);
        let si = self.sprites.len();
        let channel = match o.color1 { Some(c) if c != 0 => c, _ => def.get_num("default_base_color_channel").map_or(0, |v| v as i64) };
        if channel > 0 && def.get_bool("can_color") != Some(false) {
            s.set_num("_eeColorChannel", channel as f64);
            self.color_channel_sprites.entry(channel).or_default().push(si);
        }
        let gids = crate::triggers::object_groups(&o.groups);
        if !gids.is_empty() {
            s.props.insert("_eeGroups", Val::Arr(gids.iter().map(|g| *g as f64).collect()));
            s.set_num("_origWorldX", world_x); s.set_num("_origBaseY", world_y);
            s.set_num("_eeMoveBaseWorldX", world_x); s.set_num("_eeMoveBaseBaseY", world_y);
            s.set_num("_eeInitialWorldX", world_x); s.set_num("_eeInitialBaseY", world_y);
            let r = s.rotation;
            s.set_num("_eeInitialRotationRad", r);
            for g in &gids { self.group_sprites.entry(*g).or_default().push(si); }
        }
        self.sprites.push(s);
        self.add_to_section(si);
        si
    }
}
fn m_round(v: f64) -> f64 { (v + 0.5).floor() }

// ---- BeastAnim objects (animations.js `beastAnimSpawn`) ----
impl<'a> Builder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn spawn_beast(&mut self, o: &LevelObject, def: &Json, reg: &Reg, start: usize, world_x: f64, base_y: f64, z: f64, col1: f64, col2: f64) {
        use crate::beast::{eye_for, secondary_tags, BState, Beast};
        let id = o.id.unwrap_or(0);
        let Some(state) = BState::create_with(id, &mut *self.rng) else { return };
        let obj_scale = { let s = or0(o.scale); if s == 0.0 { 1.0 } else { s } };
        let obj_rot = or0(o.rot) * PI / 180.0;
        let (rot_cos, rot_sin) = (m::cos(obj_rot), m::sin(obj_rot));
        let detail_tags = secondary_tags(id);
        // groupMap / detailTextures, in file order
        let mut group_map: Vec<String> = Vec::new();
        let mut detail_tex: Vec<String> = Vec::new();
        for (_, parts) in &state.desc.container {
            for p in parts {
                let Some(tex) = p.texture.as_ref().filter(|t| t.ends_with(".png")) else { continue };
                if !group_map.contains(tex) { group_map.push(tex.clone()); }
                if let Some(tags) = detail_tags { if tags.contains(&p.tag) && !detail_tex.contains(tex) { detail_tex.push(tex.clone()); } }
            }
        }
        let eye_def = eye_for(id);
        let mut children_by_frame: Vec<(String, &Json)> = Vec::new();
        for c in child_vec(def) {
            if let Some(f) = c.get_str("frame").filter(|f| !f.is_empty()) {
                match children_by_frame.iter_mut().find(|e| e.0 == f) { Some(e) => e.1 = c, None => children_by_frame.push((f.to_string(), c)) }
            }
        }
        let mut slot_tex: Vec<(String, Vec<String>)> = Vec::new();
        let mut slot_child: Vec<(String, &Json)> = Vec::new();
        for (_, parts) in &state.desc.container {
            for p in parts {
                let Some(tex) = p.texture.as_ref().filter(|t| t.ends_with(".png")) else { continue };
                match slot_tex.iter_mut().find(|e| e.0 == p.key) { Some(e) => { if !e.1.contains(tex) { e.1.push(tex.clone()); } } None => slot_tex.push((p.key.clone(), vec![tex.clone()])) }
                if let Some(c) = children_by_frame.iter().find(|e| e.0 == *tex) { if !slot_child.iter().any(|e| e.0 == p.key) { slot_child.push((p.key.clone(), c.1)); } }
            }
        }
        let mut detail_part_def: Vec<(String, &Json)> = Vec::new();
        for (pk, c) in &slot_child {
            if let Some((_, texs)) = slot_tex.iter().find(|e| e.0 == *pk) {
                for t in texs { if !detail_part_def.iter().any(|e| e.0 == *t) { detail_part_def.push((t.clone(), *c)); } }
            }
        }
        let detail_channel = if col2 > 0.0 { col2 } else { 1011.0 };
        let end = self.sprites.len();
        let mut eye: Option<usize> = None;
        let mut hidden: Vec<usize> = Vec::new();
        for si in start..end {
            if self.sprites[si].num("_beastPartOf").is_some() { continue; }
            let frame = self.sprites[si].frame.clone();
            if frame.is_empty() { continue; }
            if let Some((ef, _)) = eye_def { if frame == ef {
                eye = Some(si);
                let s = &mut self.sprites[si];
                s.set_flag("_beastManaged", true);
                s.set_num("_eeZDepth", z + 0.009);
                continue;
            } }
            let base_frame = frame.replace("_glow_001.png", "_001.png");
            if !group_map.contains(&frame) && !group_map.contains(&base_frame) { continue; }
            let s = &mut self.sprites[si];
            s.visible = false; s.active = false;
            s.set_flag("_beastStaticHidden", true);
            hidden.push(si);
        }
        let beast_glow = Self::has_glow(Some(def), Some(o));
        let mut parts: Vec<(String, usize)> = Vec::new();
        let mut glows: BTreeMap<String, usize> = BTreeMap::new();
        for tex in &group_map {
            let Some(si) = self.add_image(world_x, base_y, tex) else { continue };
            {
                let s = &mut self.sprites[si];
                s.set_num("_eeLayer", 1.0);
                s.set_num("_eeWorldX", world_x);
                s.set_num("_eeBaseY", base_y);
                s.set_num("_eeZDepth", z + 0.005);
                s.set_num("_eeOrigAlpha", 1.0);
                s.set_num("_beastPartOf", id as f64);
                s.set_flag("_beastManaged", true);
            }
            let child_def: Option<&Json> = detail_part_def.iter().find(|e| e.0 == *tex).map(|e| e.1);
            // colorDef = { ...childDef, can_color: childDef.can_color ?? objectDef.can_color } or the object definition
            let cd: &Json = child_def.unwrap_or(def);
            let cd_can = if child_def.is_some() { cd.get_bool("can_color").or(def.get_bool("can_color")) } else { def.get_bool("can_color") };
            let detail_part = detail_tex.contains(tex);
            let tint = cd.get_num("tint");
            if !detail_part { if let Some(t) = tint { self.sprites[si].tint = Some(t); } }
            let black_default = !detail_part && (cd.get_bool("black") == Some(true) || tint == Some(0.0) || def.get_bool("black") == Some(true));
            if black_default {
                let s = &mut self.sprites[si];
                s.tint = Some(0.0);
                s.set_flag("_isBlack", true);
                let can = cd_can == Some(true);
                s.set_flag("_canColor", can);
                s.set_flag("_blackDefault", can && !(o.color1.map_or(false, |c| c > 0)));
            }
            self.add_to_section(si);
            let ch = if detail_part { detail_channel } else { col1 };
            self.reg_color(reg, si, ch, false, col1);
            self.reg_groups(reg, si, world_x, base_y);
            { let mut sp = std::mem::replace(&mut self.sprites[si], Sprite::new("", "", 0.0, 0.0)); reg.object(&mut sp); self.sprites[si] = sp; }
            parts.push((tex.clone(), si));
            if beast_glow && tex.ends_with("_001.png") {
                let glow_frame = tex.replace("_001.png", "_glow_001.png");
                if glow_frame != *tex {
                    let mut key = format!("{}:{}:{}:{}", num(world_x), num(base_y), glow_frame, 0);
                    if self.glow_keys.contains(&key) {
                        let mut suffix = 1;
                        while self.glow_keys.contains(&format!("{}|{}", key, suffix)) { suffix += 1; }
                        key = format!("{}|{}", key, suffix);
                    }
                    if let Some(gi) = self.add_image(world_x, base_y, &glow_frame) {
                        self.glow_keys.insert(key);
                        self.apply_visual_props(gi, &glow_frame, o, 0.0, None);
                        // colorData: the part's definition, none for a detail part
                        if !detail_part {
                            let g_tint = cd.get_num("tint");
                            let g_black = cd.get_bool("black") == Some(true) || g_tint == Some(0.0);
                            if let Some(t) = g_tint { self.sprites[gi].tint = Some(t); }
                            if g_black {
                                let s = &mut self.sprites[gi];
                                s.tint = Some(0.0);
                                s.set_flag("_isBlack", true);
                                let can = cd_can == Some(true);
                                s.set_flag("_canColor", can);
                                s.set_flag("_blackDefault", can && !(o.color1.map_or(false, |c| c > 0)));
                            }
                        }
                        let ldm = self.ldm;
                        { let s = &mut self.sprites[gi]; s.blend = ADD; s.set_alpha(0.425); s.set_num("_eeLayer", 0.0); s.set_flag("_eeIsGlowSprite", true); s.visible = !ldm; s.set_num("_eeWorldX", world_x); s.set_num("_eeBaseY", base_y); }
                        let a = self.sprites[gi].alpha;
                        self.sprites[gi].set_num("_eeOrigAlpha", a);
                        self.add_to_section(gi);
                        self.sync_glow_color(gi, si);
                        let a = self.sprites[gi].alpha;
                        { let s = &mut self.sprites[gi]; s.set_num("_eeOrigAlpha", a); s.set_num("_eeZDepth", z + 0.002); s.set_num("_beastPartOf", id as f64); s.set_flag("_beastManaged", true); }
                        self.reg_color(reg, gi, ch, false, col1);
                        self.reg_groups(reg, gi, world_x, base_y);
                        glows.insert(tex.clone(), gi);
                    }
                }
            }
        }
        if parts.is_empty() { return; }
        let mut state = state;
        let rec = Beast { state: BState { ..state.clone() }, parts, glows, hidden, eye, eye_parent: eye_def.map(|e| e.1.to_string()), world_x, base_y, scale: obj_scale, flip_x: o.flip_x, flip_y: o.flip_y, rot_cos, rot_sin, last_frame_key: None, base_rot: None };
        let mut rec = rec;
        rec.state.default_base_with(&mut *self.rng);
        let _ = &mut state;
        self.beasts.push(rec);
    }
}

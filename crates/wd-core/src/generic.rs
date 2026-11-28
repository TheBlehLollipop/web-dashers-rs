use crate::game::Game;
use crate::js::{parse_float, parse_int};
use crate::json::{num, str_lit};
use crate::jsmath as m;
use crate::level::LevelObject;
use crate::triggers::{self, easing, js_max, js_min, or0, push_unique, raw, Base, Remap, TOUCH_HALF_HITBOX};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum K {
    Random, AdvRandom, Sequence, ItemEdit, ItemCompare, ItemPersist, Timer, TimeEvent, TimeControl, Scale, TimeWarp, End, Reset,
    CameraOffset, GameplayOffset, CameraZoom, CameraRotate, Options, ChangeBackground, ChangeGround, Shader, Reverse, Gravity,
    PlayerControl, Teleport, Song, Sfx, EditSong, EditSfx, RotateGameplay, StaticCamera, CameraEdge, CameraMode, Area, BgSpeed,
    MgSpeed, LinkVisible, AdvFollow, Gradient, KeyframeAnim, Event, InstantCollision,
}

impl K {
    pub fn name(self) -> &'static str {
        match self {
            K::Random => "random", K::AdvRandom => "advRandom", K::Sequence => "sequence", K::ItemEdit => "itemEdit", K::ItemCompare => "itemCompare",
            K::ItemPersist => "itemPersist", K::Timer => "timer", K::TimeEvent => "timeEvent", K::TimeControl => "timeControl", K::Scale => "scale",
            K::TimeWarp => "timeWarp", K::End => "end", K::Reset => "reset", K::CameraOffset => "cameraOffset", K::GameplayOffset => "gameplayOffset",
            K::CameraZoom => "cameraZoom", K::CameraRotate => "cameraRotate", K::Options => "options", K::ChangeBackground => "changeBackground",
            K::ChangeGround => "changeGround", K::Shader => "shader", K::Reverse => "reverse", K::Gravity => "gravity", K::PlayerControl => "playerControl",
            K::Teleport => "teleport", K::Song => "song", K::Sfx => "sfx", K::EditSong => "editSong", K::EditSfx => "editSfx", K::RotateGameplay => "rotateGameplay",
            K::StaticCamera => "staticCamera", K::CameraEdge => "cameraEdge", K::CameraMode => "cameraMode", K::Area => "area", K::BgSpeed => "bgSpeed",
            K::MgSpeed => "mgSpeed", K::LinkVisible => "linkVisible", K::AdvFollow => "advFollow", K::Gradient => "gradient", K::KeyframeAnim => "keyframeAnim",
            K::Event => "event", K::InstantCollision => "instantCollision",
        }
    }
    pub fn ported(self) -> bool {
        !matches!(self, K::ChangeBackground | K::ChangeGround | K::Shader | K::Teleport | K::Song | K::Sfx | K::EditSong | K::EditSfx
            | K::Area | K::BgSpeed | K::MgSpeed | K::AdvFollow | K::Gradient | K::KeyframeAnim)
    }
}

pub fn kind_of_id(id: i64) -> Option<K> {
    Some(match id {
        1912 => K::Random, 2068 => K::AdvRandom, 3607 => K::Sequence, 3619 => K::ItemEdit, 3620 => K::ItemCompare, 3641 => K::ItemPersist,
        3614 => K::Timer, 3615 => K::TimeEvent, 3617 => K::TimeControl, 2067 => K::Scale, 1935 => K::TimeWarp, 3600 => K::End, 3618 => K::Reset,
        1916 => K::CameraOffset, 2901 => K::GameplayOffset, 1913 => K::CameraZoom, 2015 => K::CameraRotate, 2899 => K::Options,
        3029 => K::ChangeBackground, 3030 => K::ChangeGround,
        2904 | 2910 | 2911 | 2912 | 2905 | 2907 | 2909 | 2913 | 2914 | 2915 | 2916 | 2917 | 2919 | 2920 | 2921 | 2922 | 2923 | 2924 => K::Shader,
        1917 => K::Reverse, 2066 => K::Gravity, 1932 => K::PlayerControl, 3022 => K::Teleport, 1934 => K::Song, 3602 => K::Sfx, 3605 => K::EditSong,
        3603 => K::EditSfx, 2900 => K::RotateGameplay, 1914 => K::StaticCamera, 2062 => K::CameraEdge, 2925 => K::CameraMode,
        3006..=3015 | 3024 | 3017..=3021 | 3023 => K::Area, 3606 => K::BgSpeed, 3612 => K::MgSpeed, 3662 => K::LinkVisible,
        3016 | 3660 | 3661 => K::AdvFollow, 2903 => K::Gradient, 3033 => K::KeyframeAnim, 3604 => K::Event, 3609 => K::InstantCollision,
        _ => return None,
    })
}

#[derive(Clone, Debug)]
pub struct Gen {
    pub b: Base,
    pub kind: K,
    pub x: f64,
    pub y: f64,
    pub touch: bool,
    pub target: i64,
    pub center: i64,
    pub chance: f64,
    pub choices: Vec<(i64, i64)>,
    pub sequence: Vec<(i64, i64)>,
    pub sequence_total: i64,
    pub mode: i64,
    pub min_interval: f64,
    pub reset_time: f64,
    pub reset_step: bool,
    pub item1: i64,
    pub item2: i64,
    pub type1: i64,
    pub type2: i64,
    pub target_type: i64,
    pub mod1: f64,
    pub mod2: f64,
    pub op1: i64,
    pub op2: i64,
    pub op3: i64,
    pub tolerance: f64,
    pub round1: i64,
    pub round2: i64,
    pub sign1: i64,
    pub sign2: i64,
    pub persistent: bool,
    pub target_all: bool,
    pub reset_item: bool,
    pub is_timer: bool,
    pub start_time: f64,
    pub stop_time: f64,
    pub stop_time_enabled: bool,
    pub dont_override: bool,
    pub ignore_time_warp: bool,
    pub time_mod: f64,
    pub start_paused: bool,
    pub target_time: f64,
    pub multi_activate: bool,
    pub stop: bool,
    pub scale_x: f64,
    pub scale_y: f64,
    pub duration: f64,
    pub easing_type: i64,
    pub easing_rate: f64,
    pub only_move: bool,
    pub offset_x: f64,
    pub offset_y: f64,
    pub axis: i64,
    pub apply_x: bool,
    pub apply_y: bool,
    pub hide_ground: i64,
    pub hide_p1: i64,
    pub hide_p2: i64,
    pub disable_p1: i64,
    pub disable_p2: i64,
    pub hide_attempts: i64,
    pub respawn_mode: i64,
    pub respawn_time: f64,
    pub degrees: f64,
    pub add: bool,
    pub snap360: bool,
    pub zoom: f64,
    pub events: Vec<i64>,
    pub extra_id: i64,
    pub extra_id2: i64,
    pub block_a: i64,
    pub block_b: i64,
    pub p1: bool,
    pub p2: bool,
    pub player_pair: bool,
    pub edge: i64,
    pub edge_target: i64,
    pub free_mode: bool,
    pub edit_settings: bool,
    pub easing: f64,
    pub padding: f64,
    pub exit: bool,
    pub exit_instant: bool,
    pub no_effects: bool,
    pub real_touch: bool,
    pub rg_ground: i64,
    pub rg_move: i64,
    pub edit_velocity: bool,
    pub override_velocity: bool,
    pub velocity_mod_x: f64,
    pub velocity_mod_y: f64,
    pub channel_only: bool,
    pub change_channel: bool,
    pub target_channel: i64,
    pub dont_slide: bool,
    pub no_sfx: bool,
    pub instant: bool,
    pub gravity: f64,
    pub stop_jump: bool,
    pub stop_move: bool,
    pub stop_rotate: bool,
    pub stop_slide: bool,
    pub passive: bool,
}

fn g_raw<'a>(o: &'a LevelObject, k: i64) -> Option<&'a String> { raw(o, &k.to_string()) }
fn g_int(o: &LevelObject, k: i64, fb: i64) -> i64 {
    match g_raw(o, k) { Some(s) => parse_int(s).unwrap_or(fb), None => fb }
}
fn g_float(o: &LevelObject, k: i64, fb: f64) -> f64 {
    match g_raw(o, k) { Some(s) => match parse_float(s) { Some(v) if v.is_finite() => v, _ => fb }, None => fb }
}
fn g_bool(o: &LevelObject, k: i64) -> bool { g_raw(o, k).map(|s| s.as_str()).unwrap_or("0") == "1" }
fn g_pairs(o: &LevelObject, k: i64) -> Vec<(i64, i64)> {
    let s = g_raw(o, k).cloned().unwrap_or_default();
    let parts: Vec<Option<i64>> = s.split('.').map(parse_int).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < parts.len() {
        if let (Some(a), Some(b)) = (parts[i], parts[i + 1]) { out.push((a, b)); }
        i += 2;
    }
    out
}

impl Gen {
    fn blank(b: Base, kind: K, x: f64, y: f64) -> Gen {
        Gen {
            b, kind, x, y, touch: false, target: 0, center: 0, chance: 0.0, choices: Vec::new(), sequence: Vec::new(), sequence_total: 0, mode: 0,
            min_interval: 0.0, reset_time: 0.0, reset_step: false, item1: 0, item2: 0, type1: 0, type2: 0, target_type: 0, mod1: 0.0, mod2: 1.0, op1: 0, op2: 0,
            op3: 0, tolerance: 0.0, round1: 0, round2: 0, sign1: 0, sign2: 0, persistent: false, target_all: false, reset_item: false, is_timer: false,
            start_time: 0.0, stop_time: 0.0, stop_time_enabled: false, dont_override: false, ignore_time_warp: false, time_mod: 0.0, start_paused: false,
            target_time: 0.0, multi_activate: false, stop: false, scale_x: 1.0, scale_y: 1.0, duration: 0.0, easing_type: 0, easing_rate: 2.0, only_move: false,
            offset_x: 0.0, offset_y: 0.0, axis: 0, apply_x: false, apply_y: false, hide_ground: 0, hide_p1: 0, hide_p2: 0, disable_p1: 0, disable_p2: 0,
            hide_attempts: 0, respawn_mode: 0, respawn_time: 0.0, degrees: 0.0, add: false, snap360: false, zoom: 1.0, events: Vec::new(), extra_id: 0,
            extra_id2: 0, block_a: 0, block_b: 0, p1: false, p2: false, player_pair: false, edge: 0, edge_target: 0, free_mode: false, edit_settings: false,
            easing: 1.0, padding: 0.0, exit: false, exit_instant: false, no_effects: false, real_touch: false, rg_ground: 2, rg_move: 4, edit_velocity: false, override_velocity: false, velocity_mod_x: 0.0, velocity_mod_y: 0.0, channel_only: false, change_channel: false, target_channel: 0, dont_slide: false, no_sfx: false, instant: false, gravity: 1.0, stop_jump: false,
            stop_move: false, stop_rotate: false, stop_slide: false, passive: false,
        }
    }
}

pub fn parse(o: &LevelObject, b: Base) -> Option<Gen> {
    let id = o.id.unwrap_or(0);
    let kind = kind_of_id(id)?;
    let (x, y) = (o.x * 2.0, o.y * 2.0);
    let mut t = Gen::blank(b, kind, x, y);
    t.touch = g_bool(o, 11);
    t.b.multi = g_bool(o, 87);
    t.target = g_int(o, 51, 0);
    t.center = g_int(o, 71, 0);
    let rate_or2 = |v: f64| if v > 0.0 { v } else { 2.0 };
    match kind {
        K::Random => t.chance = js_max(0.0, js_min(100.0, g_float(o, 10, 0.0))),
        K::AdvRandom => t.choices = g_pairs(o, 152).into_iter().filter(|p| p.0 > 0 && p.1 > 0).collect(),
        K::Sequence => {
            t.sequence = g_pairs(o, 435).into_iter().filter(|p| p.0 > 0 && p.1 > 0).collect();
            t.sequence_total = t.sequence.iter().map(|p| p.1).sum();
            t.mode = g_int(o, 436, 0);
            t.min_interval = g_float(o, 437, 0.0);
            t.reset_time = g_float(o, 438, 0.0);
            t.reset_step = g_int(o, 439, 0) != 0;
        }
        K::ItemEdit | K::ItemCompare => {
            t.item1 = g_int(o, 80, 0);
            t.item2 = g_int(o, 95, 0);
            t.type1 = g_int(o, 476, 0);
            t.type2 = g_int(o, 477, 0);
            t.target_type = g_int(o, 478, 0);
            t.mod1 = g_float(o, 479, if kind == K::ItemEdit { 0.0 } else { 1.0 });
            t.mod2 = g_float(o, 483, 1.0);
            t.op1 = g_int(o, 480, 0);
            t.op2 = g_int(o, 481, 0);
            t.op3 = g_int(o, 482, 0);
            t.tolerance = g_float(o, 484, 0.0);
            t.round1 = g_int(o, 485, 0);
            t.round2 = g_int(o, 486, 0);
            t.sign1 = g_int(o, 578, 0);
            t.sign2 = g_int(o, 579, 0);
        }
        K::ItemPersist => {
            t.item1 = g_int(o, 80, 0);
            t.persistent = g_bool(o, 491);
            t.target_all = g_bool(o, 492);
            t.reset_item = g_bool(o, 493);
            t.is_timer = g_bool(o, 494);
        }
        K::Timer => {
            t.item1 = g_int(o, 80, 0);
            t.start_time = g_float(o, 467, 0.0);
            t.stop_time = g_float(o, 473, 0.0);
            t.stop_time_enabled = g_bool(o, 474);
            t.dont_override = g_bool(o, 468);
            t.ignore_time_warp = g_bool(o, 469);
            t.time_mod = g_float(o, 470, 0.0);
            t.start_paused = g_bool(o, 471);
        }
        K::TimeEvent => {
            t.item1 = g_int(o, 80, 0);
            t.target_time = g_float(o, 473, 0.0);
            t.multi_activate = g_bool(o, 475);
        }
        K::TimeControl => {
            t.item1 = g_int(o, 80, 0);
            t.stop = g_int(o, 472, 0) == 1;
        }
        K::Scale => {
            let sx = g_float(o, 150, 1.0);
            let sy = g_float(o, 151, 1.0);
            t.scale_x = if g_bool(o, 153) { if sx != 0.0 { 1.0 / sx } else { 1.0 } } else { sx };
            t.scale_y = if g_bool(o, 154) { if sy != 0.0 { 1.0 / sy } else { 1.0 } } else { sy };
            t.duration = js_max(0.0, g_float(o, 10, 0.0));
            t.easing_type = g_int(o, 30, 0);
            t.easing_rate = g_float(o, 85, 2.0);
            t.only_move = g_bool(o, 133);
        }
        K::TimeWarp => t.time_mod = js_max(0.1, js_min(2.0, g_float(o, 120, 1.0))),
        K::CameraOffset => {
            t.offset_x = g_float(o, 28, 0.0) * 2.0;
            t.offset_y = g_float(o, 29, 0.0) * 2.0;
            t.axis = g_int(o, 101, 0);
            t.duration = js_max(0.0, g_float(o, 10, 0.0));
            t.easing_type = g_int(o, 30, 0);
            t.easing_rate = rate_or2(g_float(o, 85, 0.0));
        }
        K::GameplayOffset => {
            t.offset_x = g_float(o, 28, 0.0).trunc() * 2.0;
            t.offset_y = g_float(o, 29, 0.0).trunc() * 2.0;
            t.apply_x = g_bool(o, 58);
            t.apply_y = g_bool(o, 59);
            t.mode = g_int(o, 101, 0);
        }
        K::Options => {
            t.hide_ground = g_int(o, 161, 0);
            t.hide_p1 = g_int(o, 162, 0);
            t.hide_p2 = g_int(o, 163, 0);
            t.disable_p1 = g_int(o, 165, 0);
            t.disable_p2 = g_int(o, 199, 0);
            t.hide_attempts = g_int(o, 532, 0);
            t.respawn_mode = g_int(o, 573, 0);
            t.respawn_time = g_float(o, 574, 0.0);
        }
        K::CameraRotate => {
            t.degrees = g_float(o, 68, 0.0);
            t.add = g_bool(o, 70);
            t.snap360 = g_bool(o, 394);
            t.duration = js_max(0.0, g_float(o, 10, 0.0));
            t.easing_type = g_int(o, 30, 0);
            t.easing_rate = rate_or2(g_float(o, 85, 0.0));
        }
        K::Event => {
            t.events = g_raw(o, 430).cloned().unwrap_or_default().split('.').filter_map(parse_int).filter(|v| *v > 0).collect();
            t.extra_id = g_int(o, 447, 0);
            t.extra_id2 = g_int(o, 525, 0);
        }
        K::InstantCollision => {
            t.block_a = g_int(o, 80, 0);
            t.block_b = g_int(o, 95, 0);
            t.p1 = g_bool(o, 138);
            t.p2 = g_bool(o, 200);
            t.player_pair = g_bool(o, 201);
        }
        K::CameraEdge => {
            t.edge = g_int(o, 164, 0);
            t.edge_target = g_int(o, 51, 0);
        }
        K::CameraMode => {
            t.free_mode = g_bool(o, 111);
            t.edit_settings = g_bool(o, 112);
            t.easing = js_max(1.0, js_min(40.0, g_float(o, 113, 0.0)));
            t.padding = js_max(0.0, js_min(1.0, g_float(o, 114, 0.0)));
        }
        K::StaticCamera => {
            t.target = g_int(o, 71, 0);
            t.duration = js_max(0.0, g_float(o, 10, 0.0));
            t.easing_type = g_int(o, 30, 0);
            t.easing_rate = rate_or2(g_float(o, 85, 0.0));
            t.axis = g_int(o, 101, 0);
            t.exit = g_bool(o, 110);
            t.exit_instant = g_bool(o, 465);
        }
        K::CameraZoom => {
            let z = g_float(o, 371, 1.0);
            t.zoom = if z <= 0.0 { 1.0 } else { js_max(0.4, js_min(3.0, z)) };
            t.duration = js_max(0.0, g_float(o, 10, 0.0));
            t.easing_type = g_int(o, 30, 0);
            t.easing_rate = rate_or2(g_float(o, 85, 0.0));
        }
        K::End => {
            t.no_effects = g_bool(o, 460);
            t.no_sfx = g_bool(o, 461);
            t.instant = g_bool(o, 487);
        }
        K::RotateGameplay => {
            t.real_touch = t.touch;
            t.touch = true;
            t.rg_ground = g_int(o, 166, 2);
            t.rg_move = g_int(o, 167, 4);
            t.edit_velocity = g_bool(o, 169);
            t.override_velocity = g_bool(o, 584);
            t.velocity_mod_x = g_float(o, 582, 0.0);
            t.velocity_mod_y = g_float(o, 583, 0.0);
            t.channel_only = g_bool(o, 172);
            t.change_channel = g_bool(o, 171);
            t.target_channel = g_int(o, 173, 0);
            t.dont_slide = g_bool(o, 585);
        }
        K::Gravity => {
            t.gravity = js_max(0.1, js_min(2.0, g_float(o, 148, 1.0)));
            t.p1 = !g_bool(o, 200);
            t.p2 = !g_bool(o, 138) && !g_bool(o, 201);
        }
        K::PlayerControl => {
            t.stop_jump = g_bool(o, 540);
            t.stop_move = g_bool(o, 541);
            t.stop_rotate = g_bool(o, 542);
            t.stop_slide = g_bool(o, 543);
            t.p1 = g_bool(o, 138) || !g_bool(o, 200);
            t.p2 = !g_bool(o, 138) || g_bool(o, 200);
        }
        K::LinkVisible => {
            t.passive = true;
            t.target = g_int(o, 51, 0);
        }
        _ => {}
    }
    Some(t)
}

pub fn gen_json(t: &Gen) -> String {
    let b = &t.b;
    let ints = |v: &[i64]| format!("[{}]", v.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(","));
    let pairs = |v: &[(i64, i64)]| format!("[{}]", v.iter().map(|p| format!("[{},{}]", p.0, p.1)).collect::<Vec<_>>().join(","));
    let mut s = format!("{{\"uid\":{},\"spawnTriggered\":{},\"multiTrigger\":{},\"triggerChannel\":{},\"ord\":{},\"groups\":{},\"kind\":{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"centerGroup\":{}",
        b.uid, b.spawn_triggered, t.b.multi, b.channel, b.ord, ints(&b.groups), str_lit(t.kind.name()), num(t.x), num(t.y), t.touch, t.target, t.center);
    let mut f = |k: &str, v: String| { s.push_str(&format!(",\"{}\":{}", k, v)); };
    match t.kind {
        K::Random => f("chance", num(t.chance)),
        K::AdvRandom => f("choices", pairs(&t.choices)),
        K::Sequence => {
            f("sequence", pairs(&t.sequence)); f("sequenceTotal", t.sequence_total.to_string()); f("mode", t.mode.to_string());
            f("minInterval", num(t.min_interval)); f("resetTime", num(t.reset_time)); f("resetStep", t.reset_step.to_string());
        }
        K::ItemEdit | K::ItemCompare => {
            f("item1", t.item1.to_string()); f("item2", t.item2.to_string()); f("type1", t.type1.to_string()); f("type2", t.type2.to_string());
            f("targetType", t.target_type.to_string()); f("mod1", num(t.mod1)); f("mod2", num(t.mod2)); f("op1", t.op1.to_string()); f("op2", t.op2.to_string());
            f("op3", t.op3.to_string()); f("tolerance", num(t.tolerance)); f("round1", t.round1.to_string()); f("round2", t.round2.to_string());
            f("sign1", t.sign1.to_string()); f("sign2", t.sign2.to_string());
        }
        K::ItemPersist => {
            f("item1", t.item1.to_string()); f("persistent", t.persistent.to_string()); f("targetAll", t.target_all.to_string());
            f("resetItem", t.reset_item.to_string()); f("isTimer", t.is_timer.to_string());
        }
        K::Timer => {
            f("item1", t.item1.to_string()); f("startTime", num(t.start_time)); f("stopTime", num(t.stop_time)); f("stopTimeEnabled", t.stop_time_enabled.to_string());
            f("dontOverride", t.dont_override.to_string()); f("ignoreTimeWarp", t.ignore_time_warp.to_string()); f("timeMod", num(t.time_mod)); f("startPaused", t.start_paused.to_string());
        }
        K::TimeEvent => { f("item1", t.item1.to_string()); f("targetTime", num(t.target_time)); f("multiActivate", t.multi_activate.to_string()); }
        K::TimeControl => { f("item1", t.item1.to_string()); f("stop", t.stop.to_string()); }
        K::Scale => {
            f("scaleX", num(t.scale_x)); f("scaleY", num(t.scale_y)); f("duration", num(t.duration)); f("easingType", t.easing_type.to_string());
            f("easingRate", num(t.easing_rate)); f("onlyMove", t.only_move.to_string());
        }
        K::TimeWarp => f("timeMod", num(t.time_mod)),
        K::CameraOffset => {
            f("offsetX", num(t.offset_x)); f("offsetY", num(t.offset_y)); f("axis", t.axis.to_string()); f("duration", num(t.duration));
            f("easingType", t.easing_type.to_string()); f("easingRate", num(t.easing_rate));
        }
        K::GameplayOffset => {
            f("offsetX", num(t.offset_x)); f("offsetY", num(t.offset_y)); f("applyX", t.apply_x.to_string()); f("applyY", t.apply_y.to_string()); f("mode", t.mode.to_string());
        }
        K::Options => {
            f("hideGround", t.hide_ground.to_string()); f("hideP1", t.hide_p1.to_string()); f("hideP2", t.hide_p2.to_string());
            f("disableP1", t.disable_p1.to_string()); f("disableP2", t.disable_p2.to_string()); f("hideAttempts", t.hide_attempts.to_string());
            f("respawnMode", t.respawn_mode.to_string()); f("respawnTime", num(t.respawn_time));
        }
        K::CameraRotate => {
            f("degrees", num(t.degrees)); f("add", t.add.to_string()); f("snap360", t.snap360.to_string()); f("duration", num(t.duration));
            f("easingType", t.easing_type.to_string()); f("easingRate", num(t.easing_rate));
        }
        K::Event => { f("events", ints(&t.events)); f("extraId", t.extra_id.to_string()); f("extraId2", t.extra_id2.to_string()); }
        K::InstantCollision => {
            f("blockA", t.block_a.to_string()); f("blockB", t.block_b.to_string()); f("p1", t.p1.to_string()); f("p2", t.p2.to_string()); f("playerPair", t.player_pair.to_string());
        }
        K::CameraEdge => { f("edge", t.edge.to_string()); f("edgeTarget", t.edge_target.to_string()); }
        K::CameraMode => { f("freeMode", t.free_mode.to_string()); f("editSettings", t.edit_settings.to_string()); f("easing", num(t.easing)); f("padding", num(t.padding)); }
        K::StaticCamera => {
            f("duration", num(t.duration)); f("easingType", t.easing_type.to_string()); f("easingRate", num(t.easing_rate)); f("axis", t.axis.to_string());
            f("exit", t.exit.to_string()); f("exitInstant", t.exit_instant.to_string());
        }
        K::CameraZoom => { f("zoom", num(t.zoom)); f("duration", num(t.duration)); f("easingType", t.easing_type.to_string()); f("easingRate", num(t.easing_rate)); }
        K::End => { f("noEffects", t.no_effects.to_string()); f("noSFX", t.no_sfx.to_string()); f("instant", t.instant.to_string()); }
        K::RotateGameplay => {
            f("realTouch", t.real_touch.to_string()); f("ground", t.rg_ground.to_string()); f("move", t.rg_move.to_string());
            f("editVelocity", t.edit_velocity.to_string()); f("overrideVelocity", t.override_velocity.to_string());
            f("velocityModX", num(t.velocity_mod_x)); f("velocityModY", num(t.velocity_mod_y)); f("channelOnly", t.channel_only.to_string());
            f("changeChannel", t.change_channel.to_string()); f("targetChannel", t.target_channel.to_string()); f("dontSlide", t.dont_slide.to_string());
        }
        K::Gravity => { f("gravity", num(t.gravity)); f("p1", t.p1.to_string()); f("p2", t.p2.to_string()); }
        K::PlayerControl => {
            f("stopJump", t.stop_jump.to_string()); f("stopMove", t.stop_move.to_string()); f("stopRotate", t.stop_rotate.to_string());
            f("stopSlide", t.stop_slide.to_string()); f("p1", t.p1.to_string()); f("p2", t.p2.to_string());
        }
        K::LinkVisible => f("passive", "true".into()),
        _ => {}
    }
    s.push('}');
    s
}

#[derive(Clone, Debug)]
pub struct TimerExt { pub ignore_tw: bool, pub stop_time: f64, pub stop_enabled: bool, pub target: i64 }
#[derive(Clone, Debug)]
pub struct Timer { pub value: f64, pub running: bool, pub time_mod: f64, pub stopped: bool, pub ext: Option<TimerExt>, pub seq: u64 }
#[derive(Clone, Debug)]
pub struct ScaleTween { pub trig: usize, pub target: i64, pub center: i64, pub elapsed: f64, pub prev_x: f64, pub prev_y: f64 }
#[derive(Clone, Debug, Default)]
pub struct GroupScale { pub sx: f64, pub sy: f64, pub center: i64, pub only_move: bool }
#[derive(Clone, Debug)]
pub struct Tween { pub from: f64, pub to: f64, pub elapsed: f64, pub trig: usize }
#[derive(Clone, Debug)]
pub struct StaticAxis { pub group: i64, pub elapsed: f64, pub trig: usize, pub exiting: bool, pub instant: bool, pub from: Option<f64>, pub last: Option<f64>, pub target: f64 }
#[derive(Clone, Debug, Default)]
pub struct StaticCam { pub x: Option<StaticAxis>, pub y: Option<StaticAxis> }
#[derive(Clone, Debug)]
pub struct SeqState { pub index: i64, pub last_time: f64 }

#[derive(Clone, Debug)]
pub struct GenState {
    pub idx: usize,
    pub reverse_idx: i64,
    pub direction_dirty: bool,
    pub fired: BTreeSet<i64>,
    pub touching: BTreeMap<i64, bool>,
    pub sequence: BTreeMap<usize, SeqState>,
    pub scale_tweens: Vec<ScaleTween>,
    pub time_events: Vec<(usize, Option<i64>)>,
    pub level_time: f64,
    pub points: f64,
    pub attempts: f64,
    pub timers: BTreeMap<i64, Timer>,
    pub timer_seq: u64,
    pub persistent_timers: BTreeSet<i64>,
    pub event_links: Vec<(usize, Option<i64>)>,
    pub event_stamps: BTreeMap<u64, f64>,
    pub group_scales: BTreeMap<i64, GroupScale>,
    pub has_group_scales: bool,
    pub scaled_colliders: BTreeSet<usize>,
    pub link_visible: BTreeSet<i64>,
    pub end_triggered: Option<usize>,
    pub camera_offset: (f64, f64),
    pub gameplay_offset: (f64, f64),
    pub applied_camera_offset_y: f64,
    pub applied_gameplay_offset_y: f64,
    pub offset_tweens: (Option<Tween>, Option<Tween>),
    pub zoom: f64,
    pub zoom_tween: Option<Tween>,
    pub angle: f64,
    pub angle_target: f64,
    pub angle_tween: Option<Tween>,
    pub ground_hidden: bool,
    pub hide_p1: bool,
    pub hide_p2: bool,
    pub p1_controls_disabled: bool,
    pub p2_controls_disabled: bool,
    pub direction: f64,
    pub gameplay_sideways: bool,
    pub sideways_world_x: f64,
    pub gravity_scale: (f64, f64),
    pub attempts_hidden: bool,
    pub respawn_time: Option<f64>,
    pub static_camera: Option<StaticCam>,
    pub camera_edges: Option<BTreeMap<i64, i64>>,
    pub free_mode_camera: bool,
    pub free_cam_easing: f64,
    pub free_cam_padding: f64,
    pub pc_count: u64,
    pub multi_touch_legacy: Vec<(i64, f64, f64)>,
    pub touch_list: Vec<usize>,
    pub order: Vec<usize>,
    pub toggle_blocks: BTreeMap<usize, crate::collider::ToggleBlock>,
    pub sorted: bool,
    pub snapshot: Option<Vec<usize>>,
}

impl Default for GenState {
    fn default() -> Self {
        GenState {
            idx: 0, reverse_idx: -1, direction_dirty: false, fired: BTreeSet::new(), touching: BTreeMap::new(), sequence: BTreeMap::new(), scale_tweens: Vec::new(),
            time_events: Vec::new(), level_time: 0.0, points: 0.0, attempts: 0.0, timers: BTreeMap::new(), timer_seq: 0, persistent_timers: BTreeSet::new(),
            event_links: Vec::new(), event_stamps: BTreeMap::new(), group_scales: BTreeMap::new(), has_group_scales: false, scaled_colliders: BTreeSet::new(),
            link_visible: BTreeSet::new(), end_triggered: None, camera_offset: (0.0, 0.0), gameplay_offset: (0.0, 0.0), applied_camera_offset_y: 0.0,
            applied_gameplay_offset_y: 0.0, offset_tweens: (None, None), zoom: 1.0, zoom_tween: None, angle: 0.0, angle_target: 0.0, angle_tween: None,
            ground_hidden: false, hide_p1: false, hide_p2: false, p1_controls_disabled: false, p2_controls_disabled: false, direction: 1.0, gameplay_sideways: false, sideways_world_x: 0.0, gravity_scale: (1.0, 1.0),
            attempts_hidden: false, respawn_time: None, static_camera: None, camera_edges: None, free_mode_camera: false, free_cam_easing: 10.0, free_cam_padding: 0.5,
            pc_count: 0, multi_touch_legacy: Vec::new(), touch_list: Vec::new(), order: Vec::new(), toggle_blocks: BTreeMap::new(), sorted: false, snapshot: None,
        }
    }
}

fn apply_round(v: f64, mode: i64) -> f64 {
    match mode { 1 => crate::physics::js_round(v), 2 => v.floor(), 3 => v.ceil(), _ => v }
}
fn apply_sign(v: f64, mode: i64) -> f64 {
    match mode { 1 => v.abs(), 2 => -v.abs(), _ => v }
}
fn apply_op(a: f64, b: f64, op: i64) -> f64 {
    match op { 1 => a + b, 2 => a - b, 3 => a * b, 4 => if b != 0.0 { a / b } else { 0.0 }, _ => 0.0 }
}
fn trunc_mod(v: f64) -> f64 { (v * 1000.0).trunc() / 1000.0 }
fn has_item_source(ty: i64, id: i64) -> bool { ty != 0 && (id > 0 || ty == 3 || ty == 4) }
fn wrap_angle(a: f64) -> f64 {
    if a >= -180.0 && a <= 180.0 { return a; }
    let turns = ((a.abs() / 180.0).trunc() * 0.5).trunc();
    a + (if a < -180.0 { 360.0 } else { -360.0 }) * turns
}
fn event_key(event: i64, extra: i64, extra2: i64) -> u64 { ((event as f64) * 100000000.0 + (extra as f64) * 10000.0 + extra2 as f64).to_bits() }

#[derive(Clone, Copy)]
struct Eff { target: i64, center: i64, copy: bool }

impl Game {
    pub fn reset_generic(&mut self, clear_persistent: bool) {
        let kept: BTreeMap<i64, Timer> = if clear_persistent { BTreeMap::new() } else {
            let keep = self.trig.gen.persistent_timers.clone();
            self.trig.gen.timers.iter().filter(|(k, _)| keep.contains(k)).map(|(k, v)| (*k, v.clone())).collect()
        };
        let persistent_timers = if clear_persistent { BTreeSet::new() } else { self.trig.gen.persistent_timers.clone() };
        let link_visible = self.trig.gen.link_visible.clone();
        let multi_touch = std::mem::take(&mut self.trig.gen.multi_touch_legacy);
        let touch_list = std::mem::take(&mut self.trig.gen.touch_list);
        let order = std::mem::take(&mut self.trig.gen.order);
        let toggle_blocks = std::mem::take(&mut self.trig.gen.toggle_blocks);
        let sorted = self.trig.gen.sorted;
        let snapshot = self.trig.gen.snapshot.take();
        let timer_seq = self.trig.gen.timer_seq;
        let attempts = self.trig.gen.attempts;
        let pc_count = self.trig.gen.pc_count;
        let free_cam = (self.trig.gen.free_mode_camera, self.trig.gen.free_cam_easing, self.trig.gen.free_cam_padding);
        let old_scaled: Vec<usize> = self.trig.gen.scaled_colliders.iter().copied().collect();
        self.trig.gen = GenState::default();
        self.trig.gen.timers = kept;
        self.trig.gen.persistent_timers = persistent_timers;
        self.trig.gen.link_visible = link_visible;
        self.trig.gen.multi_touch_legacy = multi_touch;
        self.trig.gen.touch_list = touch_list;
        self.trig.gen.order = order;
        self.trig.gen.toggle_blocks = toggle_blocks;
        self.trig.gen.sorted = sorted;
        self.trig.gen.snapshot = snapshot;
        self.trig.gen.timer_seq = timer_seq;
        self.trig.gen.attempts = attempts;
        self.trig.gen.pc_count = pc_count;
        (self.trig.gen.free_mode_camera, self.trig.gen.free_cam_easing, self.trig.gen.free_cam_padding) = free_cam;
        self.trig.time_warp = 1.0;
        if clear_persistent { self.trig.persistent_items.clear(); }
        for ci in old_scaled {
            if let Some((w, h)) = self.trig.meta[ci].init_wh { self.cols[ci].w = w; self.cols[ci].h = h; }
        }
        self.sprite_reset_group_scales();
    }

    pub fn reset_time_warp(&mut self) {}

    pub fn rotate_gameplay(&mut self, t: &Gen) {
        if !self.rotate_gameplay_enabled || self.p.is_dead || self.e.is_dual { return; }
        let sideways = t.rg_ground == 3 || t.rg_ground == 4;
        let flipped = t.rg_ground == 1 || t.rg_ground == 4;
        let reversed = t.rg_move == 2 || t.rg_move == 3;
        let sideways_changed = self.trig.gen.gameplay_sideways != sideways;
        if !sideways_changed && (self.trig.gen.direction < 0.0) == reversed && self.p.gravity_flipped == flipped { return; }
        let mut velocity = self.g.player_speed;
        if t.edit_velocity { velocity = if t.override_velocity { t.velocity_mod_y * 2.0 } else { velocity * t.velocity_mod_y }; }
        self.trig.gen.gameplay_sideways = sideways;
        if (self.trig.gen.direction < 0.0) != reversed {
            self.trig.gen.direction = if reversed { -1.0 } else { 1.0 };
            self.trig.gen.direction_dirty = true;
        }
        if self.p.gravity_flipped != flipped { self.sim_for_pub(false, |s| s.flip_gravity(flipped, 1.0)); }
        if !sideways_changed { return; }
        self.sync_floor_ceiling();
        self.p.y_velocity = velocity;
        self.p.on_ground = false;
        self.p.on_ceiling = false;
        self.p.can_jump = false;
    }

    pub fn reset_ground_camera(&mut self) { self.trig.gen.free_mode_camera = false; self.trig.gen.free_cam_easing = 10.0; self.trig.gen.free_cam_padding = 0.5; }

    fn gen_dead(&self, ti: usize) -> bool { self.trig.dead(&self.trig.l.generics[ti].b) }

    fn eff(&self, ti: usize, remap: Option<&Remap>) -> Eff {
        let t = &self.trig.l.generics[ti];
        let mut e = Eff { target: t.target, center: t.center, copy: false };
        if let Some(r) = remap {
            if let Some(v) = r.get(&t.target) { e.target = *v; e.copy = true; }
            if let Some(v) = r.get(&t.center) { e.center = *v; e.copy = true; }
        }
        e
    }

    pub fn activate_toggle_block(&mut self, ci: usize) {
        let Some(tb) = self.trig.gen.toggle_blocks.get(&ci).copied() else { return };
        if !(tb.group > 0) { return; }
        if tb.spawn_only { self.spawn_group_now(tb.group); }
        else if tb.activate { self.toggle_group_triggered_pub(tb.group, true); }
        else { self.toggle_group_pub(tb.group, false); }
    }

    pub fn spawn_group_now_pub(&mut self, g: i64) { self.spawn_group_now(g); }

    pub fn activate_spawned_generic(&mut self, gid: i64, remap: Option<Remap>) {
        let order = self.trig.gen.snapshot.clone().unwrap_or_else(|| self.trig.gen.order.clone());
        let list: Vec<usize> = order.into_iter().filter(|&i| { let t = &self.trig.l.generics[i]; t.b.spawn_triggered && t.b.groups.contains(&gid) }).collect();
        for ti in list {
            if !self.trig.l.generics[ti].b.multi {
                let uid = self.trig.l.generics[ti].b.uid;
                if self.trig.gen.fired.contains(&uid) { continue; }
                self.trig.gen.fired.insert(uid);
            }
            let e = self.eff(ti, remap.as_ref());
            self.fire_generic(ti, e);
        }
    }

    fn get_item_value(&self, ty: i64, id: i64) -> f64 {
        match ty {
            2 => or0(self.trig.gen.timers.get(&id).map_or(0.0, |t| t.value)),
            3 => or0(self.trig.gen.points),
            4 => or0(self.trig.gen.level_time),
            5 => or0(self.trig.gen.attempts),
            _ => or0(self.trig.item_counts.get(&id).copied().unwrap_or(0.0)),
        }
    }

    fn set_item_value(&mut self, ty: i64, id: i64, value: f64) {
        if ty == 3 { self.trig.gen.points = value.trunc(); return; }
        if ty == 2 {
            let clamped = js_max(-9999999.0, js_min(9999999.0, value));
            let seq = self.trig.gen.timer_seq;
            if !self.trig.gen.timers.contains_key(&id) {
                self.trig.gen.timer_seq += 1;
                self.trig.gen.timers.insert(id, Timer { value: 0.0, running: false, time_mod: 1.0, stopped: false, ext: None, seq });
            }
            self.trig.gen.timers.get_mut(&id).unwrap().value = clamped;
            return;
        }
        self.trig.item_counts.insert(id, value.trunc());
        self.check_count_triggers_for(id as f64);
    }

    fn fire_generic(&mut self, ti: usize, e: Eff) {
        if self.gen_dead(ti) { return; }
        let t = self.trig.l.generics[ti].clone();
        if t.passive { return; }
        match t.kind {
            K::Random => { let r = m::random() * 100.0 < t.chance; self.spawn_group_now(if r { e.target } else { e.center }); }
            K::AdvRandom => {
                let mut total = 0i64;
                for c in &t.choices { total += c.1; }
                if total <= 0 { return; }
                let mut roll = m::random() * total as f64;
                for c in &t.choices {
                    roll -= c.1 as f64;
                    if roll < 0.0 { self.spawn_group_now(c.0); return; }
                }
                if let Some(c) = t.choices.last() { self.spawn_group_now(c.0); }
            }
            K::Sequence => self.fire_sequence(ti, &t, e.copy),
            K::ItemEdit => self.fire_item_edit(&t, e),
            K::ItemCompare => self.fire_item_compare(&t, e),
            K::ItemPersist => {
                let id = t.item1;
                if t.reset_item {
                    if t.target_all { if t.is_timer { self.trig.gen.persistent_timers.clear(); } else { self.trig.persistent_items.clear(); } }
                    else if t.is_timer { self.trig.gen.persistent_timers.remove(&id); } else { self.trig.persistent_items.remove(&id); }
                    if t.is_timer { self.trig.gen.timers.remove(&id); }
                    else if t.target_all { self.trig.item_counts.clear(); }
                    else { self.trig.item_counts.remove(&id); }
                } else if t.persistent {
                    if t.is_timer { self.trig.gen.persistent_timers.insert(id); } else { self.trig.persistent_items.insert(id); }
                } else if t.is_timer { self.trig.gen.persistent_timers.remove(&id); } else { self.trig.persistent_items.remove(&id); }
            }
            K::Timer => {
                let existed = self.trig.gen.timers.contains_key(&t.item1);
                if !existed {
                    let seq = self.trig.gen.timer_seq;
                    self.trig.gen.timer_seq += 1;
                    self.trig.gen.timers.insert(t.item1, Timer { value: 0.0, running: false, time_mod: 1.0, stopped: false, ext: None, seq });
                }
                let timer = self.trig.gen.timers.get_mut(&t.item1).unwrap();
                timer.running = !t.start_paused;
                timer.time_mod = t.time_mod;
                timer.ext = Some(TimerExt { ignore_tw: t.ignore_time_warp, stop_time: t.stop_time, stop_enabled: t.stop_time_enabled, target: e.target });
                timer.stopped = false;
                if !existed || !t.dont_override { timer.value = t.start_time; }
            }
            K::TimeEvent => {
                let entry = (ti, if e.copy { Some(e.target) } else { None });
                if !self.trig.gen.time_events.iter().any(|ev| *ev == entry && !e.copy) { self.trig.gen.time_events.push(entry); }
            }
            K::TimeControl => { if let Some(timer) = self.trig.gen.timers.get_mut(&t.item1) { timer.running = !t.stop; } }
            K::Scale => {
                if !(e.target > 0) { return; }
                self.trig.gen.has_group_scales = true;
                self.trig.gen.scale_tweens.push(ScaleTween { trig: ti, target: e.target, center: e.center, elapsed: 0.0, prev_x: 1.0, prev_y: 1.0 });
            }
            K::CameraOffset => {
                if t.axis != 2 { self.start_offset_tween(true, t.offset_x, ti); }
                if t.axis != 1 { self.start_offset_tween(false, t.offset_y, ti); }
            }
            K::GameplayOffset => {
                if t.mode != 2 && t.apply_x { self.trig.gen.gameplay_offset.0 = t.offset_x; }
                if t.mode != 1 && t.apply_y { self.trig.gen.gameplay_offset.1 = t.offset_y; }
            }
            K::Options => {
                let g = &mut self.trig.gen;
                if t.hide_ground != 0 { g.ground_hidden = t.hide_ground == 1; }
                if t.hide_p1 != 0 { g.hide_p1 = t.hide_p1 == 1; }
                if t.hide_p2 != 0 { g.hide_p2 = t.hide_p2 == 1; }
                if t.disable_p1 != 0 { g.p1_controls_disabled = t.disable_p1 == 1; }
                if t.disable_p2 != 0 { g.p2_controls_disabled = t.disable_p2 == 1; }
                if t.hide_attempts != 0 { g.attempts_hidden = t.hide_attempts == 1; }
                if t.respawn_mode != 0 { g.respawn_time = if t.respawn_mode == 1 { Some(js_min(10.0, js_max(1.0, t.respawn_time))) } else { None }; }
            }
            K::CameraRotate => self.start_camera_rotate(ti, &t),
            K::CameraZoom => {
                if t.duration <= 0.0 { self.trig.gen.zoom = t.zoom; self.trig.gen.zoom_tween = None; }
                else { let from = self.trig.gen.zoom; self.trig.gen.zoom_tween = Some(Tween { from, to: t.zoom, elapsed: 0.0, trig: ti }); }
            }
            K::TimeWarp => self.trig.time_warp = t.time_mod,
            K::End => {
                if self.trig.gen.end_triggered.is_none() { self.trig.gen.end_triggered = Some(ti); }
                self.spawn_group_now(e.target);
            }
            K::Event => {
                if e.target > 0 && !t.events.is_empty() {
                    let entry = (ti, if e.copy { Some(e.target) } else { None });
                    if e.copy || !self.trig.gen.event_links.contains(&entry) { self.trig.gen.event_links.push(entry); }
                }
            }
            K::InstantCollision => {
                let hit = if t.player_pair { self.collision_ids_touch(10000, 10001) } else {
                    let a = if t.p1 { 10000 } else if t.p2 { 10001 } else { t.block_a };
                    let mut h = self.collision_ids_touch(a, t.block_b);
                    if !h && t.p1 && t.p2 { h = self.collision_ids_touch(10001, t.block_b); }
                    h
                };
                self.spawn_group_now(if hit { e.target } else { e.center });
            }
            K::CameraEdge => {
                if t.edge >= 1 && t.edge <= 4 { self.trig.gen.camera_edges.get_or_insert_with(BTreeMap::new).insert(t.edge, t.edge_target); }
            }
            K::CameraMode => {
                if t.edit_settings { self.trig.gen.free_cam_easing = t.easing; self.trig.gen.free_cam_padding = t.padding; }
                let basic = !(self.p.is_flying || self.p.is_ufo || self.p.is_wave || self.p.is_ball || self.p.is_swing || self.p.is_spider);
                if t.free_mode && !basic && !self.e.is_dual { self.set_fly_mode_trigger_off(); }
                self.trig.gen.free_mode_camera = t.free_mode;
            }
            K::StaticCamera => {
                let cam = self.trig.gen.static_camera.get_or_insert_with(StaticCam::default);
                let axes: &[bool] = if t.axis == 1 { &[true] } else if t.axis == 2 { &[false] } else { &[true, false] };
                for &is_x in axes {
                    let prev = if is_x { cam.x.clone() } else { cam.y.clone() };
                    let next = if t.exit {
                        match &prev { Some(p) if !p.exiting => Some(StaticAxis { group: p.group, elapsed: 0.0, trig: ti, exiting: true, instant: t.exit_instant, from: p.from, last: p.last, target: p.target }), _ => prev.clone() }
                    } else {
                        Some(StaticAxis { group: e.target, elapsed: 0.0, trig: ti, exiting: false, instant: false, from: prev.as_ref().and_then(|p| p.last), last: None, target: 0.0 })
                    };
                    if is_x { cam.x = next; } else { cam.y = next; }
                }
            }
            K::RotateGameplay => {
                if !t.channel_only { self.rotate_gameplay(&t); }
            }
            K::Reverse => {
                let d = if self.trig.gen.direction == 0.0 || self.trig.gen.direction.is_nan() { 1.0 } else { self.trig.gen.direction };
                self.trig.gen.direction = d * -1.0;
                self.game_event(75, 0, 0);
                self.trig.gen.direction_dirty = true;
            }
            K::Gravity => {
                if t.p1 { self.trig.gen.gravity_scale.0 = t.gravity; }
                if t.p2 { self.trig.gen.gravity_scale.1 = t.gravity; }
            }
            K::PlayerControl => self.trig.gen.pc_count += 1,
            K::Reset => self.fire_reset(e.target),
            _ => {}
        }
    }

    fn set_fly_mode_trigger_off(&mut self) {}

    fn fire_sequence(&mut self, ti: usize, t: &Gen, copy: bool) {
        if t.sequence_total == 0 { return; }
        let mut state = if copy { SeqState { index: 0, last_time: -1.0 } } else { self.trig.gen.sequence.get(&ti).cloned().unwrap_or(SeqState { index: 0, last_time: -1.0 }) };
        let now = self.trig.gen.level_time;
        let elapsed = now - state.last_time;
        if t.min_interval > 0.0 && state.last_time >= 0.0 && elapsed < t.min_interval {
            if !copy { self.trig.gen.sequence.insert(ti, state); }
            return;
        }
        let mut index = state.index;
        if index > 0 && t.reset_time > 0.0 && state.last_time >= 0.0 && elapsed > t.reset_time {
            index = if t.reset_step { (index - (elapsed / t.reset_time).floor() as i64).max(0) } else { 0 };
        }
        state.last_time = now;
        let mut position = index + 1;
        if index >= t.sequence_total {
            if t.mode == 0 {
                state.index = index;
                if !copy { self.trig.gen.sequence.insert(ti, state); }
                return;
            }
            position = if t.mode == 1 { 1 } else { t.sequence_total };
        }
        state.index = position;
        if !copy { self.trig.gen.sequence.insert(ti, state); }
        let mut covered = 0;
        for entry in &t.sequence {
            covered += entry.1;
            if position <= covered { self.spawn_group_now(entry.0); return; }
        }
    }

    fn fire_item_edit(&mut self, t: &Gen, e: Eff) {
        let target_type = if t.target_type < 2 { 1 } else { t.target_type };
        if e.target < 1 && target_type != 3 { return; }
        let has1 = has_item_source(t.type1, t.item1);
        let has2 = has_item_source(t.type2, t.item2);
        let md = trunc_mod(t.mod1);
        let mut value = md;
        if has1 || has2 {
            let source = if has1 && has2 {
                apply_op(self.get_item_value(t.type1, t.item1), self.get_item_value(t.type2, t.item2), if t.op2 < 2 { 1 } else { t.op2 })
            } else if has1 { self.get_item_value(t.type1, t.item1) } else { self.get_item_value(t.type2, t.item2) };
            value = apply_op(source, md, if t.op3 < 4 { 3 } else { 4 });
        }
        value = apply_sign(apply_round(value, t.round1), t.sign1);
        if t.op1 != 0 { value = apply_op(self.get_item_value(target_type, e.target), value, t.op1); }
        value = apply_sign(apply_round(value, t.round2), t.sign2);
        let id = e.target.min(9999).max(0);
        self.set_item_value(target_type, id, value);
    }

    fn fire_item_compare(&mut self, t: &Gen, e: Eff) {
        let type1 = if t.type1 < 2 { 1 } else { t.type1 };
        let left = apply_sign(apply_round(apply_op(self.get_item_value(type1, t.item1), trunc_mod(t.mod1), if t.op1 > 0 { t.op1 } else { 3 }), t.round1), t.sign1);
        let mod2 = trunc_mod(t.mod2);
        let mut right = mod2;
        if has_item_source(t.type2, t.item2) {
            right = apply_op(self.get_item_value(t.type2, t.item2), mod2, if t.op2 > 0 { t.op2 } else { 3 });
        }
        right = apply_sign(apply_round(right, t.round2), t.sign2);
        let tol = trunc_mod(t.tolerance);
        let matched = match t.op3 {
            1 => left + tol > right,
            2 => left + tol >= right,
            3 => left - tol < right,
            4 => left - tol <= right,
            5 => (left - right).abs() > tol,
            _ => (left - right).abs() <= tol,
        };
        self.spawn_group_now(if matched { e.target } else { e.center });
    }

    fn fire_reset(&mut self, target: i64) {
        if !(target > 0) { return; }
        let mut uids: Vec<i64> = Vec::new();
        macro_rules! collect { ($l:ident) => { for t in &self.trig.l.$l { if t.b.groups.contains(&target) { uids.push(t.b.uid); } } }; }
        collect!(generics); collect!(colors); collect!(moves); collect!(alphas); collect!(spawns); collect!(stops); collect!(follow_ys); collect!(follows);
        collect!(toggles); collect!(animations); collect!(shakes); collect!(player_vis); collect!(on_deaths); collect!(touches); collect!(pickups); collect!(counts);
        collect!(instant_counts); collect!(rotates); collect!(pulses); collect!(collisions);
        for uid in uids {
            self.trig.gen.fired.remove(&uid);
            self.trig.spawn_fired.remove(&uid);
            self.trig.release_touch(uid);
        }
    }

    pub fn game_event(&mut self, event: i64, extra: i64, extra2: i64) {
        if self.trig.gen.event_links.is_empty() { return; }
        let stamp = self.e.physics_tick;
        let k = event_key(event, extra, extra2);
        if self.trig.gen.event_stamps.get(&k).copied() == Some(stamp) { return; }
        self.trig.gen.event_stamps.insert(k, stamp);
        let links = self.trig.gen.event_links.clone();
        for (ti, tg) in links {
            let (ex, ex2, has, target) = { let t = &self.trig.l.generics[ti]; (t.extra_id, t.extra_id2, t.events.contains(&event), tg.unwrap_or(t.target)) };
            if ex != extra || (ex2 != extra2 && ex2 != 0) { continue; }
            if !has || self.gen_dead(ti) { continue; }
            self.spawn_group_now(target);
        }
    }

    fn player_box(&self, id: i64) -> Option<(f64, f64)> {
        if id != 10000 && id != 10001 { return None; }
        let y = if id == 10000 { self.trig.pos.player_y } else if self.e.is_dual { self.p2.y } else { return None };
        Some((or0(self.trig.pos.player_x), or0(y)))
    }

    fn collision_ids_touch(&self, a: i64, b: i64) -> bool {
        let pa = self.player_box(a);
        let pb = self.player_box(b);
        let half = 20.0;
        match (pa, pb) {
            (None, None) => self.blocks_touch_pub(a as f64, b as f64),
            (Some(p), Some(q)) => (p.0 - q.0).abs() < half + half && (p.1 - q.1).abs() < half + half,
            _ => {
                let (player, other) = match pa { Some(p) => (p, b), None => (pb.unwrap(), a) };
                for blk in &self.trig.l.blocks {
                    if blk.block_id != other as f64 || !self.block_enabled_pub(blk) { continue; }
                    let off = self.group_offset_sum_pub(&blk.groups);
                    let reach = blk.half + half;
                    if (blk.x + off.0 - player.0).abs() < reach && (blk.y + off.1 - player.1).abs() < reach { return true; }
                }
                false
            }
        }
    }

    fn start_offset_tween(&mut self, is_x: bool, target: f64, ti: usize) {
        let dur = self.trig.l.generics[ti].duration;
        if dur <= 0.0 {
            if is_x { self.trig.gen.camera_offset.0 = target; self.trig.gen.offset_tweens.0 = None; }
            else { self.trig.gen.camera_offset.1 = target; self.trig.gen.offset_tweens.1 = None; }
            return;
        }
        let from = if is_x { self.trig.gen.camera_offset.0 } else { self.trig.gen.camera_offset.1 };
        let tw = Some(Tween { from, to: target, elapsed: 0.0, trig: ti });
        if is_x { self.trig.gen.offset_tweens.0 = tw; } else { self.trig.gen.offset_tweens.1 = tw; }
    }

    fn start_camera_rotate(&mut self, ti: usize, t: &Gen) {
        let mut target = if t.add { self.trig.gen.angle_target + t.degrees } else { t.degrees };
        if t.snap360 {
            self.trig.gen.angle = wrap_angle(self.trig.gen.angle);
            target = wrap_angle(target);
        }
        self.trig.gen.angle_target = target;
        if t.duration <= 0.0 {
            self.trig.gen.angle = t.degrees;
            self.trig.gen.angle_tween = None;
            return;
        }
        let from = self.trig.gen.angle;
        self.trig.gen.angle_tween = Some(Tween { from, to: target, elapsed: 0.0, trig: ti });
    }

    fn step_camera_tweens(&mut self, dt: f64) {
        for is_x in [true, false] {
            let tw = if is_x { self.trig.gen.offset_tweens.0.clone() } else { self.trig.gen.offset_tweens.1.clone() };
            let Some(mut tw) = tw else { continue };
            tw.elapsed += dt;
            let (dur, et, er) = { let t = &self.trig.l.generics[tw.trig]; (t.duration, t.easing_type, t.easing_rate) };
            let progress = js_min(tw.elapsed / dur, 1.0);
            let sample = easing(et as f64, er, progress);
            let v = tw.from + (tw.to - tw.from) * sample;
            if is_x { self.trig.gen.camera_offset.0 = v; } else { self.trig.gen.camera_offset.1 = v; }
            let keep = if progress >= 1.0 { None } else { Some(tw) };
            if is_x { self.trig.gen.offset_tweens.0 = keep; } else { self.trig.gen.offset_tweens.1 = keep; }
        }
        if let Some(mut tw) = self.trig.gen.angle_tween.clone() {
            tw.elapsed += dt;
            let (dur, et, er) = { let t = &self.trig.l.generics[tw.trig]; (t.duration, t.easing_type, t.easing_rate) };
            let progress = js_min(tw.elapsed / dur, 1.0);
            let sample = easing(et as f64, er, progress);
            self.trig.gen.angle = tw.from + (tw.to - tw.from) * sample;
            self.trig.gen.angle_tween = if progress >= 1.0 { None } else { Some(tw) };
        }
        if let Some(mut tw) = self.trig.gen.zoom_tween.clone() {
            tw.elapsed += dt;
            let (dur, et, er) = { let t = &self.trig.l.generics[tw.trig]; (t.duration, t.easing_type, t.easing_rate) };
            let progress = js_min(tw.elapsed / dur, 1.0);
            let sample = easing(et as f64, er, progress);
            self.trig.gen.zoom = tw.from + (tw.to - tw.from) * sample;
            self.trig.gen.zoom_tween = if progress >= 1.0 { None } else { Some(tw) };
        }
    }

    pub fn view_pad(&self) -> f64 {
        let zoom = if self.trig.gen.zoom == 0.0 || self.trig.gen.zoom.is_nan() { 1.0 } else { self.trig.gen.zoom };
        let angle = if self.trig.gen.angle.is_nan() { 0.0 } else { self.trig.gen.angle };
        let radians = angle * std::f64::consts::PI / 180.0;
        let sw = self.screen_width;
        let visible = (sw * m::cos(radians).abs() + 640.0 * m::sin(radians).abs()) / zoom;
        if visible > sw { (visible - sw) / 2.0 } else { 0.0 }
    }

    fn step_timers(&mut self, dt: f64, raw_dt: f64) {
        let mut order: Vec<i64> = self.trig.gen.timers.keys().copied().collect();
        order.sort_by_key(|k| { let timer = &self.trig.gen.timers[k]; if *k >= 0 && *k < 4294967295 { (0u8, *k as u64) } else { (1u8, timer.seq) } });
        for id in order {
            let Some(timer) = self.trig.gen.timers.get(&id).cloned() else { continue };
            if !timer.running || timer.stopped { continue; }
            let before = timer.value;
            let ignore = timer.ext.as_ref().map_or(false, |x| x.ignore_tw);
            let mut after = before + timer.time_mod * (if ignore { raw_dt } else { dt });
            let mut stopped = false;
            let mut spawn_target: Option<i64> = None;
            if let Some(x) = &timer.ext {
                if x.stop_enabled {
                    let stop = x.stop_time;
                    if (before < stop && after >= stop) || (before > stop && after <= stop) {
                        after = stop;
                        stopped = true;
                        spawn_target = Some(x.target);
                    }
                }
            }
            if let Some(tm) = self.trig.gen.timers.get_mut(&id) { if stopped { tm.stopped = true; } }
            if let Some(tg) = spawn_target { self.spawn_group_now(tg); }
            if let Some(tm) = self.trig.gen.timers.get_mut(&id) { tm.value = after; }
            let mut i = self.trig.gen.time_events.len() as isize - 1;
            while i >= 0 {
                let Some(&(ti, tg)) = self.trig.gen.time_events.get(i as usize) else { i -= 1; continue };
                let (item, target, target_group, multi) = { let e = &self.trig.l.generics[ti]; (e.item1, e.target_time, tg.unwrap_or(e.target), e.multi_activate) };
                if item == id && ((before < target && after >= target) || (before > target && after <= target)) {
                    self.spawn_group_now(target_group);
                    if !multi { if (i as usize) < self.trig.gen.time_events.len() { self.trig.gen.time_events.remove(i as usize); } }
                }
                i -= 1;
            }
        }
    }

    fn step_scale_tweens(&mut self, dt: f64) {
        if self.trig.gen.scale_tweens.is_empty() { return; }
        let mut local: Vec<i64> = Vec::new();
        let mut i = 0;
        while i < self.trig.gen.scale_tweens.len() {
            let anim = self.trig.gen.scale_tweens[i].clone();
            let t = self.trig.l.generics[anim.trig].clone();
            let elapsed = anim.elapsed + dt;
            let progress = if t.duration > 0.0 { js_min(elapsed / t.duration, 1.0) } else { 1.0 };
            let sample = easing(t.easing_type as f64, t.easing_rate, progress);
            let fx = 1.0 + (t.scale_x - 1.0) * sample;
            let fy = 1.0 + (t.scale_y - 1.0) * sample;
            let st = self.trig.gen.group_scales.entry(anim.target).or_insert(GroupScale { sx: 1.0, sy: 1.0, center: 0, only_move: false });
            if anim.prev_x != 0.0 { st.sx *= fx / anim.prev_x; }
            if anim.prev_y != 0.0 { st.sy *= fy / anim.prev_y; }
            st.center = anim.center;
            st.only_move = t.only_move;
            {
                let a = &mut self.trig.gen.scale_tweens[i];
                a.elapsed = elapsed; a.prev_x = fx; a.prev_y = fy;
            }
            if self.trig.defer { push_unique(&mut self.trig.pending, anim.target); } else { push_unique(&mut local, anim.target); }
            if progress >= 1.0 { self.trig.gen.scale_tweens.remove(i); } else { i += 1; }
        }
        if !self.trig.defer { self.apply_group_transforms_pub(&local); }
    }

    fn check_generic_touch(&mut self, player_y: f64, slot: i64, player_x: f64) {
        let touch_list = self.trig.gen.touch_list.clone();
        for ti in touch_list {
            let t = &self.trig.l.generics[ti];
            if t.b.spawn_triggered { continue; }
            let inside = (player_x - t.x).abs() <= TOUCH_HALF_HITBOX && (player_y - t.y).abs() <= TOUCH_HALF_HITBOX;
            let k = t.b.uid * 2 + slot;
            let was = self.trig.gen.touching.get(&k).copied() == Some(true);
            if inside == was { continue; }
            self.trig.gen.touching.insert(k, inside);
            if !inside { continue; }
            let (multi, uid) = (t.b.multi, t.b.uid);
            if !multi {
                if self.trig.gen.fired.contains(&uid) { continue; }
                self.trig.gen.fired.insert(uid);
            }
            let e = self.eff(ti, None);
            self.fire_generic(ti, e);
        }
    }

    fn release_multi_touch(&mut self, px: f64, py: f64, py2: Option<f64>) {
        let list = self.trig.gen.multi_touch_legacy.clone();
        for (uid, x, y) in list {
            if (px - x).abs() > TOUCH_HALF_HITBOX { self.trig.release_touch(uid); continue; }
            let touching = (py - y).abs() <= TOUCH_HALF_HITBOX || py2.map_or(false, |v| (v - y).abs() <= TOUCH_HALF_HITBOX);
            if touching { continue; }
            self.trig.release_touch(uid);
        }
    }

    pub fn update_generic(&mut self, px: f64, py: Option<f64>, py2: Option<f64>, raw_dt: f64) {
        let tw = if self.trig.time_warp == 0.0 || self.trig.time_warp.is_nan() { 1.0 } else { self.trig.time_warp };
        let dt = raw_dt * tw;
        self.trig.gen.level_time += dt;
        let n = self.trig.l.generics.len();
        if n > 0 {
            if !self.trig.gen.sorted {
                self.trig.gen.sorted = true;
                let gens = &self.trig.l.generics;
                let mut order = self.trig.gen.order.clone();
                order.sort_by(|&a, &b| (gens[a].x - gens[b].x).partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal));
                self.trig.gen.touch_list = order.iter().copied().filter(|&i| gens[i].touch).collect();
                self.trig.gen.order = order;
            }
            let gx = |s: &Game, p: usize| s.trig.l.generics[s.trig.gen.order[p]].x;
            if self.trig.gen.direction_dirty {
                self.trig.gen.direction_dirty = false;
                let (mut lo, mut hi) = (0usize, n);
                while lo < hi { let mid = (lo + hi) >> 1; if gx(self, mid) < px { lo = mid + 1; } else { hi = mid; } }
                self.trig.gen.reverse_idx = lo as i64 - 1;
                let (mut fw, mut hi) = (0usize, n);
                while fw < hi { let mid = (fw + hi) >> 1; if gx(self, mid) <= px { fw = mid + 1; } else { hi = mid; } }
                self.trig.gen.idx = fw;
            }
            let dir = |s: &Game| if s.trig.gen.direction == 0.0 || s.trig.gen.direction.is_nan() { 1.0 } else { s.trig.gen.direction };
            if dir(self) < 0.0 {
                while self.trig.gen.reverse_idx >= 0 {
                    let ri = self.trig.gen.order[self.trig.gen.reverse_idx as usize];
                    let (tx, sp, tc, multi, uid) = { let t = &self.trig.l.generics[ri]; (t.x, t.b.spawn_triggered, t.touch, t.b.multi, t.b.uid) };
                    if tx <= px { break; }
                    if dir(self) > 0.0 { break; }
                    self.trig.gen.reverse_idx -= 1;
                    if sp || tc { continue; }
                    if !multi && self.trig.gen.fired.contains(&uid) { continue; }
                    self.trig.gen.fired.insert(uid);
                    let e = self.eff(ri, None);
                    self.fire_generic(ri, e);
                }
            } else {
                while self.trig.gen.idx < n {
                    if dir(self) < 0.0 { break; }
                    let gi = self.trig.gen.order[self.trig.gen.idx];
                    let (tx, sp, tc) = { let t = &self.trig.l.generics[gi]; (t.x, t.b.spawn_triggered, t.touch) };
                    if tx > px { break; }
                    if !sp && !tc { let e = self.eff(gi, None); self.fire_generic(gi, e); }
                    self.trig.gen.idx += 1;
                }
            }
            if !self.trig.gen.touch_list.is_empty() {
                if let Some(py) = py {
                    self.check_generic_touch(or0(py), 0, px);
                    if let Some(y2) = py2 { self.check_generic_touch(or0(y2), 1, px); }
                }
            }
        }
        if let Some(py) = py { self.release_multi_touch(px, or0(py), py2); }
        self.step_timers(dt, raw_dt);
        self.step_scale_tweens(dt);
        self.step_camera_tweens(dt);
    }

    pub fn apply_group_scale_to_collider(&mut self, ci: usize, ids: &[i64], fx: &mut f64, fy: &mut f64) {
        let mut scale_x = 1.0;
        let mut scale_y = 1.0;
        for gid in ids {
            let Some(st) = self.trig.gen.group_scales.get(gid).cloned() else { continue };
            if st.center > 0 {
                let (cx, cy) = self.center_cached(st.center);
                let center_y = 460.0 - cy;
                *fx = cx + (*fx - cx) * st.sx;
                *fy = center_y + (*fy - center_y) * st.sy;
            }
            if !st.only_move { scale_x *= st.sx; scale_y *= st.sy; }
        }
        if self.cols[ci].ty == crate::pcol::type_code("slope") { return; }
        if self.trig.meta[ci].init_wh.is_none() { self.trig.meta[ci].init_wh = Some((self.cols[ci].w, self.cols[ci].h)); }
        let (iw, ih) = self.trig.meta[ci].init_wh.unwrap();
        let width = iw * scale_x.abs();
        let height = ih * scale_y.abs();
        if self.cols[ci].w != width || self.cols[ci].h != height {
            self.cols[ci].w = width;
            self.cols[ci].h = height;
            self.trig.gen.scaled_colliders.insert(ci);
        }
    }

    pub fn generic_state_json(&self) -> String {
        let g = &self.trig.gen;
        let f = |v: f64| num(v);
        let tween = |t: &Option<Tween>| t.as_ref().map_or("null".to_string(), |t| format!("[{},{},{}]", f(t.from), f(t.to), f(t.elapsed)));
        let mut timers: Vec<String> = g.timers.iter().map(|(k, t)| {
            let mut s = format!("\"{}\":{{\"value\":{},\"running\":{},\"timeMod\":{},\"stopped\":{}", k, f(t.value), t.running, f(t.time_mod), t.stopped);
            if let Some(x) = &t.ext { s.push_str(&format!(",\"ignoreTimeWarp\":{},\"stopTime\":{},\"stopTimeEnabled\":{},\"targetGroup\":{}", x.ignore_tw, f(x.stop_time), x.stop_enabled, x.target)); }
            s.push('}');
            s
        }).collect();
        timers.sort();
        let mut seqs_v: Vec<(i64, String)> = g.sequence.iter().map(|(k, s)| (self.gpos(*k), format!("[{},{},{}]", self.gpos(*k), s.index, f(s.last_time)))).collect();
        seqs_v.sort_by_key(|e| e.0);
        let seqs: Vec<String> = seqs_v.into_iter().map(|e| e.1).collect();
        let stamps: Vec<String> = g.event_stamps.iter().map(|(k, v)| format!("[{},{}]", f(f64::from_bits(*k)), f(*v))).collect();
        let ints = |v: Vec<i64>| format!("[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","));
        let scales: Vec<String> = g.group_scales.iter().map(|(k, s)| format!("\"{}\":[{},{},{},{}]", k, f(s.sx), f(s.sy), s.center, s.only_move)).collect();
        let st_axis = |a: &Option<StaticAxis>| a.as_ref().map_or("null".to_string(), |a| format!("[{},{},{},{},{}]", a.group, f(a.elapsed), a.exiting, a.from.map_or("null".to_string(), f), a.instant));
        let static_cam = g.static_camera.as_ref().map_or("null".to_string(), |c| format!("[{},{}]", st_axis(&c.x), st_axis(&c.y)));
        let edges = g.camera_edges.as_ref().map_or("null".to_string(), |e| format!("{{{}}}", e.iter().map(|(k, v)| format!("\"{}\":{}", k, v)).collect::<Vec<_>>().join(",")));
        format!("{{\"idx\":[{},{},{}],\"fired\":{},\"touching\":[{}],\"sequence\":[{}],\"scaleTweens\":[{}],\"timeEvents\":[{}],\"levelTime\":{},\"points\":{},\"timeWarp\":{},\
\"timers\":{{{}}},\"persistentItems\":{},\"persistentTimers\":{},\"eventLinks\":[{}],\"eventStamps\":[{}],\"groupScales\":{{{}}},\"hasGroupScales\":{},\"scaledColliders\":{},\"linkVisible\":{},\
\"endTriggered\":{},\"cameraOffset\":[{},{},{},{}],\"offsetTweens\":[{},{}],\"zoom\":[{},{}],\"angle\":[{},{},{}],\"flags\":[{},{},{},{},{},{},{}],\"direction\":{},\"gravity\":[{},{}],\"respawnTime\":{},\
\"staticCamera\":{},\"cameraEdges\":{},\"freeCam\":[{},{},{}],\"spawnFired\":{}}}",
            g.idx, g.reverse_idx, g.direction_dirty, ints(g.fired.iter().copied().collect()),
            g.touching.iter().map(|(k, v)| format!("[{},{}]", k, v)).collect::<Vec<_>>().join(","),
            seqs.join(","),
            g.scale_tweens.iter().map(|a| format!("[{},{},{},{},{},{}]", self.gpos(a.trig), a.target, a.center, f(a.elapsed), f(a.prev_x), f(a.prev_y))).collect::<Vec<_>>().join(","),
            g.time_events.iter().map(|e| format!("[{},{}]", self.gpos(e.0), e.1.map_or("null".to_string(), |v| v.to_string()))).collect::<Vec<_>>().join(","),
            f(g.level_time), f(g.points), f(self.trig.time_warp),
            timers.join(","), ints(self.trig.persistent_items.iter().copied().collect()), ints(g.persistent_timers.iter().copied().collect()),
            g.event_links.iter().map(|e| format!("[{},{}]", self.gpos(e.0), e.1.map_or("null".to_string(), |v| v.to_string()))).collect::<Vec<_>>().join(","),
            stamps.join(","), scales.join(","), g.has_group_scales, ints(g.scaled_colliders.iter().map(|c| *c as i64).collect()), ints(g.link_visible.iter().copied().collect()),
            g.end_triggered.map_or(-1, |i| self.gpos(i)), f(g.camera_offset.0), f(g.camera_offset.1), f(g.gameplay_offset.0), f(g.gameplay_offset.1),
            tween(&g.offset_tweens.0), tween(&g.offset_tweens.1), f(g.zoom), tween(&g.zoom_tween), f(g.angle), f(g.angle_target), tween(&g.angle_tween),
            g.ground_hidden, g.hide_p1, g.hide_p2, g.p1_controls_disabled, g.p2_controls_disabled, g.attempts_hidden, g.free_mode_camera,
            f(g.direction), f(g.gravity_scale.0), f(g.gravity_scale.1), g.respawn_time.map_or("null".to_string(), f),
            static_cam, edges, f(g.free_cam_easing), f(g.free_cam_padding), g.pc_count,
            ints(self.trig.spawn_fired.iter().copied().collect()))
    }

    pub fn generic_lists_json(&self) -> String {
        format!("[{}]", self.trig.gen.order.iter().map(|&i| gen_json(&self.trig.l.generics[i])).collect::<Vec<_>>().join(","))
    }

    fn gpos(&self, ti: usize) -> i64 { self.trig.gen.order.iter().position(|&i| i == ti).map_or(-1, |p| p as i64) }
}

pub fn gen_list_json(v: &[Gen]) -> String { format!("[{}]", v.iter().map(gen_json).collect::<Vec<_>>().join(",")) }

pub fn build_generic_extras(l: &mut triggers::Lists, gen: &mut GenState) {
    let mut seen: BTreeSet<i64> = BTreeSet::new();
    gen.multi_touch_legacy.clear();
    macro_rules! legacy { ($l:ident) => { for t in &l.$l { if t.touch && t.b.multi && !t.b.spawn_triggered && seen.insert(t.b.uid) { gen.multi_touch_legacy.push((t.b.uid, t.x, t.y)); } } }; }
    legacy!(colors); legacy!(moves); legacy!(alphas); legacy!(spawns); legacy!(stops); legacy!(follow_ys); legacy!(follows); legacy!(toggles);
    legacy!(animations); legacy!(shakes); legacy!(player_vis); legacy!(on_deaths); legacy!(touches); legacy!(pickups); legacy!(counts); legacy!(instant_counts);
    gen.order = (0..l.generics.len()).collect();
    gen.sorted = false;
    gen.snapshot = None;
    gen.touch_list = Vec::new();
    for t in &l.generics { if t.kind == K::LinkVisible && t.target > 0 { gen.link_visible.insert(t.target); } }
}

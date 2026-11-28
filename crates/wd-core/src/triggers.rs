//! Level triggers that change what the player can collide with: move, rotate, spawn, stop and toggle
//! (level.js `_spawnObject` trigger branch, `check*Triggers`, `step*Triggers`, group transforms) and the
//! trigger-update order of game-scene.js `update`.
//! Not ported yet: alpha / pulse / color (visual only), follow, follow-Y, touch, pickup, count, instant count,
//! collision, on-death, shake, animation and player-visibility triggers. A spawn trigger ignores groups of those kinds.
use crate::game::Game;
use crate::js::{parse_float, parse_int, to_number};
use crate::json::{num, str_lit, Obj};
use crate::jsmath as m;
use crate::level::LevelObject;
use std::collections::{BTreeMap, BTreeSet};

const PI: f64 = std::f64::consts::PI;
/// `30 + playerSize`, where `playerSize` is never defined in the game, so the fallback 20 applies.
pub(crate) const TOUCH_HALF_HITBOX: f64 = 50.0;
/// Key used for a group id that is NaN (`obj["NaN"]` in JS).
const NAN_KEY: i64 = i64::MIN;

pub(crate) fn or0(x: f64) -> f64 { if x == 0.0 || x.is_nan() { 0.0 } else { x } }
pub(crate) fn key(x: f64) -> i64 { if x.is_nan() { NAN_KEY } else { x as i64 } }
pub(crate) fn js_min(a: f64, b: f64) -> f64 { if a.is_nan() || b.is_nan() { f64::NAN } else if a < b { a } else { b } }
fn js_sign(x: f64) -> f64 { if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { x } }
pub(crate) fn js_max(a: f64, b: f64) -> f64 { if a.is_nan() || b.is_nan() { f64::NAN } else if a > b { a } else { b } }

// ---- Easing (triggers.js) ----
pub fn easing(ty: f64, rate: f64, x: f64) -> f64 {
    if x == 0.0 || x == 1.0 { return x; }
    let bounce_time = |x: f64| -> f64 {
        if x < 1.0 / 2.75 { 7.5625 * x * x }
        else if x < 2.0 / 2.75 { let t = x - 1.5 / 2.75; 7.5625 * t * t + 0.75 }
        else if x < 2.5 / 2.75 { let t = x - 2.25 / 2.75; 7.5625 * t * t + 0.9375 }
        else { let t = x - 2.625 / 2.75; 7.5625 * t * t + 0.984375 }
    };
    let k = if ty.is_nan() { -1 } else { ty as i64 };
    match k {
        0 => x,
        1 => { let t = x * 2.0; if t < 1.0 { 0.5 * m::pow(t, rate) } else { 1.0 - 0.5 * m::pow(2.0 - t, rate) } }
        2 => m::pow(x, rate),
        3 => m::pow(x, 1.0 / rate),
        4 => {
            let period = if rate == 0.0 || rate.is_nan() { 0.3 * 1.5 } else { rate };
            let s = period / 4.0; let t = x - 1.0;
            if t < 0.0 { -0.5 * m::pow(2.0, 10.0 * t) * m::sin((t - s) * (2.0 * PI) / period) }
            else { m::pow(2.0, -10.0 * t) * m::sin((t - s) * (2.0 * PI) / period) * 0.5 + 1.0 }
        }
        5 => { let s = rate / 4.0; let t = x - 1.0; -m::pow(2.0, 10.0 * t) * m::sin((t - s) * (2.0 * PI) / rate) }
        6 => { let s = rate / 4.0; m::pow(2.0, -10.0 * x) * m::sin((x - s) * (2.0 * PI) / rate) + 1.0 }
        7 => if x < 0.5 { (1.0 - bounce_time(1.0 - x * 2.0)) * 0.5 } else { bounce_time(x * 2.0 - 1.0) * 0.5 + 0.5 },
        8 => 1.0 - bounce_time(1.0 - x),
        9 => bounce_time(x),
        10 => if x < 0.5 { 0.5 * m::pow(2.0, 10.0 * (x * 2.0 - 1.0)) } else { 0.5 * (-m::pow(2.0, -10.0 * (x * 2.0 - 1.0)) + 2.0) },
        11 => m::pow(2.0, 10.0 * (x - 1.0)) - 0.001,
        12 => -m::pow(2.0, -10.0 * x) + 1.0,
        13 => -0.5 * (m::cos(x * PI) - 1.0),
        14 => 1.0 - m::cos((x * PI) / 2.0),
        15 => m::sin((x * PI) / 2.0),
        16 => {
            let ov = 1.70158 * 1.525; let t = x * 2.0;
            if t < 1.0 { (t * t * ((ov + 1.0) * t - ov)) / 2.0 } else { ((t - 2.0) * (t - 2.0) * ((ov + 1.0) * (t - 2.0) + ov)) / 2.0 + 1.0 }
        }
        17 => { let ov = 1.70158; x * x * ((ov + 1.0) * x - ov) }
        18 => { let ov = 1.70158; let t = x - 1.0; t * t * ((ov + 1.0) * t + ov) + 1.0 }
        _ => x,
    }
}

// ---- trigger definitions ----
#[derive(Clone, Debug)]
pub struct Base { pub uid: i64, pub spawn_triggered: bool, pub multi: bool, pub channel: i64, pub ord: i64, pub groups: Vec<i64> }
#[derive(Clone, Debug)]
pub struct Move { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub duration: f64, pub easing_type: f64, pub easing_rate: f64, pub target: i64,
    pub offset_x: f64, pub offset_y: f64, pub lock_x: bool, pub lock_y: bool, pub lock_cam_x: bool, pub lock_cam_y: bool }
#[derive(Clone, Debug)]
pub struct Rotate { pub b: Base, pub x: f64, pub target: i64, pub degrees: f64, pub duration: f64, pub easing_type: f64, pub easing_rate: f64,
    pub lock_rotation: bool, pub times360: f64, pub center: i64 }
#[derive(Clone, Debug)]
pub struct Spawn { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: i64, pub delay: f64, pub random_delay: f64, pub remap: Option<Remap> }
#[derive(Clone, Debug)]
pub struct Stop { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: i64 }
#[derive(Clone, Debug)]
pub struct Toggle { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: i64, pub activate: bool }

#[derive(Clone, Debug)]
pub struct FollowY { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: i64, pub speed: f64, pub delay: f64, pub offset: f64, pub max_speed: f64, pub duration: f64 }
#[derive(Clone, Debug)]
pub struct Follow { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: i64, pub follow_group: i64, pub x_mod: f64, pub y_mod: f64, pub duration: f64 }

#[derive(Clone, Debug)]
pub struct OnDeath { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: i64, pub activate: bool }
#[derive(Clone, Debug)]
pub struct TouchTrig { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: i64, pub toggle_type: f64, pub dual_touch: bool, pub hold_mode: bool, pub player_filter: f64 }
#[derive(Clone, Debug)]
pub struct Pickup { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub item_id: f64, pub count: f64 }
#[derive(Clone, Debug)]
pub struct Count { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: i64, pub item_id: f64, pub target_count: f64, pub activate: bool, pub multi_activate: bool }
#[derive(Clone, Debug)]
pub struct InstantCount { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: i64, pub item_id: f64, pub target_count: f64, pub activate: bool, pub comparison: f64 }
#[derive(Clone, Debug)]
pub struct CollisionTrig { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub block_a: f64, pub block_b: f64, pub target: i64, pub activate: bool, pub on_exit: bool }
#[derive(Clone, Debug)]
pub struct Block { pub block_id: f64, pub dynamic: bool, pub x: f64, pub y: f64, pub half: f64, pub groups: Vec<i64> }

#[derive(Clone, Debug)]
pub struct ColorTrig { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub index: i64, pub color: crate::colors::Rgb, pub duration: f64, pub tint_ground: bool,
    pub blending: Option<bool>, pub opacity: Option<f64>, pub copy: Option<crate::colors::Copy> }
#[derive(Clone, Debug)]
pub struct AlphaTrig { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub duration: f64, pub target_group: i64, pub target_opacity: f64 }
#[derive(Clone, Debug)]
pub struct ShakeTrig { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub duration: f64, pub strength: f64, pub interval: f64 }
#[derive(Clone, Debug)]
pub struct PlayerVisTrig { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub hide: bool }
#[derive(Clone, Debug)]
pub struct AnimationTrig { pub b: Base, pub x: f64, pub y: f64, pub touch: bool, pub target: f64, pub animate_id: f64 }
#[derive(Clone, Debug)]
pub struct EnterEffectTrig { pub b: Base, pub x: f64, pub effect: i64 }
#[derive(Clone, Copy, Debug)]
pub struct ShakeState { pub strength: f64, pub end_time: f64, pub interval: f64, pub last_step: f64 }

/// `objectDef.enterEffect` of the enter-effect trigger objects (0 is falsy in the original, so no trigger is made for it).
pub fn enter_effect_of(id: i64) -> i64 {
    match id { 23 => 1, 24 => 2, 25 => 3, 26 => 4, 27 => 5, 28 => 6, 55 => 7, 56 => 8, 57 => 9, 58 => 10, 59 => 11, _ => 0 }
}

#[derive(Clone, Debug)]
pub struct ActivePulse { pub trig: usize, pub tg: f64, pub elapsed: f64, pub total: f64 }
#[derive(Clone, Debug)]
pub struct AlphaTween { pub trig: usize, pub tg: i64, pub elapsed: f64, pub start: f64 }
#[derive(Clone, Debug)]
pub struct PulseTrig { pub b: Base, pub x: f64, pub target_group: f64, pub target_channel: f64, pub target_type: f64, pub color: crate::colors::Rgb,
    pub fade_in: f64, pub hold: f64, pub fade_out: f64, pub hsv: Option<crate::colors::Hsv>, pub copy_source: f64 }

pub(crate) fn raw<'a>(o: &'a LevelObject, k: &str) -> Option<&'a String> { o.raw.get(k) }
/// `parseInt(_raw[k] ?? d, 10)`
pub(crate) fn r_int(o: &LevelObject, k: &str, d: f64) -> f64 { match raw(o, k) { Some(s) => parse_int(s).map_or(f64::NAN, |v| v as f64), None => d } }
/// `parseFloat(_raw[k] ?? d)`
pub(crate) fn r_float(o: &LevelObject, k: &str, d: f64) -> f64 { match raw(o, k) { Some(s) => parse_float(s).unwrap_or(f64::NAN), None => d } }
/// `String(_raw[k] ?? d) === "1"`
pub(crate) fn r_flag(o: &LevelObject, k: &str, d: &str) -> bool { raw(o, k).map(|s| s.as_str()).unwrap_or(d) == "1" }

/// `_getLevelObjectGroupIds`: raw 33, raw 57 and `groups`, each split on "." with `parseInt`, positives only, unique.
pub(crate) fn level_object_group_ids(o: &LevelObject) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    let mut add = |s: &str| for part in s.split('.') {
        if let Some(v) = parse_int(part) { if v > 0 && !out.contains(&v) { out.push(v); } }
    };
    if let Some(s) = raw(o, "33") { add(s); }
    if let Some(s) = raw(o, "57") { add(s); }
    add(&o.groups);
    out
}

/// The groups of a collider/sprite (`levelObj.groups.split(".").map(Number).filter(n => n > 0)`, unique).
pub fn object_groups(groups: &str) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    if groups.is_empty() { return out; }
    for part in groups.split('.') {
        let n = to_number(part);
        if n > 0.0 {
            let v = n as i64;
            if !out.contains(&v) { out.push(v); }
        }
    }
    out
}

/// `_parseRotateTriggerGroups`: the first positive id of raw 51 (target) and raw 71 (center).
fn first_group(raw: Option<&String>) -> i64 {
    let s = raw.map(|s| s.as_str()).unwrap_or("0");
    for part in s.split(|c| c == ',' || c == '.') {
        if let Some(v) = parse_int(part.trim()) { if v > 0 { return v; } }
    }
    0
}

#[derive(Default, Clone, Debug)]
pub struct Lists {
    pub moves: Vec<Move>, pub rotates: Vec<Rotate>, pub spawns: Vec<Spawn>, pub stops: Vec<Stop>, pub toggles: Vec<Toggle>,
    pub follow_ys: Vec<FollowY>, pub follows: Vec<Follow>,
    pub on_deaths: Vec<OnDeath>, pub touches: Vec<TouchTrig>, pub pickups: Vec<Pickup>, pub counts: Vec<Count>, pub instant_counts: Vec<InstantCount>,
    pub collisions: Vec<CollisionTrig>, pub blocks: Vec<Block>, pub generics: Vec<crate::generic::Gen>, pub camera_guides: BTreeMap<i64, (f64, f64)>,
    pub colors: Vec<ColorTrig>, pub pulses: Vec<PulseTrig>, pub alphas: Vec<AlphaTrig>, pub shakes: Vec<ShakeTrig>, pub player_vis: Vec<PlayerVisTrig>, pub animations: Vec<AnimationTrig>, pub enter_effects: Vec<EnterEffectTrig>,
}

/// Builds the trigger lists of a level (objects as passed to `_spawnLevelObjects`), sorted by x like the original.
pub fn build_lists(objects: &[LevelObject]) -> Lists {
    let mut l = Lists::default();
    let mut next_uid = 0i64;
    for o in objects {
        let id = o.id.unwrap_or(0);
        let def = crate::objects::object_from_id(id);
        let uid = next_uid;
        next_uid += 1;
        if !crate::objects::is_trigger_branch(id) { continue; }
        let _ = def;
        let b = Base { uid, spawn_triggered: r_flag(o, "62", "0"), multi: r_flag(o, "87", "0"), channel: r_int(o, "170", 0.0) as i64, ord: r_int(o, "115", 0.0) as i64, groups: level_object_group_ids(o) };
        let (x, y) = (o.x * 2.0, o.y * 2.0);
        if id == 2016 {
            for g in raw(o, "57").map(|s| s.as_str()).unwrap_or("").split('.').filter_map(|v| parse_int(v)).filter(|v| *v > 0) {
                l.camera_guides.entry(g).or_insert((x, y));
            }
            continue;
        }
        let touch = r_flag(o, "11", "0");
        if let Some(g) = crate::generic::parse(o, b.clone()) { l.generics.push(g); }
        let ee = enter_effect_of(id);
        if ee != 0 { l.enter_effects.push(EnterEffectTrig { b: b.clone(), x, effect: ee }); }
        match o.id {
            Some(1520) => l.shakes.push(ShakeTrig { b: b.clone(), x, y, touch, duration: js_max(0.0, r_float(o, "10", 0.0)),
                strength: js_max(0.0, js_min(100.0, r_float(o, "75", 0.0))) * 2.0, interval: js_max(0.0, r_float(o, "84", 0.0)) }),
            Some(1612) | Some(1613) => l.player_vis.push(PlayerVisTrig { b: b.clone(), x, y, touch, hide: o.id == Some(1612) }),
            Some(1585) => l.animations.push(AnimationTrig { b: b.clone(), x, y, touch, target: r_int(o, "51", 0.0), animate_id: r_int(o, "76", 0.0) }),
            _ => {}
        }
        match o.id {
            Some(901) => l.moves.push(Move {
                b, x, y, touch, duration: r_float(o, "10", 0.0), easing_type: r_int(o, "30", 0.0), easing_rate: r_float(o, "85", 2.0),
                target: key(r_int(o, "51", 0.0)), offset_x: r_float(o, "28", 0.0) * 2.0, offset_y: r_float(o, "29", 0.0) * 2.0,
                lock_x: r_flag(o, "58", "0"), lock_y: r_flag(o, "59", "0"), lock_cam_x: r_flag(o, "302", "0"), lock_cam_y: r_flag(o, "303", "0"),
            }),
            Some(1346) => l.rotates.push(Rotate {
                b, x, target: first_group(raw(o, "51")), degrees: r_float(o, "68", 0.0), duration: r_float(o, "10", 0.0),
                easing_type: r_int(o, "30", 0.0), easing_rate: r_float(o, "85", 2.0), lock_rotation: raw(o, "70").map(|s| s.as_str()) == Some("1"),
                times360: r_int(o, "69", 0.0), center: first_group(raw(o, "71")),
            }),
            Some(1268) => l.spawns.push(Spawn {
                b, x, y, touch, target: key(r_int(o, "51", 0.0)), delay: or0(js_max(0.0, r_float(o, "63", 0.0))),
                random_delay: or0(js_max(0.0, r_float(o, "556", 0.0))), remap: parse_remap(raw(o, "442")),
            }),
            Some(1616) => l.stops.push(Stop { b, x, y, touch, target: key(r_int(o, "51", 0.0)) }),
            Some(1814) => l.follow_ys.push(FollowY {
                b, x, y, touch, target: key(r_int(o, "51", 0.0)), speed: js_max(0.0, r_float(o, "90", 1.0)), delay: js_max(0.0, r_float(o, "91", 0.0)),
                offset: r_float(o, "92", 0.0) * 2.0, max_speed: js_max(0.0, r_float(o, "105", 0.0)) * 2.0, duration: js_max(0.0, r_float(o, "10", 0.0)),
            }),
            Some(1347) => l.follows.push(Follow {
                b, x, y, touch, target: key(r_int(o, "51", 0.0)), follow_group: key(r_int(o, "71", 0.0)), x_mod: r_float(o, "72", 1.0), y_mod: r_float(o, "73", 1.0),
                duration: js_max(0.0, r_float(o, "10", 0.0)),
            }),
            Some(29) | Some(30) => {
                let r = |k: &str| r_int(o, k, 255.0);
                l.colors.push(ColorTrig { b, x, y, touch, index: if o.id == Some(29) { 1000 } else { 1001 }, color: crate::colors::Rgb { r: r("7"), g: r("8"), b: r("9") },
                    duration: r_float(o, "10", 0.0), tint_ground: raw(o, "14").map(|s| s.as_str()) == Some("1"), blending: None, opacity: None, copy: None });
            }
            Some(lid) if matches!(lid, 105 | 221 | 717 | 718 | 743 | 744 | 899 | 900 | 915) => {
                let default = match lid { 105 => 1004, 221 => 1, 717 => 2, 718 => 3, 743 => 4, 744 => 1003, 899 => 1, 900 => 1009, _ => 1002 } as f64;
                let parsed = r_int(o, "23", default);
                let channel = if parsed == 0.0 || parsed.is_nan() { default } else { parsed };
                let copy_source = r_int(o, "50", 0.0);
                if channel > 0.0 {
                    let r = |k: &str| r_int(o, k, 255.0);
                    l.colors.push(ColorTrig { b, x, y, touch, index: channel as i64, color: crate::colors::Rgb { r: r("7"), g: r("8"), b: r("9") },
                        duration: r_float(o, "10", 0.0), tint_ground: raw(o, "14").map(|s| s.as_str()) == Some("1"), blending: Some(raw(o, "17").map(|s| s.as_str()).unwrap_or("0") == "1"),
                        opacity: Some(r_float(o, "35", 1.0)), copy: if copy_source > 0.0 { Some(crate::colors::Copy { source: copy_source, hsv: crate::colors::parse_channel_hsv(raw(o, "49").map(|s| s.as_str())) }) } else { None } });
                }
            }
            Some(1007) => l.alphas.push(AlphaTrig { b, x, y, touch: false /* the original never sets touchTriggered on alpha triggers */, duration: r_float(o, "10", 0.0), target_group: key(r_int(o, "51", 0.0)),
                target_opacity: js_max(0.0, js_min(1.0, r_float(o, "35", 1.0))) }),
            Some(1006) => {
                let ty = r_int(o, "52", 0.0);
                let tgt = r_int(o, "51", 0.0);
                let r = |k: &str| r_int(o, k, 255.0);
                l.pulses.push(PulseTrig { b, x, target_group: if ty == 1.0 { tgt } else { 0.0 }, target_channel: if ty == 0.0 { tgt } else { 0.0 }, target_type: ty,
                    color: crate::colors::Rgb { r: r("7"), g: r("8"), b: r("9") }, fade_in: r_float(o, "45", 0.0), hold: r_float(o, "46", 0.0), fade_out: r_float(o, "47", 0.0),
                    hsv: if raw(o, "48").map(|s| s.as_str()).unwrap_or("0") == "1" { crate::colors::parse_channel_hsv(raw(o, "49").map(|s| s.as_str())) } else { None },
                    copy_source: { let c = r_int(o, "50", 0.0); if c == 0.0 || c.is_nan() { 0.0 } else { c } } });
            }
            Some(1812) => l.on_deaths.push(OnDeath { b, x, y, touch, target: key(r_int(o, "51", 0.0)), activate: r_flag(o, "56", "1") }),
            Some(1595) => l.touches.push(TouchTrig {
                b, x, y, touch, target: key(r_int(o, "51", 0.0)), toggle_type: or0(r_int(o, "82", 0.0)), dual_touch: r_flag(o, "89", "0"), hold_mode: r_flag(o, "81", "0"),
                player_filter: or0(r_int(o, "198", 0.0)),
            }),
            Some(1817) if raw(o, "80").is_some() => l.pickups.push(Pickup { b, x, y, touch, item_id: r_int(o, "80", 0.0), count: r_int(o, "77", 1.0) }),
            Some(1611) => l.counts.push(Count {
                b, x, y, touch, target: key(r_int(o, "51", 0.0)), item_id: r_int(o, "80", 0.0), target_count: r_int(o, "77", 0.0),
                activate: r_flag(o, "56", "1"), multi_activate: r_flag(o, "104", "0"),
            }),
            Some(1811) => l.instant_counts.push(InstantCount {
                b, x, y, touch, target: key(r_int(o, "51", 0.0)), item_id: r_int(o, "80", 0.0), target_count: r_int(o, "77", 0.0),
                activate: r_flag(o, "56", "1"), comparison: r_int(o, "88", 0.0),
            }),
            Some(1815) => l.collisions.push(CollisionTrig {
                b, x, y, touch, block_a: or0(r_int(o, "80", 0.0)), block_b: or0(r_int(o, "95", 0.0)), target: key(r_int(o, "51", 0.0)),
                activate: r_flag(o, "56", "0"), on_exit: r_flag(o, "93", "0"),
            }),
            Some(1816) => l.blocks.push(Block {
                block_id: or0(r_int(o, "80", 0.0)), dynamic: r_flag(o, "94", "0"), x, y, half: 30.0 * { let sc = or0(o.scale); if sc == 0.0 { 1.0 } else { sc } }, groups: b.groups.clone(),
            }),
            Some(1049) => l.toggles.push(Toggle { b, x, y, touch, target: key(r_int(o, "51", 0.0)), activate: r_flag(o, "56", "0") }),
            _ => {}
        }
    }
    fn by_x<T>(v: &mut Vec<T>, f: impl Fn(&T) -> f64) {
        v.sort_by(|a, b| (f(a) - f(b)).partial_cmp(&0.0).unwrap_or(std::cmp::Ordering::Equal));
    }
    by_x(&mut l.moves, |t| t.x); by_x(&mut l.rotates, |t| t.x);  // spawn triggers keep the level's order: the original never sorts them
    by_x(&mut l.stops, |t| t.x); by_x(&mut l.toggles, |t| t.x);
    by_x(&mut l.follow_ys, |t| t.x); by_x(&mut l.follows, |t| t.x);
    by_x(&mut l.on_deaths, |t| t.x); by_x(&mut l.touches, |t| t.x); by_x(&mut l.pickups, |t| t.x); by_x(&mut l.counts, |t| t.x);
    by_x(&mut l.instant_counts, |t| t.x); by_x(&mut l.collisions, |t| t.x);
    by_x(&mut l.colors, |t| t.x); by_x(&mut l.pulses, |t| t.x); by_x(&mut l.alphas, |t| t.x); by_x(&mut l.shakes, |t| t.x); by_x(&mut l.player_vis, |t| t.x); by_x(&mut l.animations, |t| t.x); by_x(&mut l.enter_effects, |t| t.x);
    l
}

// ---- runtime state ----
#[derive(Clone, Debug)]
pub struct ColMeta {
    pub groups: Vec<i64>,
    pub init_x: f64,
    pub init_y: f64,
    pub init_rot: f64,
    pub toggled: BTreeSet<i64>,
    /// `_eeCollisionSectionIndex` (NaN when never set / not an integer)
    pub sec: f64,
    pub init_wh: Option<(f64, f64)>,
}

/// A sprite as far as group transforms are concerned: its start position (screen Y) and groups.
#[derive(Clone, Debug)]
pub struct Anchor { pub x: f64, pub y: f64, pub groups: Vec<i64>, pub toggled: BTreeSet<i64> }

#[derive(Clone, Debug)]
pub struct MoveTween { trig: usize, tg: i64, elapsed: f64, prev_progress: f64, prev_px: f64, prev_py: f64, prev_cx: f64, prev_cy: f64 }
#[derive(Clone, Debug)]
pub struct RotTween { trig: usize, tg: i64, center: i64, elapsed: f64, prev_progress: f64, total_rad: f64 }
#[derive(Clone, Debug)]
pub struct FollowYTween { trig: usize, tg: i64, elapsed: f64, history: Vec<(f64, f64)>, initial_gdy: f64 }
#[derive(Clone, Debug)]
pub struct FollowTween { trig: usize, tg: i64, fg: i64, elapsed: f64, last_x: f64, last_y: f64 }
#[derive(Clone, Debug)]
pub struct TouchEntry { pub ti: usize, pub ov: Option<i64>, pub holding: bool }
#[derive(Clone, Debug)]
pub struct CountEntry { pub ti: usize, pub ov: Option<i64>, pub prev: f64 }
#[derive(Clone, Debug)]
pub struct CollEntry { pub ti: usize, pub ov: Option<i64>, pub colliding: bool }
#[derive(Clone, Debug)]
pub struct SpawnDelay { pub target: i64, pub delay: f64, pub elapsed: f64, pub remap: RemapRef }
pub type Remap = std::sync::Arc<BTreeMap<i64, i64>>;
#[derive(Clone, Debug)]
pub enum RemapRef { Undef, Null, Map(Remap) }
impl PartialEq for RemapRef {
    fn eq(&self, o: &RemapRef) -> bool {
        match (self, o) { (RemapRef::Undef, RemapRef::Undef) | (RemapRef::Null, RemapRef::Null) => true, (RemapRef::Map(a), RemapRef::Map(b)) => std::sync::Arc::ptr_eq(a, b), _ => false }
    }
}
pub fn parse_remap(value: Option<&String>) -> Option<Remap> {
    let v = value?;
    if v.is_empty() { return None; }
    let parts: Vec<Option<i64>> = v.split('.').map(parse_int).collect();
    let mut map = BTreeMap::new();
    let mut i = 0;
    while i + 1 < parts.len() {
        if let (Some(a), Some(b)) = (parts[i], parts[i + 1]) { map.insert(a, b); }
        i += 2;
    }
    if map.is_empty() { None } else { Some(std::sync::Arc::new(map)) }
}
#[derive(Clone, Debug)]
pub struct RotData { pub total_rad: f64, pub center: i64, pub lock_rotation: bool }

/// Button state read by touch triggers (`upKeyDown` / `upKeyPressed` of each player; the second player's only counts in dual mode).
#[derive(Clone, Copy, Debug, Default)]
pub struct Input { pub p1_hold: bool, pub p1_press: bool, pub p2_hold: bool, pub p2_press: bool }

/// What `_getMoveTriggerPlayerPosition` reads from the scene.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pos { pub player_x: f64, pub player_y: f64, pub camera_x: f64, pub camera_y: f64 }

#[derive(Default, Clone, Debug)]
pub struct Triggers {
    /// `level._timeWarp` and the progress of the time warp triggers
    pub time_warp: f64, pub persistent_items: BTreeSet<i64>,
    pub center_cache: Option<BTreeMap<i64, (f64, f64)>>,
    pub phase_limit: u32,
    pub l: Lists,
    pub gen: crate::generic::GenState,
    pub move_idx: usize, pub rotate_idx: usize, pub spawn_idx: usize, pub stop_idx: usize, pub toggle_idx: usize, pub follow_y_idx: usize, pub follow_idx: usize,
    pub color_idx: usize, pub pulse_idx: usize, pub active_pulses: Vec<ActivePulse>, pub group_ops: Vec<(i64, Option<(usize, f64)>)>, pub shake_idx: usize, pub touch_shake: BTreeSet<i64>, pub shake_state: Option<ShakeState>, pub shake_off: (f64, f64), pub now_ms: f64,
    pub vis_idx: usize, pub touch_vis: BTreeSet<i64>, pub player_hidden: bool,
    pub anim_idx: usize, pub touch_anim: BTreeSet<i64>, pub anim_events: Vec<(i64, f64)>,
    pub enter_idx: usize, pub active_enter: i64, pub active_exit: i64,
    pub alpha_idx: usize, pub active_alpha: Vec<AlphaTween>, pub touch_alpha: BTreeSet<i64>, pub group_opacity: BTreeMap<i64, f64>, pub touch_color: BTreeSet<i64>, pub on_death_idx: usize, pub touch_idx: usize, pub pickup_idx: usize, pub count_idx: usize, pub instant_idx: usize, pub collision_idx: usize,
    pub armed_on_death: Vec<(usize, Option<i64>)>, pub active_touch: Vec<TouchEntry>, pub active_count: Vec<CountEntry>, pub active_collision: Vec<CollEntry>,
    pub touch_on_death: BTreeSet<i64>, pub touch_touch: BTreeSet<i64>, pub touch_pickup: BTreeSet<i64>, pub touch_count: BTreeSet<i64>, pub touch_instant: BTreeSet<i64>,
    pub item_counts: BTreeMap<i64, f64>,
    pub active_follow_y: Vec<FollowYTween>, pub active_follow: Vec<FollowTween>, pub touch_follow_y: BTreeSet<i64>, pub touch_follow: BTreeSet<i64>,
    pub active_move: Vec<MoveTween>, pub active_rotate: Vec<RotTween>, pub active_spawn: Vec<SpawnDelay>, pub spawn_fired: BTreeSet<i64>,
    pub touch_move: BTreeSet<i64>, pub touch_spawn: BTreeSet<i64>, pub touch_stop: BTreeSet<i64>, pub touch_toggle: BTreeSet<i64>,
    pub offsets: BTreeMap<i64, (f64, f64)>,
    pub rotations: BTreeMap<i64, RotData>,
    pub toggled_off: BTreeSet<i64>,
    pub meta: Vec<ColMeta>,
    pub group_cols: BTreeMap<i64, Vec<usize>>,
    pub anchors: Vec<Anchor>,
    pub group_anchors: BTreeMap<i64, Vec<usize>>,
    pub pos: Pos,
    pub defer: bool,
    pub pending: Vec<i64>,
}

pub(crate) fn rm_v(remap: Option<&Remap>, v: i64) -> i64 { remap.and_then(|m| m.get(&v)).copied().unwrap_or(v) }
pub(crate) fn rm_opt(remap: Option<&Remap>, v: i64) -> Option<i64> { remap.and_then(|m| m.get(&v)).copied() }
pub(crate) fn push_unique(v: &mut Vec<i64>, g: i64) { if !v.contains(&g) { v.push(g); } }

impl Triggers {
    pub fn set_anchors(&mut self, anchors: Vec<Anchor>) {
        self.group_anchors.clear();
        for (i, a) in anchors.iter().enumerate() {
            for &g in &a.groups { let list = self.group_anchors.entry(g).or_default(); if !list.contains(&i) { list.push(i); } }
        }
        self.anchors = anchors;
    }

    fn trigger_toggled_off(&self, groups: &[i64]) -> bool { groups.iter().any(|g| self.toggled_off.contains(g)) }
    pub(crate) fn dead(&self, b: &Base) -> bool { self.trigger_toggled_off(&b.groups) }
    pub(crate) fn release_touch(&mut self, uid: i64) {
        for set in [&mut self.touch_color, &mut self.touch_move, &mut self.touch_spawn, &mut self.touch_stop, &mut self.touch_toggle, &mut self.touch_alpha, &mut self.touch_follow_y,
            &mut self.touch_follow, &mut self.touch_anim, &mut self.touch_shake, &mut self.touch_vis, &mut self.touch_on_death, &mut self.touch_touch, &mut self.touch_pickup,
            &mut self.touch_count, &mut self.touch_instant] { set.remove(&uid); }
    }
}

pub fn section_index(x: f64) -> f64 { js_max(0.0, (x / 400.0).floor()) }
fn is_integer(v: f64) -> bool { v.is_finite() && v.fract() == 0.0 }

impl Game {
    // ---- collision sections ----
    /// `_refreshCollisionSection`
    pub fn refresh_collision_section(&mut self, ci: usize) {
        let new = section_index(self.cols[ci].x);
        let cur = self.trig.meta[ci].sec;
        let mut old = if is_integer(cur) { cur } else { -1.0 };
        if old < 0.0 {
            for (idx, list) in self.sections.iter() {
                if list.contains(&ci) { old = *idx as f64; break; }
            }
        }
        if old == new { self.trig.meta[ci].sec = new; return; }
        if old >= 0.0 && old < 4294967295.0 {
            if let Some(list) = self.sections.get_mut(&(old as u64)) {
                if let Some(p) = list.iter().position(|&c| c == ci) { list.remove(p); }
            }
        }
        if new.is_finite() && new < 4294967295.0 {
            let idx = new as u64;
            let list = self.sections.entry(idx).or_default();
            if !list.contains(&ci) { list.push(ci); }
            if idx + 1 > self.sections_len { self.sections_len = idx + 1; }
        }
        self.trig.meta[ci].sec = new;
    }

    // ---- group transforms ----
    fn group_offset_sum(&self, groups: &[i64]) -> (f64, f64) {
        let (mut x, mut y) = (0.0, 0.0);
        for g in groups { if let Some(off) = self.trig.offsets.get(g) { x += or0(off.0); y += or0(off.1); } }
        (x, y)
    }

    pub fn group_offset_sum_pub(&self, groups: &[i64]) -> (f64, f64) { self.group_offset_sum(groups) }
    /// `_getGroupCenter`
    pub fn group_center_pub(&self, gid: i64) -> (f64, f64) { self.group_center(gid) }
    /// `_getGroupCenter` with the per-`_applyGroupTransforms` cache
    pub fn center_cached(&mut self, gid: i64) -> (f64, f64) {
        if let Some(c) = self.trig.center_cache.as_ref().and_then(|m| m.get(&gid)) { return *c; }
        let c = self.group_center(gid);
        if let Some(m) = self.trig.center_cache.as_mut() { m.insert(gid, c); }
        c
    }
    fn group_center(&self, gid: i64) -> (f64, f64) {
        let (mut cx, mut cy, mut n) = (0.0, 0.0, 0.0);
        if let Some(list) = self.trig.group_anchors.get(&gid) {
            for &ai in list {
                let a = &self.trig.anchors[ai];
                // with real sprites the position is the sprite's (emitters have no initial position, so theirs follows them around)
                let (ax, ay, active) = match self.fx.sprites.get(ai) {
                    Some(s) => (s.num("_eeInitialWorldX").unwrap_or_else(|| s.num("_eeWorldX").unwrap_or(s.x)), s.num("_eeInitialBaseY").unwrap_or_else(|| s.num("_eeBaseY").unwrap_or(s.y)), s.active),
                    None => (a.x, a.y, a.toggled.is_empty()),
                };
                if !active { continue; }
                let off = self.group_offset_sum(&a.groups);
                cx += ax + off.0;
                cy += ay + off.1;
                n += 1.0;
            }
        }
        if n > 0.0 { (cx / n, cy / n) } else { (0.0, 0.0) }
    }

    /// `_applyGroupedColliderTransform`
    fn apply_collider_transform(&mut self, ci: usize) {
        let (ix, iy, irot) = { let mt = &self.trig.meta[ci]; (mt.init_x, mt.init_y, mt.init_rot) };
        let ids = self.trig.meta[ci].groups.clone();
        let (move_x, move_y) = self.group_offset_sum(&ids);
        let mut fx = ix + move_x;
        let mut fy = iy - move_y;
        let mut has_rot = false;
        let mut frot = irot;
        for gid in &ids {
            let (total, center, lock) = match self.trig.rotations.get(gid) { Some(r) if r.total_rad != 0.0 => (r.total_rad, r.center, r.lock_rotation), _ => continue };
            has_rot = true;
            if !lock { frot += total * 180.0 / PI; }
            if center > 0 {
                let (cx, cy) = self.center_cached(center);
                let ccy = 460.0 - cy;
                let (dx, dy) = (fx - cx, fy - ccy);
                let crot = -total;
                let (cos, sin) = (m::cos(crot), m::sin(crot));
                fx = cx + dx * cos - dy * sin;
                fy = ccy + dx * sin + dy * cos;
            }
        }
        if self.trig.gen.has_group_scales { self.apply_group_scale_to_collider(ci, &ids, &mut fx, &mut fy); }
        let c = &mut self.cols[ci];
        c.x = fx;
        c.y = fy;
        if has_rot { c.rotation_degrees = frot; }
        if self.trig.meta[ci].sec != section_index(fx) { self.refresh_collision_section(ci); }
    }

    /// `_applyGroupTransforms(groupIds)`: colliders of the groups, in group order, each once.
    fn apply_group_transforms(&mut self, groups: &[i64]) {
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let mut order: Vec<usize> = Vec::new();
        for g in groups {
            if let Some(list) = self.trig.group_cols.get(g) { for &ci in list { if seen.insert(ci) { order.push(ci); } } }
        }
        self.trig.center_cache = Some(BTreeMap::new());
        self.apply_sprite_group_transforms(groups);
        for ci in order { self.apply_collider_transform(ci); }
        self.trig.center_cache = None;
    }

    pub fn begin_group_transform_batch(&mut self) { self.trig.defer = true; self.trig.pending.clear(); }
    pub fn flush_group_transform_batch(&mut self) {
        self.trig.defer = false;
        let pending = std::mem::take(&mut self.trig.pending);
        if !pending.is_empty() { self.apply_group_transforms(&pending); }
    }

    // ---- move ----
    fn start_move(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.moves[ti].b) { return; }
        let p = self.trig.pos;
        let tg = rm_v(remap, self.trig.l.moves[ti].target);
        self.trig.active_move.push(MoveTween { trig: ti, tg, elapsed: 0.0, prev_progress: 0.0, prev_px: p.player_x, prev_py: p.player_y, prev_cx: p.camera_x, prev_cy: p.camera_y });
        self.trig.offsets.entry(tg).or_insert((0.0, 0.0));
    }

    pub fn check_move(&mut self, px: f64) {
        while self.trig.move_idx < self.trig.l.moves.len() {
            let ti = self.trig.move_idx;
            let t = &self.trig.l.moves[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.start_move(ti, None); }
            self.trig.move_idx += 1;
        }
    }

    pub fn check_touch_move(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.moves.len() {
            let t = &self.trig.l.moves[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) { continue; }
            if self.trig.touch_move.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_move.insert(uid);
                self.start_move(ti, None);
            }
        }
    }

    pub fn step_move(&mut self, dt: f64) {
        let mut local: Vec<i64> = Vec::new();
        let mut i = 0;
        while i < self.trig.active_move.len() {
            let anim = self.trig.active_move[i].clone();
            let t = self.trig.l.moves[anim.trig].clone();
            let dur = if t.duration > 0.0 { t.duration } else { 0.0 };
            let elapsed = anim.elapsed + dt;
            let progress = if dur > 0.0 { js_min(elapsed / dur, 1.0) } else { 1.0 };
            let cur = easing(t.easing_type, t.easing_rate, progress);
            let prev = easing(t.easing_type, t.easing_rate, anim.prev_progress);
            let amount = cur - prev;
            let follow = t.lock_x || t.lock_y || t.lock_cam_x || t.lock_cam_y;
            let pos = self.trig.pos;
            let dx = if t.lock_x { pos.player_x - anim.prev_px } else if t.lock_cam_x { pos.camera_x - anim.prev_cx } else { t.offset_x * amount };
            let dy = if t.lock_y { -(pos.player_y - anim.prev_py) } else if t.lock_cam_y { -(pos.camera_y - anim.prev_cy) } else { -(t.offset_y * amount) };
            {
                let a = &mut self.trig.active_move[i];
                a.elapsed = elapsed;
                a.prev_progress = progress;
                if follow { a.prev_px = pos.player_x; a.prev_py = pos.player_y; a.prev_cx = pos.camera_x; a.prev_cy = pos.camera_y; }
            }
            let off = self.trig.offsets.entry(anim.tg).or_insert((0.0, 0.0));
            off.0 += dx;
            off.1 += dy;
            if self.trig.defer { push_unique(&mut self.trig.pending, anim.tg); } else { push_unique(&mut local, anim.tg); }
            if progress >= 1.0 { self.trig.active_move.remove(i); } else { i += 1; }
        }
        if !self.trig.defer { self.apply_group_transforms(&local); }
    }

    pub fn reset_move(&mut self) {
        self.trig.move_idx = 0;
        self.trig.active_move.clear();
        self.trig.touch_move.clear();
        self.trig.offsets.clear();
        self.trig.rotations.clear();
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let order: Vec<usize> = self.trig.group_cols.values().flatten().copied().filter(|c| seen.insert(*c)).collect();
        for ci in order {
            let (bx, by) = { let mt = &self.trig.meta[ci]; (mt.init_x, mt.init_y) };
            self.cols[ci].x = bx;
            self.cols[ci].y = by;
            self.refresh_collision_section(ci);
        }
        self.sprite_reset_move();
    }

    // ---- rotate ----
    fn start_rotate(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.rotates[ti].b) { return; }
        let t = &self.trig.l.rotates[ti];
        let total_deg = t.degrees + (t.times360 * 360.0);
        let (tg, center) = (rm_v(remap, t.target), rm_v(remap, t.center));
        self.trig.active_rotate.push(RotTween { trig: ti, tg, center, elapsed: 0.0, prev_progress: 0.0, total_rad: total_deg * PI / 180.0 });
    }

    pub fn check_rotate(&mut self, px: f64) {
        while self.trig.rotate_idx < self.trig.l.rotates.len() {
            let ti = self.trig.rotate_idx;
            let t = &self.trig.l.rotates[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered { self.start_rotate(ti, None); }
            self.trig.rotate_idx += 1;
        }
    }

    pub fn step_rotate(&mut self, dt: f64) {
        let mut local: Vec<i64> = Vec::new();
        let mut i = 0;
        while i < self.trig.active_rotate.len() {
            let anim = self.trig.active_rotate[i].clone();
            let t = self.trig.l.rotates[anim.trig].clone();
            let dur = if t.duration > 0.0 { t.duration } else { 0.0 };
            let elapsed = anim.elapsed + dt;
            let progress = if dur > 0.0 { js_min(elapsed / dur, 1.0) } else { 1.0 };
            let cur = easing(t.easing_type, t.easing_rate, progress);
            let prev = easing(t.easing_type, t.easing_rate, anim.prev_progress);
            let delta = (cur - prev) * anim.total_rad;
            {
                let a = &mut self.trig.active_rotate[i];
                a.elapsed = elapsed;
                a.prev_progress = progress;
            }
            let rs = self.trig.rotations.entry(anim.tg).or_insert(RotData { total_rad: 0.0, center: anim.center, lock_rotation: t.lock_rotation });
            rs.total_rad += delta;
            rs.center = anim.center;
            rs.lock_rotation = t.lock_rotation;
            if self.trig.defer { push_unique(&mut self.trig.pending, anim.tg); } else { push_unique(&mut local, anim.tg); }
            if progress >= 1.0 { self.trig.active_rotate.remove(i); } else { i += 1; }
        }
        if !self.trig.defer { self.apply_group_transforms(&local); }
    }

    pub fn reset_rotate(&mut self) {
        self.trig.rotate_idx = 0;
        self.trig.active_rotate.clear();
        self.trig.rotations.clear();
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let order: Vec<usize> = self.trig.group_cols.values().flatten().copied().filter(|c| seen.insert(*c)).collect();
        for ci in order {
            let (bx, by, br) = { let mt = &self.trig.meta[ci]; (mt.init_x, mt.init_y, mt.init_rot) };
            self.cols[ci].x = bx;
            self.cols[ci].y = by;
            self.cols[ci].rotation_degrees = br;
            self.refresh_collision_section(ci);
        }
        self.sprite_reset_rotate();
    }

    // ---- stop ----
    fn execute_stop(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.stops[ti].b) { return; }
        let tg = rm_v(remap, self.trig.l.stops[ti].target);
        if tg <= 0 { return; }
        self.trig.active_move.retain(|a| a.tg != tg);
        self.trig.active_rotate.retain(|a| a.tg != tg);
        self.trig.active_follow_y.retain(|a| a.tg != tg);
        self.trig.active_follow.retain(|a| a.tg != tg);
    }

    pub fn check_stop(&mut self, px: f64) {
        while self.trig.stop_idx < self.trig.l.stops.len() {
            let ti = self.trig.stop_idx;
            let t = &self.trig.l.stops[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.execute_stop(ti, None); }
            self.trig.stop_idx += 1;
        }
    }

    pub fn check_touch_stop(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.stops.len() {
            let t = &self.trig.l.stops[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) { continue; }
            if self.trig.touch_stop.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_stop.insert(uid);
                self.execute_stop(ti, None);
            }
        }
    }

    pub fn reset_stop(&mut self) { self.trig.stop_idx = 0; self.trig.touch_stop.clear(); }

    // ---- follow player Y ----
    fn start_follow_y(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.follow_ys[ti].b) { return; }
        let tg = rm_v(remap, self.trig.l.follow_ys[ti].target);
        if tg <= 0 { return; }
        self.trig.offsets.entry(tg).or_insert((0.0, 0.0));
        let (mut sum, mut n) = (0.0, 0.0);
        if let Some(list) = self.trig.group_anchors.get(&tg) {
            for &ai in list {
                let a = &self.trig.anchors[ai];
                let (ay, active) = match self.fx.sprites.get(ai) {
                    Some(s) => (s.num("_eeInitialBaseY").or_else(|| s.num("_origBaseY")).unwrap_or(s.y), s.active),
                    None => (a.y, a.toggled.is_empty()),
                };
                if !active { continue; }
                sum += 460.0 - ay;
                n += 1.0;
            }
        }
        if n == 0.0 {
            if let Some(list) = self.trig.group_cols.get(&tg) {
                for &ci in list { sum += or0(self.trig.meta[ci].init_y); n += 1.0; }
            }
        }
        let initial_gdy = if n > 0.0 { sum / n } else { 0.0 };
        self.trig.active_follow_y.push(FollowYTween { trig: ti, tg, elapsed: 0.0, history: Vec::new(), initial_gdy });
    }

    pub fn check_follow_y(&mut self, px: f64) {
        while self.trig.follow_y_idx < self.trig.l.follow_ys.len() {
            let ti = self.trig.follow_y_idx;
            let t = &self.trig.l.follow_ys[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.start_follow_y(ti, None); }
            self.trig.follow_y_idx += 1;
        }
    }

    pub fn check_touch_follow_y(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.follow_ys.len() {
            let t = &self.trig.l.follow_ys[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) { continue; }
            if self.trig.touch_follow_y.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_follow_y.insert(uid);
                self.start_follow_y(ti, None);
            }
        }
    }

    pub fn step_follow_y(&mut self, dt: f64) {
        if self.trig.active_follow_y.is_empty() { return; }
        let current_py = self.trig.pos.player_y;
        let mut local: Vec<i64> = Vec::new();
        let mut i = 0;
        while i < self.trig.active_follow_y.len() {
            let t = self.trig.l.follow_ys[self.trig.active_follow_y[i].trig].clone();
            let tgt = self.trig.active_follow_y[i].tg;
            let (elapsed, target_player_y, initial_gdy) = {
                let a = &mut self.trig.active_follow_y[i];
                a.elapsed += dt;
                a.history.push((a.elapsed, current_py));
                let target_time = a.elapsed - t.delay;
                while a.history.len() > 2 && a.history[1].0 <= target_time { a.history.remove(0); }
                let mut ty = a.history.first().map(|h| h.1).unwrap_or(current_py);
                if a.history.len() >= 2 && a.history[0].0 <= target_time && target_time <= a.history[1].0 {
                    let (t0, t1) = (a.history[0].0, a.history[1].0);
                    let span = t1 - t0;
                    let alpha = if span > 0.0 { (target_time - t0) / span } else { 0.0 };
                    ty = a.history[0].1 + (a.history[1].1 - a.history[0].1) * alpha;
                }
                (a.elapsed, ty, a.initial_gdy)
            };
            let off = self.trig.offsets.entry(tgt).or_insert((0.0, 0.0));
            let current_gdy = initial_gdy - off.1;
            let target_gdy = target_player_y + t.offset;
            let diff = target_gdy - current_gdy;
            let rate = js_min(1.0, js_max(0.0, dt * 60.0 * t.speed));
            let mut step = diff * rate;
            if t.max_speed > 0.0 {
                let max_step = t.max_speed * dt * 60.0;
                if step.abs() > max_step { step = js_sign(step) * max_step; }
            }
            off.1 -= step;
            if self.trig.defer { push_unique(&mut self.trig.pending, tgt); } else { push_unique(&mut local, tgt); }
            if t.duration > 0.0 && elapsed >= t.duration { self.trig.active_follow_y.remove(i); } else { i += 1; }
        }
        if !self.trig.defer { self.apply_group_transforms(&local); }
    }

    pub fn reset_follow_y(&mut self) { self.trig.follow_y_idx = 0; self.trig.active_follow_y.clear(); self.trig.touch_follow_y.clear(); }

    // ---- follow ----
    fn start_follow(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.follows[ti].b) { return; }
        let (tg, fg) = { let t = &self.trig.l.follows[ti]; (rm_v(remap, t.target), rm_v(remap, t.follow_group)) };
        if tg <= 0 || fg <= 0 { return; }
        let base = self.trig.offsets.get(&fg).copied().unwrap_or((0.0, 0.0));
        self.trig.active_follow.push(FollowTween { trig: ti, tg, fg, elapsed: 0.0, last_x: base.0, last_y: base.1 });
        self.trig.offsets.entry(tg).or_insert((0.0, 0.0));
    }

    pub fn check_follow(&mut self, px: f64) {
        while self.trig.follow_idx < self.trig.l.follows.len() {
            let ti = self.trig.follow_idx;
            let t = &self.trig.l.follows[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.start_follow(ti, None); }
            self.trig.follow_idx += 1;
        }
    }

    pub fn check_touch_follow(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.follows.len() {
            let t = &self.trig.l.follows[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) { continue; }
            if self.trig.touch_follow.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_follow.insert(uid);
                self.start_follow(ti, None);
            }
        }
    }

    pub fn step_follow(&mut self, dt: f64) {
        if self.trig.active_follow.is_empty() { return; }
        let mut local: Vec<i64> = Vec::new();
        let mut i = 0;
        while i < self.trig.active_follow.len() {
            let t = self.trig.l.follows[self.trig.active_follow[i].trig].clone();
            let (tgt, fgt) = (self.trig.active_follow[i].tg, self.trig.active_follow[i].fg);
            let cur = self.trig.offsets.get(&fgt).copied().unwrap_or((0.0, 0.0));
            let elapsed = {
                let a = &mut self.trig.active_follow[i];
                a.elapsed += dt;
                let dx = (cur.0 - a.last_x) * t.x_mod;
                let dy = (cur.1 - a.last_y) * t.y_mod;
                a.last_x = cur.0;
                a.last_y = cur.1;
                let e = a.elapsed;
                let off = self.trig.offsets.entry(tgt).or_insert((0.0, 0.0));
                off.0 += dx;
                off.1 += dy;
                e
            };
            if self.trig.defer { push_unique(&mut self.trig.pending, tgt); } else { push_unique(&mut local, tgt); }
            if t.duration > 0.0 && elapsed >= t.duration { self.trig.active_follow.remove(i); } else { i += 1; }
        }
        if !self.trig.defer { self.apply_group_transforms(&local); }
    }

    pub fn reset_follow(&mut self) { self.trig.follow_idx = 0; self.trig.active_follow.clear(); self.trig.touch_follow.clear(); }

    // ---- colour and pulse triggers ----
    /// `setChannelBlending(ch, on)` (the level state; sprites follow it when drawn)
    pub fn set_channel_blending(&mut self, ch: i64, on: bool) {
        if !(ch > 0) || ch >= 1000 { return; }
        if self.channel_blending.get(&ch).copied().unwrap_or(false) == on { return; }
        self.channel_blending.insert(ch, on);
        self.sprite_channel_blending(ch, on);
    }

    /// `_activateSpawnedGroup`'s / `_applyColorTriggerFn`'s colour step. `spawned` = through a spawn trigger (uses `trig.duration` too).
    fn apply_color_trigger(&mut self, ti: usize, spawned: bool) {
        let t = self.trig.l.colors[ti].clone();
        let _ = spawned;
        self.cm.trigger_color(t.index, t.color, t.duration, t.copy, t.opacity);
        self.set_channel_blending(t.index, t.blending.unwrap_or(false));
        if !spawned && t.tint_ground { self.cm.trigger_color(1001, t.color, t.duration, t.copy, None); }
    }

    /// `checkColorTriggers(playerX)` + the scene applying them
    pub fn check_color(&mut self, px: f64) {
        let mut fired: Vec<usize> = Vec::new();
        while self.trig.color_idx < self.trig.l.colors.len() {
            let ti = self.trig.color_idx;
            let t = &self.trig.l.colors[ti];
            if !(t.x <= px) { break; }
            if !self.trig.dead(&t.b) && !t.touch && !t.b.spawn_triggered { fired.push(ti); }
            self.trig.color_idx += 1;
        }
        for ti in fired { self.apply_color_trigger(ti, false); }
    }

    pub fn check_touch_color(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        let mut fired: Vec<usize> = Vec::new();
        for ti in 0..self.trig.l.colors.len() {
            let t = &self.trig.l.colors[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) { continue; }
            if self.trig.touch_color.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX { fired.push(ti); }
        }
        for ti in fired { let uid = self.trig.l.colors[ti].b.uid; self.trig.touch_color.insert(uid); self.apply_color_trigger(ti, false); }
    }

    pub fn reset_color(&mut self) { self.trig.color_idx = 0; self.trig.touch_color.clear(); self.fx.color_dirty = true; }

    fn start_pulse(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.pulses[ti].b) { return; }
        if self.ldm { return; }
        let mut t = self.trig.l.pulses[ti].clone();
        if let Some(m) = remap { if let Some(v) = m.get(&(t.target_group as i64)) { if t.target_group.is_finite() { t.target_group = *v as f64; } } }
        if t.target_type == 0.0 && t.target_channel > 0.0 {
            self.cm.start_pulse(t.target_channel, t.color, t.hsv, t.copy_source, t.fade_in, t.hold, t.fade_out);
            return;
        }
        let total = t.fade_in + t.hold + t.fade_out;
        self.trig.active_pulses.push(ActivePulse { trig: ti, tg: t.target_group, elapsed: 0.0, total: if total > 0.0 { total } else { 0.01 } });
    }

    /// `stepPulseTriggers`: ages the group pulses and leaves the tint operation of every pulsed group in `group_ops`
    /// (`None` = the pulse ended: restore the sprites). Only groups that have sprites (`group_anchors`) get an entry.
    pub fn step_pulse(&mut self, dt: f64) {
        if self.ldm { return; }
        if self.trig.active_pulses.is_empty() { return; }
        self.trig.group_ops.clear();
        let mut i = 0;
        while i < self.trig.active_pulses.len() {
            self.trig.active_pulses[i].elapsed += dt;
            let p = self.trig.active_pulses[i].clone();
            let t = &self.trig.l.pulses[p.trig];
            let (fi, ho, fo) = (t.fade_in, t.hold, t.fade_out);
            let tm = p.elapsed;
            let mut intensity = 0.0;
            if tm < fi { intensity = if fi > 0.0 { tm / fi } else { 1.0 }; }
            else if tm < fi + ho { intensity = 1.0; }
            else if tm < fi + ho + fo { intensity = if fo > 0.0 { 1.0 - (tm - fi - ho) / fo } else { 0.0 }; }
            let finished = p.elapsed >= p.total;
            if t.target_type == 1.0 && p.tg > 0.0 {
                let g = p.tg as i64;
                if self.trig.group_anchors.contains_key(&g) {
                    self.trig.group_ops.retain(|e| e.0 != g);
                    self.trig.group_ops.push((g, if finished || intensity <= 0.01 { None } else { Some((p.trig, js_min(1.0, intensity))) }));
                }
            }
            if finished { self.trig.active_pulses.remove(i); } else { i += 1; }
        }
        self.apply_pulse_ops();
    }

    pub fn check_pulse(&mut self, px: f64) {
        if self.ldm { return; }
        while self.trig.pulse_idx < self.trig.l.pulses.len() {
            let ti = self.trig.pulse_idx;
            let t = &self.trig.l.pulses[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered { self.start_pulse(ti, None); }
            self.trig.pulse_idx += 1;
        }
    }

    // ---- shake, player visibility, animation and enter-effect triggers ----
    fn execute_shake(&mut self, ti: usize) {
        if self.trig.dead(&self.trig.l.shakes[ti].b) { return; }
        let t = &self.trig.l.shakes[ti];
        let dur = js_max(0.0, or0(t.duration));
        let st = js_max(0.0, or0(t.strength));
        if dur <= 0.0 || st <= 0.0 { return; }
        let iv = js_max(0.0, or0(t.interval));
        self.trig.shake_state = Some(ShakeState { strength: st, end_time: self.shake_clock() + dur * 1000.0, interval: iv, last_step: f64::NEG_INFINITY });
    }

    /// The shake runs on the physics ticks (`tickBased`): `_physicsTick * u * 1000`.
    fn shake_clock(&self) -> f64 { self.e.physics_tick * (1.0 / 240.0) * 1000.0 }

    /// `applyShakeToOffsets(nowMs)` (the shake state is always tick based now, so the argument is ignored)
    pub fn apply_shake(&mut self, _now: f64) {
        let now = self.shake_clock();
        let Some(mut s) = self.trig.shake_state else { return };
        if now > s.end_time {
            self.trig.shake_state = None;
            self.trig.shake_off = (0.0, 0.0);
            return;
        }
        if now - s.last_step >= s.interval * 1000.0 {
            let x = (m::random() * 2.0 - 1.0) * s.strength;
            let y = (m::random() * 2.0 - 1.0) * s.strength;
            self.trig.shake_off = (x, y);
            s.last_step = now;
            self.trig.shake_state = Some(s);
        }
    }

    pub fn check_shake(&mut self, px: f64) {
        while self.trig.shake_idx < self.trig.l.shakes.len() {
            let ti = self.trig.shake_idx;
            let t = &self.trig.l.shakes[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.execute_shake(ti); }
            self.trig.shake_idx += 1;
        }
    }

    pub fn check_touch_shake(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.shakes.len() {
            let t = &self.trig.l.shakes[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) || self.trig.touch_shake.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_shake.insert(uid);
                self.execute_shake(ti);
            }
        }
    }

    pub fn save_time_warp(&self) -> (f64, usize, Vec<i64>) { (1.0, 0, Vec::new()) }
    pub fn restore_time_warp(&mut self, _saved: Option<&(f64, usize, Vec<i64>)>, _px: f64) {}

    pub fn reset_shake(&mut self) { self.trig.shake_idx = 0; self.trig.touch_shake.clear(); self.trig.shake_state = None; self.trig.shake_off = (0.0, 0.0); }

    pub fn check_player_vis(&mut self, px: f64) {
        while self.trig.vis_idx < self.trig.l.player_vis.len() {
            let t = &self.trig.l.player_vis[self.trig.vis_idx];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.trig.player_hidden = t.hide; }
            self.trig.vis_idx += 1;
        }
    }

    pub fn check_touch_player_vis(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.player_vis.len() {
            let t = &self.trig.l.player_vis[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) || self.trig.touch_vis.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let (uid, hide) = (t.b.uid, t.hide);
                self.trig.touch_vis.insert(uid);
                self.trig.player_hidden = hide;
            }
        }
    }

    pub fn reset_player_vis(&mut self) { self.trig.vis_idx = 0; self.trig.touch_vis.clear(); self.trig.player_hidden = false; }

    /// `_executeAnimationTrigger`: the group's animated objects get the command (the renderer drains `anim_events`).
    fn execute_animation(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.animations[ti].b) { return; }
        let t = &self.trig.l.animations[ti];
        let tv = if t.target.is_finite() { rm_v(remap, t.target as i64) as f64 } else { t.target };
        if !(tv.is_finite() && tv > 0.0) { return; }
        let g = tv as i64;
        if !self.trig.group_anchors.contains_key(&g) { return; }
        let id = t.animate_id;
        self.trig.anim_events.push((g, id));
        self.beast_command(g, id);
    }

    pub fn check_animation(&mut self, px: f64) {
        while self.trig.anim_idx < self.trig.l.animations.len() {
            let ti = self.trig.anim_idx;
            let t = &self.trig.l.animations[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.execute_animation(ti, None); }
            self.trig.anim_idx += 1;
        }
    }

    pub fn check_touch_animation(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.animations.len() {
            let t = &self.trig.l.animations[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) || self.trig.touch_anim.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_anim.insert(uid);
                self.execute_animation(ti, None);
            }
        }
    }

    pub fn reset_animation(&mut self) { self.trig.anim_idx = 0; self.trig.touch_anim.clear(); self.beasts_default(); }

    pub fn check_enter_effect(&mut self, px: f64) {
        while self.trig.enter_idx < self.trig.l.enter_effects.len() {
            let t = &self.trig.l.enter_effects[self.trig.enter_idx];
            if !(t.x <= px) { break; }
            if !t.b.spawn_triggered { self.trig.active_enter = t.effect; self.trig.active_exit = t.effect; }
            self.trig.enter_idx += 1;
        }
    }

    pub fn reset_enter_effect(&mut self) { self.trig.enter_idx = 0; self.trig.active_enter = 0; self.trig.active_exit = 0; self.sprite_reset_enter_effects(); }

    // ---- alpha (group opacity) triggers ----
    fn start_alpha(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.alphas[ti].b) { return; }
        let g = rm_v(remap, self.trig.l.alphas[ti].target_group);
        let start = self.trig.group_opacity.get(&g).copied().unwrap_or(1.0);
        self.trig.active_alpha.push(AlphaTween { trig: ti, tg: g, elapsed: 0.0, start });
    }

    pub fn check_alpha(&mut self, px: f64) {
        while self.trig.alpha_idx < self.trig.l.alphas.len() {
            let ti = self.trig.alpha_idx;
            let t = &self.trig.l.alphas[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.start_alpha(ti, None); }
            self.trig.alpha_idx += 1;
        }
    }

    pub fn check_touch_alpha(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.alphas.len() {
            let t = &self.trig.l.alphas[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) { continue; }
            if self.trig.touch_alpha.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_alpha.insert(uid);
                self.start_alpha(ti, None);
            }
        }
    }

    pub fn step_alpha(&mut self, dt: f64) {
        let mut changed = false;
        let mut i = 0;
        while i < self.trig.active_alpha.len() {
            self.trig.active_alpha[i].elapsed += dt;
            let a = self.trig.active_alpha[i].clone();
            let t = &self.trig.l.alphas[a.trig];
            let dur = if t.duration > 0.0 { t.duration } else { 0.0 };
            let progress = if dur > 0.0 { js_min(a.elapsed / dur, 1.0) } else { 1.0 };
            let op = a.start + (t.target_opacity - a.start) * progress;
            let clamped = js_max(0.0, js_min(1.0, op));
            let g = a.tg;
            if self.trig.group_opacity.get(&g).copied() != Some(clamped) { changed = true; }
            self.trig.group_opacity.insert(g, clamped);
            if progress >= 1.0 { self.trig.active_alpha.remove(i); } else { i += 1; }
        }
        self.apply_alpha(changed);
    }

    pub fn reset_alpha(&mut self) { self.trig.alpha_idx = 0; self.trig.active_alpha.clear(); self.trig.touch_alpha.clear(); self.trig.group_opacity.clear(); self.sprite_reset_alpha(); }

    pub fn reset_pulse(&mut self) { self.cm.clear_pulses(); self.trig.pulse_idx = 0; self.trig.active_pulses.clear(); self.sprite_reset_pulse(); }

    /// `resetChannelBlending`
    pub fn reset_channel_blending(&mut self) {
        let initial = self.channel_blending_initial.clone();
        let mut keys: BTreeSet<i64> = self.channel_blending.keys().copied().collect();
        keys.extend(initial.keys().copied());
        for k in keys { self.set_channel_blending(k, initial.get(&k).copied().unwrap_or(false)); }
    }

    // ---- toggle by group number (the `{ targetGroup, activate }` calls made by other triggers) ----
    fn toggle_group(&mut self, tg: i64, activate: bool) {
        if tg <= 0 { return; }
        if activate { self.trig.toggled_off.remove(&tg); } else { self.trig.toggled_off.insert(tg); }
        if let Some(list) = self.trig.group_anchors.get(&tg).cloned() {
            for ai in list {
                let a = &mut self.trig.anchors[ai];
                if activate { a.toggled.remove(&tg); } else { a.toggled.insert(tg); }
            }
        }
        if let Some(list) = self.trig.group_cols.get(&tg).cloned() {
            for ci in list {
                let mt = &mut self.trig.meta[ci];
                if activate { mt.toggled.remove(&tg); } else { mt.toggled.insert(tg); }
                self.disabled[ci] = !mt.toggled.is_empty();
            }
        }
        self.sprite_toggle_group(tg);
    }

    pub(crate) fn toggle_group_triggered_pub(&mut self, tg: i64, activate: bool) { self.toggle_group_triggered(tg, activate); }
    pub(crate) fn toggle_group_pub(&mut self, tg: i64, activate: bool) { self.toggle_group(tg, activate); }

    fn toggle_group_triggered(&mut self, tg: i64, activate: bool) {
        self.toggle_group(tg, activate);
        if activate { self.activate_spawned_group(tg); }
    }

    // ---- on death ----
    fn arm_on_death(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.on_deaths[ti].b) { return; }
        let ov = rm_opt(remap, self.trig.l.on_deaths[ti].target);
        if ov.is_some() || !self.trig.armed_on_death.iter().any(|e| e.0 == ti && e.1.is_none()) { self.trig.armed_on_death.push((ti, ov)); }
    }

    fn execute_on_death(&mut self, ti: usize, ov: Option<i64>) {
        if self.trig.dead(&self.trig.l.on_deaths[ti].b) { return; }
        let (tg, activate) = { let t = &self.trig.l.on_deaths[ti]; (ov.unwrap_or(t.target), t.activate) };
        if activate { self.toggle_group_triggered(tg, true); } else { self.toggle_group(tg, false); }
    }

    /// `triggerOnDeath`
    pub fn trigger_on_death(&mut self) {
        let mut i = 0;
        while i < self.trig.armed_on_death.len() { let (ti, ov) = self.trig.armed_on_death[i]; self.execute_on_death(ti, ov); i += 1; }
    }

    pub fn check_on_death(&mut self, px: f64) {
        while self.trig.on_death_idx < self.trig.l.on_deaths.len() {
            let ti = self.trig.on_death_idx;
            let t = &self.trig.l.on_deaths[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.arm_on_death(ti, None); }
            self.trig.on_death_idx += 1;
        }
    }

    pub fn check_touch_on_death(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.on_deaths.len() {
            let t = &self.trig.l.on_deaths[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) || self.trig.touch_on_death.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_on_death.insert(uid);
                self.arm_on_death(ti, None);
            }
        }
    }

    pub fn reset_on_death(&mut self) { self.trig.on_death_idx = 0; self.trig.armed_on_death.clear(); self.trig.touch_on_death.clear(); }

    // ---- touch (player input) triggers ----
    fn arm_touch(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.touches[ti].b) { return; }
        let ov = rm_opt(remap, self.trig.l.touches[ti].target);
        if ov.is_some() || !self.trig.active_touch.iter().any(|e| e.ti == ti && e.ov.is_none()) { self.trig.active_touch.push(TouchEntry { ti, ov, holding: false }); }
    }

    pub fn check_touch_triggers(&mut self, px: f64) {
        while self.trig.touch_idx < self.trig.l.touches.len() {
            let ti = self.trig.touch_idx;
            let t = &self.trig.l.touches[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.arm_touch(ti, None); }
            self.trig.touch_idx += 1;
        }
    }

    pub fn check_touch_touch(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.touches.len() {
            let t = &self.trig.l.touches[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) || self.trig.touch_touch.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_touch.insert(uid);
                self.arm_touch(ti, None);
            }
        }
    }

    pub fn step_touch(&mut self, p1_hold: bool, p1_press: bool, p2_hold: bool, p2_press: bool) {
        let mut i = 0;
        while i < self.trig.active_touch.len() {
            let pos = i;
            let (ti, ov) = { let e = &self.trig.active_touch[pos]; (e.ti, e.ov) };
            i += 1;
            if self.trig.dead(&self.trig.l.touches[ti].b) { continue; }
            let (hold_mode, toggle_type, tg, filter) = { let t = &self.trig.l.touches[ti]; (t.hold_mode, t.toggle_type, ov.unwrap_or(t.target), t.player_filter) };
            let use_p1 = filter != 2.0;
            let use_p2 = filter != 1.0;
            let holding = (use_p1 && p1_hold) || (use_p2 && p2_hold);
            let pressed = (use_p1 && p1_press) || (use_p2 && p2_press);
            let off = self.trig.toggled_off.contains(&tg);
            let mut activate: Option<bool> = None;
            if hold_mode {
                if holding != self.trig.active_touch[pos].holding {
                    self.trig.active_touch[pos].holding = holding;
                    activate = Some(if toggle_type == 0.0 { off } else { (toggle_type == 1.0) == holding });
                }
            } else if pressed {
                activate = Some(if toggle_type == 0.0 { off } else { toggle_type == 1.0 });
            }
            if let Some(a) = activate { self.toggle_group_triggered(tg, a); }
        }
    }

    pub fn touch_trigger_dual_mode(&self) -> bool { self.trig.l.touches.iter().any(|t| t.dual_touch) }

    pub fn reset_touch(&mut self) {
        self.trig.touch_idx = 0;
        self.trig.active_touch.clear();
        self.trig.touch_touch.clear();
    }

    // ---- item counts ----
    fn execute_pickup(&mut self, ti: usize) {
        if self.trig.dead(&self.trig.l.pickups[ti].b) { return; }
        let (item, count) = { let t = &self.trig.l.pickups[ti]; (t.item_id, t.count) };
        if item == 0.0 || item.is_nan() { return; }
        let id = item as i64;
        let cur = or0(self.trig.item_counts.get(&id).copied().unwrap_or(0.0));
        self.trig.item_counts.insert(id, cur + count);
        self.check_count_triggers_for(item);
    }

    pub fn check_pickup(&mut self, px: f64) {
        while self.trig.pickup_idx < self.trig.l.pickups.len() {
            let ti = self.trig.pickup_idx;
            let t = &self.trig.l.pickups[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.execute_pickup(ti); }
            self.trig.pickup_idx += 1;
        }
    }

    pub fn check_touch_pickup(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.pickups.len() {
            let t = &self.trig.l.pickups[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) || self.trig.touch_pickup.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_pickup.insert(uid);
                self.execute_pickup(ti);
            }
        }
    }

    pub fn reset_pickup(&mut self) {
        self.trig.pickup_idx = 0;
        self.trig.touch_pickup.clear();
        let persistent = &self.trig.persistent_items;
        self.trig.item_counts.retain(|k, _| persistent.contains(k));
    }

    fn item_count(&self, item: f64) -> f64 {
        if item.is_nan() { return 0.0; }
        or0(self.trig.item_counts.get(&(item as i64)).copied().unwrap_or(0.0))
    }

    fn arm_count(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.counts[ti].b) { return; }
        let ov = rm_opt(remap, self.trig.l.counts[ti].target);
        if ov.is_some() || !self.trig.active_count.iter().any(|e| e.ti == ti && e.ov.is_none()) {
            let item = self.trig.l.counts[ti].item_id;
            let prev = self.item_count(item);
            self.trig.active_count.push(CountEntry { ti, ov, prev });
        }
    }

    fn evaluate_count(&mut self, pos: usize) {
        let (ti, ov, previous) = { let e = &self.trig.active_count[pos]; (e.ti, e.ov, e.prev) };
        if self.trig.dead(&self.trig.l.counts[ti].b) { return; }
        let (item, target, activate, multi, tg) = { let t = &self.trig.l.counts[ti]; (t.item_id, t.target_count, t.activate, t.multi_activate, ov.unwrap_or(t.target)) };
        let current = self.item_count(item);
        if current == previous { return; }
        self.trig.active_count[pos].prev = current;
        if !((previous < target && current >= target) || (previous > target && current <= target)) { return; }
        if !multi { self.trig.active_count.remove(pos); }
        self.toggle_group_triggered(tg, activate);
    }

    pub(crate) fn check_count_triggers_for(&mut self, item: f64) {
        let mut i = self.trig.active_count.len() as isize - 1;
        while i >= 0 {
            if let Some(e) = self.trig.active_count.get(i as usize) {
                if self.trig.l.counts[e.ti].item_id == item { self.evaluate_count(i as usize); }
            }
            i -= 1;
        }
    }

    pub fn check_count(&mut self, px: f64) {
        while self.trig.count_idx < self.trig.l.counts.len() {
            let ti = self.trig.count_idx;
            let t = &self.trig.l.counts[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.arm_count(ti, None); }
            self.trig.count_idx += 1;
        }
    }

    pub fn check_touch_count(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.counts.len() {
            let t = &self.trig.l.counts[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) || self.trig.touch_count.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_count.insert(uid);
                self.arm_count(ti, None);
            }
        }
    }

    pub fn reset_count(&mut self) { self.trig.count_idx = 0; self.trig.active_count.clear(); self.trig.touch_count.clear(); }

    fn execute_instant_count(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.instant_counts[ti].b) { return; }
        let (item, target_count, activate, cmp, tg) = { let t = &self.trig.l.instant_counts[ti]; (t.item_id, t.target_count, t.activate, t.comparison, rm_v(remap, t.target)) };
        let current = self.item_count(item);
        let matched = if cmp == 1.0 { current > target_count } else if cmp == 2.0 { current < target_count } else { current == target_count };
        if matched {
            if activate { self.toggle_group_triggered(tg, true); } else { self.toggle_group(tg, false); }
        }
    }

    pub fn check_instant_count(&mut self, px: f64) {
        while self.trig.instant_idx < self.trig.l.instant_counts.len() {
            let ti = self.trig.instant_idx;
            let t = &self.trig.l.instant_counts[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.execute_instant_count(ti, None); }
            self.trig.instant_idx += 1;
        }
    }

    pub fn check_touch_instant_count(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.instant_counts.len() {
            let t = &self.trig.l.instant_counts[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) || self.trig.touch_instant.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_instant.insert(uid);
                self.execute_instant_count(ti, None);
            }
        }
    }

    pub fn reset_instant_count(&mut self) { self.trig.instant_idx = 0; self.trig.touch_instant.clear(); }

    // ---- collision triggers ----
    fn arm_collision(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.collisions[ti].b) { return; }
        let t = &self.trig.l.collisions[ti];
        let ov = rm_opt(remap, t.target);
        if t.block_a == 0.0 || t.block_b == 0.0 || !(ov.unwrap_or(t.target) > 0) { return; }
        if ov.is_none() && self.trig.active_collision.iter().any(|e| e.ti == ti && e.ov.is_none()) { return; }
        self.trig.active_collision.push(CollEntry { ti, ov, colliding: false });
    }

    pub fn check_collision_triggers(&mut self, px: f64) {
        while self.trig.collision_idx < self.trig.l.collisions.len() {
            let ti = self.trig.collision_idx;
            let t = &self.trig.l.collisions[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.arm_collision(ti, None); }
            self.trig.collision_idx += 1;
        }
    }

    pub(crate) fn block_enabled_pub(&self, b: &Block) -> bool { self.block_enabled(b) }
    pub(crate) fn blocks_touch_pub(&self, a: f64, b: f64) -> bool { self.blocks_touch(a, b) }
    pub(crate) fn apply_group_transforms_pub(&mut self, g: &[i64]) { self.apply_group_transforms(g); }
    fn block_enabled(&self, b: &Block) -> bool { !b.groups.iter().any(|g| self.trig.toggled_off.contains(g)) }

    fn blocks_touch(&self, id_a: f64, id_b: f64) -> bool {
        let blocks = &self.trig.l.blocks;
        for (i, a) in blocks.iter().enumerate() {
            if a.block_id != id_a || !self.block_enabled(a) { continue; }
            let off_a = self.group_offset_sum(&a.groups);
            let (ax, ay) = (a.x + off_a.0, a.y + off_a.1);
            for (j, b) in blocks.iter().enumerate() {
                if j == i || b.block_id != id_b || (!a.dynamic && !b.dynamic) || !self.block_enabled(b) { continue; }
                let off_b = self.group_offset_sum(&b.groups);
                let reach = a.half + b.half;
                if (ax - (b.x + off_b.0)).abs() < reach && (ay - (b.y + off_b.1)).abs() < reach { return true; }
            }
        }
        false
    }

    pub fn update_collision_triggers(&mut self) {
        let mut i = 0;
        while i < self.trig.active_collision.len() {
            let (ti, ov, colliding) = { let e = &self.trig.active_collision[i]; (e.ti, e.ov, e.colliding) };
            i += 1;
            let t = self.trig.l.collisions[ti].clone();
            if self.trig.trigger_toggled_off(&t.b.groups) { continue; }
            let touching = self.blocks_touch(t.block_a, t.block_b);
            if touching == colliding { continue; }
            self.trig.active_collision[i - 1].colliding = touching;
            if touching != t.on_exit { self.toggle_group_triggered(ov.unwrap_or(t.target), t.activate); }
        }
    }

    pub fn reset_collision(&mut self) { self.trig.collision_idx = 0; self.trig.active_collision.clear(); }

    // ---- toggle ----
    pub fn execute_toggle_pub(&mut self, ti: usize) { self.execute_toggle(ti, None); }

    fn execute_toggle(&mut self, ti: usize, remap: Option<&Remap>) {
        if self.trig.dead(&self.trig.l.toggles[ti].b) { return; }
        let (tg, activate) = { let t = &self.trig.l.toggles[ti]; (rm_v(remap, t.target), t.activate) };
        self.toggle_group(tg, activate);
    }

    pub fn check_toggle(&mut self, px: f64) {
        while self.trig.toggle_idx < self.trig.l.toggles.len() {
            let ti = self.trig.toggle_idx;
            let t = &self.trig.l.toggles[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.execute_toggle(ti, None); }
            self.trig.toggle_idx += 1;
        }
    }

    pub fn check_touch_toggle(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.toggles.len() {
            let t = &self.trig.l.toggles[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) { continue; }
            if self.trig.touch_toggle.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_toggle.insert(uid);
                self.execute_toggle(ti, None);
            }
        }
    }

    pub fn reset_toggle(&mut self) {
        self.trig.toggle_idx = 0;
        self.trig.touch_toggle.clear();
        self.trig.toggled_off.clear();
        for a in &mut self.trig.anchors { a.toggled.clear(); }
        for ci in 0..self.cols.len() {
            self.trig.meta[ci].toggled.clear();
            self.disabled[ci] = false;
        }
        self.sprite_reset_toggles();
    }

    // ---- spawn ----
    fn queue_spawn(&mut self, ti: usize) {
        let remap = match &self.trig.l.spawns[ti].remap { Some(m) => RemapRef::Map(m.clone()), None => RemapRef::Null };
        let tg = self.trig.l.spawns[ti].target;
        self.queue_spawn_with(ti, tg, remap);
    }

    fn queue_spawn_with(&mut self, ti: usize, tg: i64, remap: RemapRef) {
        if self.trig.dead(&self.trig.l.spawns[ti].b) { return; }
        let (delay, rdelay) = { let t = &self.trig.l.spawns[ti]; (t.delay, t.random_delay) };
        if tg <= 0 { return; }
        let base = js_max(0.0, or0(delay));
        let random = js_max(0.0, or0(rdelay));
        let randomized = if random > 0.0 { js_max(0.0, base + ((m::random() * 2.0) - 1.0) * random) } else { base };
        for p in &self.trig.active_spawn {
            if p.target == tg && p.remap == remap && ((p.delay - p.elapsed) - randomized).abs() < 1e-6 { return; }
        }
        self.trig.active_spawn.push(SpawnDelay { target: tg, delay: randomized, elapsed: 0.0, remap });
    }

    pub fn spawn_group_now(&mut self, g: i64) {
        if g <= 0 { return; }
        for p in &self.trig.active_spawn { if p.target == g && p.delay == 0.0 && p.elapsed == 0.0 { return; } }
        self.trig.active_spawn.push(SpawnDelay { target: g, delay: 0.0, elapsed: 0.0, remap: RemapRef::Undef });
    }

    pub fn check_spawn(&mut self, px: f64) {
        while self.trig.spawn_idx < self.trig.l.spawns.len() {
            let ti = self.trig.spawn_idx;
            let t = &self.trig.l.spawns[ti];
            if t.x > px { break; }
            if !t.b.spawn_triggered && !t.touch { self.queue_spawn(ti); }
            self.trig.spawn_idx += 1;
        }
    }

    pub fn check_touch_spawn(&mut self, px: f64, py: f64) {
        let (px, py) = (or0(px), or0(py));
        for ti in 0..self.trig.l.spawns.len() {
            let t = &self.trig.l.spawns[ti];
            if !t.touch || t.b.spawn_triggered || self.trig.dead(&t.b) { continue; }
            if self.trig.touch_spawn.contains(&t.b.uid) { continue; }
            if (px - t.x).abs() <= TOUCH_HALF_HITBOX && (py - t.y).abs() <= TOUCH_HALF_HITBOX {
                let uid = t.b.uid;
                self.trig.touch_spawn.insert(uid);
                self.queue_spawn(ti);
            }
        }
    }

    pub fn activate_spawned_group(&mut self, gid: i64) { self.activate_spawned_group_r(gid, None); }

    pub fn activate_spawned_group_r(&mut self, gid: i64, remap: Option<Remap>) {
        if gid <= 0 { return; }
        if self.trig.gen.snapshot.is_none() { self.trig.gen.snapshot = Some(self.trig.gen.order.clone()); }
        let mut fired_now: BTreeSet<i64> = BTreeSet::new();
        macro_rules! matches { ($list:ident) => {{
            let mut out: Vec<usize> = Vec::new();
            for i in 0..self.trig.l.$list.len() {
                let t = &self.trig.l.$list[i];
                if !(t.b.spawn_triggered && t.b.groups.contains(&gid)) { continue; }
                if self.trig.dead(&t.b) { continue; }
                if !t.b.multi {
                    let uid = t.b.uid;
                    if self.trig.spawn_fired.contains(&uid) && !fired_now.contains(&uid) { continue; }
                    self.trig.spawn_fired.insert(uid);
                    fired_now.insert(uid);
                }
                out.push(i);
            }
            out
        }}; }
        for i in matches!(colors) { self.apply_color_trigger(i, true); }
        for i in matches!(enter_effects) { let e = self.trig.l.enter_effects[i].effect; self.trig.active_enter = e; self.trig.active_exit = e; }
        let rmp = remap.clone();
        let rr = rmp.as_ref();
        for i in matches!(moves) { self.start_move(i, rr); }
        for i in matches!(alphas) { self.start_alpha(i, rr); }
        for i in matches!(rotates) { self.start_rotate(i, rr); }
        for i in matches!(stops) { self.execute_stop(i, rr); }
        for i in matches!(follow_ys) { self.start_follow_y(i, rr); }
        for i in matches!(follows) { self.start_follow(i, rr); }
        for i in matches!(toggles) { self.execute_toggle(i, rr); }
        for i in matches!(animations) { self.execute_animation(i, rr); }
        for i in matches!(shakes) { self.execute_shake(i); }
        for i in matches!(player_vis) { self.trig.player_hidden = self.trig.l.player_vis[i].hide; }
        for i in matches!(pulses) { self.start_pulse(i, rr); }
        for i in matches!(spawns) {
            match &remap {
                None => self.queue_spawn(i),
                Some(r) => {
                    let (tg, own) = { let t = &self.trig.l.spawns[i]; (t.target, t.remap.clone()) };
                    let tg = r.get(&tg).copied().unwrap_or(tg);
                    let mut merged: BTreeMap<i64, i64> = (**r).clone();
                    if let Some(o) = own { for (a, b) in o.iter() { merged.insert(*a, *b); } }
                    self.queue_spawn_with(i, tg, RemapRef::Map(std::sync::Arc::new(merged)));
                }
            }
        }
        for i in matches!(on_deaths) { self.arm_on_death(i, rr); }
        for i in matches!(touches) { self.arm_touch(i, rr); }
        for i in matches!(pickups) { self.execute_pickup(i); }
        for i in matches!(counts) { self.arm_count(i, rr); }
        for i in matches!(instant_counts) { self.execute_instant_count(i, rr); }
        for i in matches!(collisions) { self.arm_collision(i, rr); }
        self.activate_spawned_generic(gid, remap);
    }

    pub fn step_spawn(&mut self, dt: f64) {
        let mut i = 0usize;
        let mut guard = 0;
        while i < self.trig.active_spawn.len() && guard < 256 {
            guard += 1;
            self.trig.active_spawn[i].elapsed += dt;
            let (elapsed, delay, tg, remap) = { let it = &self.trig.active_spawn[i]; (it.elapsed, it.delay, it.target, match &it.remap { RemapRef::Map(m) => Some(m.clone()), _ => None }) };
            if elapsed >= delay {
                self.trig.active_spawn.remove(i);
                self.activate_spawned_group_r(tg, remap);
            } else {
                i += 1;
            }
            if i >= self.trig.active_spawn.len() {
                for p in &self.trig.active_spawn {
                    if p.delay <= p.elapsed { i = 0; break; }
                }
            }
        }
    }

    pub fn reset_spawn(&mut self) { self.trig.spawn_fired.clear(); self.trig.spawn_idx = 0; self.trig.active_spawn.clear(); self.trig.touch_spawn.clear(); }

    /// The trigger part of game-scene.js `update`, in its order. `dual_y` is the second player's Y when it is alive in dual mode.
    pub fn trigger_frame(&mut self, pos: Pos, dual_y: Option<f64>, dt: f64, input: Input) {
        self.trig.pos = pos;
        let px = pos.player_x;
        self.check_color(px);
        self.check_touch_color(px, pos.player_y);
        if let Some(y2) = dual_y { self.check_touch_color(px, y2); }
        if self.trig.phase_limit >= 1 { self.check_move(px); }
        if self.trig.phase_limit >= 2 { self.check_spawn(px); }
        if self.trig.phase_limit >= 3 { self.check_touch_spawn(px, pos.player_y); }
        if self.trig.phase_limit >= 4 { if let Some(y2) = dual_y { self.check_touch_spawn(px, y2); } }
        if self.trig.phase_limit >= 5 { self.check_touch_move(px, pos.player_y); }
        if self.trig.phase_limit >= 6 { if let Some(y2) = dual_y { self.check_touch_move(px, y2); } }
        if self.trig.phase_limit >= 7 { self.begin_group_transform_batch(); }
        if self.trig.phase_limit >= 8 { self.step_move(dt); self.update_generic(px, Some(pos.player_y), dual_y, dt); }
        if self.trig.phase_limit >= 9 { self.step_spawn(dt); }
        if self.trig.phase_limit >= 9 {
            self.check_alpha(px);
            self.check_touch_alpha(px, pos.player_y);
            if let Some(y2) = dual_y { self.check_touch_alpha(px, y2); }
            self.step_alpha(dt);
        }
        if self.trig.phase_limit >= 10 { self.check_rotate(px); }
        if self.trig.phase_limit >= 11 { self.step_rotate(dt); }
        if self.trig.phase_limit >= 12 { self.check_stop(px); }
        if self.trig.phase_limit >= 13 { self.check_touch_stop(px, pos.player_y); }
        if self.trig.phase_limit >= 14 { if let Some(y2) = dual_y { self.check_touch_stop(px, y2); } }
        if self.trig.phase_limit >= 15 { self.check_follow_y(px); }
        if self.trig.phase_limit >= 16 { self.check_touch_follow_y(px, pos.player_y); }
        if self.trig.phase_limit >= 17 { if let Some(y2) = dual_y { self.check_touch_follow_y(px, y2); } }
        if self.trig.phase_limit >= 18 { self.step_follow_y(dt); }
        if self.trig.phase_limit >= 19 { self.check_follow(px); }
        if self.trig.phase_limit >= 20 { self.check_touch_follow(px, pos.player_y); }
        if self.trig.phase_limit >= 21 { if let Some(y2) = dual_y { self.check_touch_follow(px, y2); } }
        if self.trig.phase_limit >= 22 { self.step_follow(dt); }
        if self.trig.phase_limit >= 23 { self.flush_group_transform_batch(); }
        if self.trig.phase_limit >= 24 { self.check_toggle(px); }
        self.check_animation(px);
        self.check_touch_animation(px, pos.player_y);
        if let Some(y2) = dual_y { self.check_touch_animation(px, y2); }
        if self.trig.phase_limit >= 25 { self.check_touch_toggle(px, pos.player_y); }
        if self.trig.phase_limit >= 26 { if let Some(y2) = dual_y { self.check_touch_toggle(px, y2); } }
        self.check_shake(px);
        self.check_player_vis(px);
        self.check_touch_player_vis(px, pos.player_y);
        if let Some(y2) = dual_y { self.check_touch_player_vis(px, y2); }
        self.check_touch_shake(px, pos.player_y);
        if let Some(y2) = dual_y { self.check_touch_shake(px, y2); }
        if self.trig.phase_limit >= 27 { self.check_on_death(px); }
        if self.trig.phase_limit >= 28 { self.check_touch_on_death(px, pos.player_y); }
        if self.trig.phase_limit >= 29 { if let Some(y2) = dual_y { self.check_touch_on_death(px, y2); } }
        if self.trig.phase_limit >= 30 { self.check_touch_triggers(px); }
        if self.trig.phase_limit >= 31 { self.check_touch_touch(px, pos.player_y); }
        if self.trig.phase_limit >= 32 { if let Some(y2) = dual_y { self.check_touch_touch(px, y2); } }
        let dual_inputs = self.touch_trigger_dual_mode();
        if self.trig.phase_limit >= 33 { self.step_touch(input.p1_hold, input.p1_press, dual_inputs && input.p2_hold, dual_inputs && input.p2_press); }
        if self.trig.phase_limit >= 34 { self.check_pickup(px); }
        if self.trig.phase_limit >= 35 { self.check_touch_pickup(px, pos.player_y); }
        if self.trig.phase_limit >= 36 { if let Some(y2) = dual_y { self.check_touch_pickup(px, y2); } }
        if self.trig.phase_limit >= 37 { self.check_count(px); }
        if self.trig.phase_limit >= 38 { self.check_collision_triggers(px); }
        if self.trig.phase_limit >= 39 { self.update_collision_triggers(); }
        if self.trig.phase_limit >= 40 { self.check_touch_count(px, pos.player_y); }
        if self.trig.phase_limit >= 41 { if let Some(y2) = dual_y { self.check_touch_count(px, y2); } }
        if self.trig.phase_limit >= 42 { self.check_instant_count(px); }
        if self.trig.phase_limit >= 43 { self.check_touch_instant_count(px, pos.player_y); }
        if self.trig.phase_limit >= 44 { if let Some(y2) = dual_y { self.check_touch_instant_count(px, y2); } }
        self.check_pulse(px);
        self.step_pulse(dt);
        self.cm.step(dt);
        self.check_enter_effect(px);
        self.apply_shake(0.0);
    }

    // ---- inspection (tests) ----
    pub fn trigger_state_json(&self) -> String {
        let t = &self.trig;
        let mut o = Obj::new();
        let offs: Vec<String> = t.offsets.iter().map(|(k, v)| format!("{}:[{},{}]", str_lit(&gkey(*k)), num(v.0), num(v.1))).collect();
        o.field("offsets", format!("{{{}}}", offs.join(",")));
        let rots: Vec<String> = t.rotations.iter().map(|(k, v)| format!("{}:[{},{},{}]", str_lit(&gkey(*k)), num(v.total_rad), v.center, v.lock_rotation)).collect();
        o.field("rotations", format!("{{{}}}", rots.join(",")));
        o.field("active", format!("[{},{},{},{},{}]", t.active_move.len(), t.active_rotate.len(), t.active_spawn.len(), t.active_follow_y.len(), t.active_follow.len()));
        let hist: Vec<String> = t.active_follow_y.iter().map(|a| format!("[{},{},{}]", num(a.elapsed), num(a.initial_gdy), a.history.len())).collect();
        o.field("followY", format!("[{}]", hist.join(",")));
        let fol: Vec<String> = t.active_follow.iter().map(|a| format!("[{},{},{}]", num(a.elapsed), num(a.last_x), num(a.last_y))).collect();
        o.field("follow", format!("[{}]", fol.join(",")));
        let sp: Vec<String> = t.active_spawn.iter().map(|s| format!("[{},{},{},{},[{}]]", s.target, num(s.delay), num(s.elapsed), match &s.remap { RemapRef::Undef => 0, RemapRef::Null => 1, RemapRef::Map(_) => 2 }, match &s.remap { RemapRef::Map(m) => m.iter().map(|(a, b)| format!("[{},{}]", a, b)).collect::<Vec<_>>().join(","), _ => String::new() })).collect();
        o.field("spawnDelays", format!("[{}]", sp.join(",")));
        o.field("idx", format!("[{},{},{},{},{},{},{},{},{},{},{},{},{}]", t.move_idx, t.rotate_idx, t.spawn_idx, t.stop_idx, t.toggle_idx, t.follow_y_idx, t.follow_idx, t.on_death_idx, t.touch_idx, t.pickup_idx, t.count_idx, t.instant_idx, t.collision_idx));
        let ints = |v: &[usize]| format!("[{}]", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","));
        o.field("armedOnDeath", ints(&t.armed_on_death.iter().map(|e| e.0).collect::<Vec<_>>()));
        o.field("activeTouch", ints(&t.active_touch.iter().map(|e| e.ti).collect::<Vec<_>>()));
        o.field("holding", format!("[{}]", t.active_touch.iter().map(|e| e.holding.to_string()).collect::<Vec<_>>().join(",")));
        o.field("activeCount", ints(&t.active_count.iter().map(|e| e.ti).collect::<Vec<_>>()));
        o.field("activeCollision", format!("[{}]", t.active_collision.iter().map(|e| format!("[{},{}]", e.ti, e.colliding)).collect::<Vec<_>>().join(",")));
        o.field("blending", format!("{{{}}}", self.channel_blending.iter().map(|(k, v)| format!("\"{}\":{}", k, v)).collect::<Vec<_>>().join(",")));
        o.field("pulses", format!("[{}]", t.active_pulses.iter().map(|p| format!("[{},{},{}]", p.trig, num(p.elapsed), num(p.total))).collect::<Vec<_>>().join(",")));
        o.field("groupOps", format!("[{}]", t.group_ops.iter().map(|(g, op)| format!("[{},{}]", g, op.map_or("null".to_string(), |(i, v)| format!("[{},{}]", i, num(v))))).collect::<Vec<_>>().join(",")));
        o.field("shake", format!("[{},{},{},{},{}]", t.shake_idx, num(t.shake_off.0), num(t.shake_off.1), t.shake_state.map_or("null".to_string(), |s| format!("[{},{},{},{}]", num(s.strength), num(s.end_time), num(s.interval), if s.last_step.is_finite() { num(s.last_step) } else { "\"-inf\"".to_string() })), t.touch_shake.len()));
        o.field("vis", format!("[{},{},{}]", t.vis_idx, t.player_hidden, t.touch_vis.len()));
        o.field("anim", format!("[{},{},[{}]]", t.anim_idx, t.touch_anim.len(), t.anim_events.iter().map(|e| num(e.1)).collect::<Vec<_>>().join(",")));
        o.field("enter", format!("[{},{},{}]", t.enter_idx, t.active_enter, t.active_exit));
        o.field("colorIdx", format!("[{},{}]", t.color_idx, t.pulse_idx));
        o.field("groupOpacity", format!("{{{}}}", t.group_opacity.iter().map(|(k, v)| format!("{}:{}", str_lit(&gkey(*k)), num(*v))).collect::<Vec<_>>().join(",")));
        o.field("alpha", format!("[{}]", std::iter::once(t.alpha_idx.to_string()).chain(t.active_alpha.iter().map(|a| format!("[{},{},{}]", a.trig, num(a.elapsed), num(a.start)))).collect::<Vec<_>>().join(",")));
        o.field("itemCounts", format!("{{{}}}", t.item_counts.iter().map(|(k, v)| format!("\"{}\":{}", k, num(*v))).collect::<Vec<_>>().join(",")));
        let off: Vec<String> = t.toggled_off.iter().map(|g| g.to_string()).collect();
        o.field("toggledOff", format!("[{}]", off.join(",")));
        o.field("generic", self.generic_state_json());
        o.finish()
    }

    pub fn trigger_lists_json(&self) -> String {
        let l = &self.trig.l;
        let b = |b: &Base| format!("\"uid\":{},\"spawnTriggered\":{},\"multiTrigger\":{},\"triggerChannel\":{},\"ord\":{},\"groups\":[{}]", b.uid, b.spawn_triggered, b.multi, b.channel, b.ord, b.groups.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(","));
        let k = |g: i64| if g == NAN_KEY { "null".to_string() } else { g.to_string() };
        let moves: Vec<String> = l.moves.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"duration\":{},\"easingType\":{},\"easingRate\":{},\"targetGroup\":{},\"offsetX\":{},\"offsetY\":{},\"lockX\":{},\"lockY\":{},\"lockCameraX\":{},\"lockCameraY\":{}}}",
            b(&t.b), num(t.x), num(t.y), t.touch, num(t.duration), num(t.easing_type), num(t.easing_rate), k(t.target), num(t.offset_x), num(t.offset_y), t.lock_x, t.lock_y, t.lock_cam_x, t.lock_cam_y)).collect();
        let rotates: Vec<String> = l.rotates.iter().map(|t| format!("{{{},\"x\":{},\"targetGroup\":{},\"degrees\":{},\"duration\":{},\"easingType\":{},\"easingRate\":{},\"lockRotation\":{},\"times360\":{},\"centerGroup\":{}}}",
            b(&t.b), num(t.x), t.target, num(t.degrees), num(t.duration), num(t.easing_type), num(t.easing_rate), t.lock_rotation, num(t.times360), t.center)).collect();
        let spawns: Vec<String> = l.spawns.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"delay\":{},\"randomDelay\":{},\"remap\":{}}}",
            b(&t.b), num(t.x), num(t.y), t.touch, k(t.target), num(t.delay), num(t.random_delay), t.remap.as_ref().map_or("null".to_string(), |m| format!("{{{}}}", m.iter().map(|(a, b)| format!("\"{}\":{}", a, b)).collect::<Vec<_>>().join(","))))).collect();
        let stops: Vec<String> = l.stops.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, k(t.target))).collect();
        let toggles: Vec<String> = l.toggles.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"activate\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, k(t.target), t.activate)).collect();
        let fys: Vec<String> = l.follow_ys.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"speed\":{},\"delay\":{},\"offset\":{},\"maxSpeed\":{},\"duration\":{}}}",
            b(&t.b), num(t.x), num(t.y), t.touch, k(t.target), num(t.speed), num(t.delay), num(t.offset), num(t.max_speed), num(t.duration))).collect();
        let fols: Vec<String> = l.follows.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"followGroup\":{},\"xMod\":{},\"yMod\":{},\"duration\":{}}}",
            b(&t.b), num(t.x), num(t.y), t.touch, k(t.target), k(t.follow_group), num(t.x_mod), num(t.y_mod), num(t.duration))).collect();
        let ods: Vec<String> = l.on_deaths.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"activate\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, k(t.target), t.activate)).collect();
        let tts: Vec<String> = l.touches.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"toggleType\":{},\"dualTouch\":{},\"holdMode\":{},\"playerFilter\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, k(t.target), num(t.toggle_type), t.dual_touch, t.hold_mode, num(t.player_filter))).collect();
        let pks: Vec<String> = l.pickups.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"itemId\":{},\"count\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, num(t.item_id), num(t.count))).collect();
        let cts: Vec<String> = l.counts.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"itemId\":{},\"targetCount\":{},\"activate\":{},\"multiActivate\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, k(t.target), num(t.item_id), num(t.target_count), t.activate, t.multi_activate)).collect();
        let ics: Vec<String> = l.instant_counts.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"itemId\":{},\"targetCount\":{},\"activate\":{},\"comparison\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, k(t.target), num(t.item_id), num(t.target_count), t.activate, num(t.comparison))).collect();
        let cls: Vec<String> = l.collisions.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"blockA\":{},\"blockB\":{},\"targetGroup\":{},\"activate\":{},\"onExit\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, num(t.block_a), num(t.block_b), k(t.target), t.activate, t.on_exit)).collect();
        let bls: Vec<String> = l.blocks.iter().map(|t| format!("{{\"blockId\":{},\"dynamic\":{},\"x\":{},\"y\":{},\"half\":{},\"_eeGroups\":[{}]}}", num(t.block_id), t.dynamic, num(t.x), num(t.y), num(t.half), t.groups.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(","))).collect();
        let hsvj = |h: Option<crate::colors::Hsv>| h.map_or("null".to_string(), |h| format!("{{\"h\":{},\"s\":{},\"v\":{},\"sAdd\":{},\"vAdd\":{}}}", num(h.h), num(h.s), num(h.v), h.s_add, h.v_add));
        let rgbj = |c: crate::colors::Rgb| format!("{{\"r\":{},\"g\":{},\"b\":{}}}", num(c.r), num(c.g), num(c.b));
        let cols: Vec<String> = l.colors.iter().map(|t| {
            let mut s = format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"index\":{},\"color\":{},\"duration\":{},\"tintGround\":{}", b(&t.b), num(t.x), num(t.y), t.touch, t.index, rgbj(t.color), num(t.duration), t.tint_ground);
            if let Some(bl) = t.blending { s.push_str(&format!(",\"blending\":{}", bl)); }
            if let Some(op) = t.opacity { s.push_str(&format!(",\"opacity\":{}", num(op))); }
            if t.blending.is_some() { s.push_str(&format!(",\"copy\":{}", t.copy.map_or("null".to_string(), |c| format!("{{\"source\":{},\"hsv\":{}}}", num(c.source), hsvj(c.hsv))))); }
            s.push('}');
            s
        }).collect();
        let pls: Vec<String> = l.pulses.iter().map(|t| format!("{{{},\"x\":{},\"targetGroup\":{},\"targetChannel\":{},\"targetType\":{},\"color\":{},\"fadeIn\":{},\"hold\":{},\"fadeOut\":{},\"hsv\":{},\"copySource\":{}}}",
            b(&t.b), num(t.x), num(t.target_group), num(t.target_channel), num(t.target_type), rgbj(t.color), num(t.fade_in), num(t.hold), num(t.fade_out), hsvj(t.hsv), num(t.copy_source))).collect();
        let shs: Vec<String> = l.shakes.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"duration\":{},\"strength\":{},\"interval\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, num(t.duration), num(t.strength), num(t.interval))).collect();
        let pvs: Vec<String> = l.player_vis.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"hide\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, t.hide)).collect();
        let ans: Vec<String> = l.animations.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"touchTriggered\":{},\"targetGroup\":{},\"animateId\":{}}}", b(&t.b), num(t.x), num(t.y), t.touch, num(t.target), num(t.animate_id))).collect();
        let ees: Vec<String> = l.enter_effects.iter().map(|t| format!("{{{},\"x\":{},\"effect\":{}}}", b(&t.b), num(t.x), t.effect)).collect();
        let als: Vec<String> = l.alphas.iter().map(|t| format!("{{{},\"x\":{},\"y\":{},\"duration\":{},\"targetGroup\":{},\"targetOpacity\":{}}}", b(&t.b), num(t.x), num(t.y), num(t.duration), k(t.target_group), num(t.target_opacity))).collect();
        let gens = crate::generic::gen_list_json(&self.trig.l.generics);
        format!("{{\"generic\":{},\"timeWarp\":[],\"move\":[{}],\"rotate\":[{}],\"spawn\":[{}],\"stop\":[{}],\"toggle\":[{}],\"followY\":[{}],\"follow\":[{}],\"onDeath\":[{}],\"touch\":[{}],\"pickup\":[{}],\"count\":[{}],\"instantCount\":[{}],\"collision\":[{}],\"blocks\":[{}],\"color\":[{}],\"pulse\":[{}],\"alpha\":[{}],\"shake\":[{}],\"playerVis\":[{}],\"animation\":[{}],\"enterEffect\":[{}]}}",
            gens, moves.join(","), rotates.join(","), spawns.join(","), stops.join(","), toggles.join(","), fys.join(","), fols.join(","), ods.join(","), tts.join(","), pks.join(","), cts.join(","), ics.join(","), cls.join(","), bls.join(","), cols.join(","), pls.join(","), als.join(","), shs.join(","), pvs.join(","), ans.join(","), ees.join(","))
    }
}

fn gkey(k: i64) -> String { if k == NAN_KEY { "NaN".to_string() } else { k.to_string() } }

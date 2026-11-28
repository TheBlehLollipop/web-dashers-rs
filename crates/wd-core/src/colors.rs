//! The colour channel manager (triggers.js `ColorManager`, `TriggerStepper`, HSV helpers).
use crate::physics::js_round;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb { pub r: f64, pub g: f64, pub b: f64 }
fn rgb(r: f64, g: f64, b: f64) -> Rgb { Rgb { r, g, b } }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv { pub h: f64, pub s: f64, pub v: f64, pub s_add: bool, pub v_add: bool }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Copy { pub source: f64, pub hsv: Option<Hsv> }

const FS: i64 = 1000;
const GS: i64 = 1001;

pub fn channel_hex_to_rgb(v: f64) -> Rgb {
    if !v.is_finite() { return rgb(255.0, 255.0, 255.0); }
    let n = v as i64;
    rgb(((n >> 16) & 255) as f64, ((n >> 8) & 255) as f64, (n & 255) as f64)
}

pub fn rgb_to_hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let diff = max - min;
    let mut h = 0.0;
    if diff != 0.0 {
        if max == r { h = 60.0 * (((g - b) / diff) % 6.0); }
        else if max == g { h = 60.0 * ((b - r) / diff + 2.0); }
        else { h = 60.0 * ((r - g) / diff + 4.0); }
        if h < 0.0 { h += 360.0; }
    }
    (h, if max == 0.0 { 0.0 } else { (diff / max) * 100.0 }, (max / 255.0) * 100.0)
}

fn sector(h: f64, c: f64, x: f64) -> (f64, f64, f64) {
    if h < 60.0 { (c, x, 0.0) } else if h < 120.0 { (x, c, 0.0) } else if h < 180.0 { (0.0, c, x) } else if h < 240.0 { (0.0, x, c) } else if h < 300.0 { (x, 0.0, c) } else { (c, 0.0, x) }
}

pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Rgb {
    let sat = s.max(0.0).min(100.0) / 100.0;
    let val = v.max(0.0).min(100.0) / 100.0;
    let c = val * sat;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = val - c;
    let (r, g, b) = sector(h, c, x);
    rgb(js_round((r + m) * 255.0), js_round((g + m) * 255.0), js_round((b + m) * 255.0))
}

pub fn light_bg_color(bg: Rgb, p1: Rgb) -> Rgb {
    let max = bg.r.max(bg.g).max(bg.b) / 255.0;
    let min = bg.r.min(bg.g).min(bg.b) / 255.0;
    let base = rgb_to_hsv(bg.r, bg.g, bg.b);
    let sat = 0.0001f64.max(1f64.min((if max == 0.0 { 0.0 } else { (max - min) / max }) - 0.2));
    let val = 0.0001f64.max(1f64.min(max + 0.2));
    let c = val * sat;
    let h = base.0;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = val - c;
    let (r, g, b) = sector(h, c, x);
    let target = rgb(((r + m) * 255.0).trunc(), ((g + m) * 255.0).trunc(), ((b + m) * 255.0).trunc());
    let sum = bg.r + bg.g + bg.b;
    if sum >= 150.0 { return target; }
    let f = sum / 150.0;
    let mix = |a: f64, t: f64| (0f64.max(255f64.min((1.0 - f) * a + f * t))).trunc();
    rgb(mix(p1.r, target.r), mix(p1.g, target.g), mix(p1.b, target.b))
}

/// `parseChannelHsv`: "h a s a v a sAdd a vAdd"
pub fn parse_channel_hsv(value: Option<&str>) -> Option<Hsv> {
    let value = value.filter(|v| !v.is_empty())?;
    let parts: Vec<&str> = value.split('a').collect();
    let num = |i: usize, d: f64| parts.get(i).and_then(|p| crate::js::parse_float(p)).filter(|n| n.is_finite()).unwrap_or(d);
    Some(Hsv { h: num(0, 0.0), s: num(1, 1.0), v: num(2, 1.0), s_add: parts.get(3) == Some(&"1"), v_add: parts.get(4) == Some(&"1") })
}

pub fn apply_channel_hsv(color: Rgb, hsv: Option<Hsv>) -> Rgb {
    let Some(hsv) = hsv else { return color };
    let base = rgb_to_hsv(color.r, color.g, color.b);
    let mut h = (base.0 + hsv.h) % 360.0;
    if h < 0.0 { h += 360.0; }
    let s = if hsv.s_add { base.1 / 100.0 + hsv.s } else { (base.1 / 100.0) * hsv.s };
    let v = if hsv.v_add { base.2 / 100.0 + hsv.v } else { (base.2 / 100.0) * hsv.v };
    hsv_to_rgb(h, s * 100.0, v * 100.0)
}

#[derive(Clone, Debug)]
struct Stepper { from: Rgb, to: Rgb, duration: f64, elapsed: f64, done: bool, current: Rgb, copy: Option<Copy> }
impl Stepper {
    fn new(old: Rgb, new: Rgb, duration: f64) -> Stepper {
        let done = duration <= 0.0;
        Stepper { from: old, to: new, duration, elapsed: 0.0, done, current: if done { new } else { old }, copy: None }
    }
    fn step(&mut self, ms: f64) {
        if self.done { return; }
        self.elapsed += ms;
        let progress = if self.duration > 0.0 { (self.elapsed / self.duration).min(1.0) } else { 1.0 };
        if progress >= 1.0 { self.current = self.to; self.done = true; }
        else {
            self.current = rgb(
                js_round(self.from.r + (self.to.r - self.from.r) * progress),
                js_round(self.from.g + (self.to.g - self.from.g) * progress),
                js_round(self.from.b + (self.to.b - self.from.b) * progress));
        }
    }
}

#[derive(Clone, Debug)]
pub struct Pulse { channel: f64, color: Rgb, hsv: Option<Hsv>, copy_source: f64, fade_in: f64, hold: f64, fade_out: f64, total: f64, elapsed: f64 }

#[derive(Clone, Debug)]
struct OpacityAction { from: f64, to: f64, duration: f64, elapsed: f64 }

#[derive(Clone, Debug, Default)]
pub struct ColorManager {
    pub main_color: f64,
    pub secondary_color: f64,
    initial_colors: BTreeMap<i64, Rgb>,
    initial_copies: BTreeMap<i64, Copy>,
    initial_opacity: BTreeMap<i64, f64>,
    colors: BTreeMap<i64, Rgb>,
    copies: BTreeMap<i64, Copy>,
    opacity: BTreeMap<i64, f64>,
    opacity_actions: BTreeMap<i64, OpacityAction>,
    pulses: Vec<Pulse>,
    colored: BTreeSet<i64>,
    actions: BTreeMap<i64, Stepper>,
}

impl ColorManager {
    pub fn new(main_color: f64, secondary_color: f64) -> ColorManager {
        let mut c = ColorManager { main_color, secondary_color, ..Default::default() };
        c.reset();
        c
    }
    pub fn set_initial_opacity(&mut self, ch: i64, op: f64) { if !op.is_finite() { return; } self.initial_opacity.insert(ch, op); self.opacity.insert(ch, op); }
    pub fn set_initial_color(&mut self, ch: i64, c: Rgb) { self.initial_colors.insert(ch, c); self.colors.insert(ch, c); }
    pub fn set_initial_copy(&mut self, ch: i64, c: Copy) { if !(c.source > 0.0) { return; } self.initial_copies.insert(ch, c); self.copies.insert(ch, c); }
    pub fn reset(&mut self) {
        self.colors = BTreeMap::new();
        self.colors.insert(FS, rgb(0.0, 102.0, 255.0));
        self.colors.insert(GS, rgb(0.0, 68.0, 170.0));
        for k in [1002, 1003, 1004] { self.colors.insert(k, rgb(255.0, 255.0, 255.0)); }
        for (k, v) in &self.initial_colors { self.colors.insert(*k, *v); }
        self.copies = self.initial_copies.clone();
        self.opacity = self.initial_opacity.clone();
        self.opacity_actions.clear();
        self.pulses.clear();
        self.colored.clear();
        self.actions.clear();
    }
    pub fn start_pulse(&mut self, channel: f64, color: Rgb, hsv: Option<Hsv>, copy_source: f64, fade_in: f64, hold: f64, fade_out: f64) {
        let fi = 0f64.max(if fade_in.is_nan() { 0.0 } else { fade_in });
        let ho = 0f64.max(if hold.is_nan() { 0.0 } else { hold });
        let fo = 0f64.max(if fade_out.is_nan() { 0.0 } else { fade_out });
        self.pulses.push(Pulse { channel, color, hsv, copy_source: if copy_source > 0.0 { copy_source } else { 0.0 }, fade_in: fi, hold: ho, fade_out: fo, total: 0.01f64.max(fi + ho + fo), elapsed: 0.0 });
    }
    pub fn clear_pulses(&mut self) { self.pulses.clear(); }

    fn apply_pulses(&self, index: i64, color: Rgb, depth: u32) -> Rgb {
        let id = index as f64;
        let mut out = color;
        for p in &self.pulses {
            if p.channel != id { continue; }
            let t = p.elapsed;
            let mut intensity = if t < p.fade_in { t / p.fade_in } else if t < p.fade_in + p.hold { 1.0 } else if p.fade_out > 0.0 { 1.0 - (t - p.fade_in - p.hold) / p.fade_out } else { 0.0 };
            if intensity <= 0.0 { continue; }
            if intensity > 1.0 { intensity = 1.0; }
            let target = match p.hsv {
                Some(h) => apply_channel_hsv(if p.copy_source > 0.0 && depth < 8 { self.get_color_depth(p.copy_source as i64, depth + 1) } else { out }, Some(h)),
                None => p.color,
            };
            out = rgb(js_round(out.r + (target.r - out.r) * intensity), js_round(out.g + (target.g - out.g) * intensity), js_round(out.b + (target.b - out.b) * intensity));
        }
        out
    }

    fn resolve_copy(&self, copy: &Copy, depth: u32) -> Rgb { apply_channel_hsv(self.get_color_depth(copy.source as i64, depth + 1), copy.hsv) }

    pub fn trigger_color(&mut self, index: i64, new_color: Rgb, duration: f64, copy: Option<Copy>, opacity: Option<f64>) {
        self.colored.insert(index);
        if let Some(o) = opacity.filter(|o| o.is_finite()) {
            let to = 0f64.max(1f64.min(o));
            if duration <= 0.0 { self.opacity.insert(index, to); self.opacity_actions.remove(&index); }
            else { let from = self.get_opacity(index); self.opacity_actions.insert(index, OpacityAction { from, to, duration, elapsed: 0.0 }); }
        }
        let old = self.get_base_color(index, 0);
        let has_copy = copy.filter(|c| c.source > 0.0 && c.source != index as f64);
        match has_copy { Some(c) => { self.copies.insert(index, c); } None => { self.copies.remove(&index); } }
        let target = match has_copy { Some(c) => self.resolve_copy(&c, 0), None => new_color };
        let mut action = Stepper::new(old, target, duration);
        action.copy = has_copy;
        self.actions.insert(index, action);
        if duration <= 0.0 { self.colors.insert(index, target); }
    }

    pub fn step(&mut self, ms: f64) {
        if !self.pulses.is_empty() {
            let mut i = self.pulses.len() as isize - 1;
            while i >= 0 {
                let p = &mut self.pulses[i as usize];
                p.elapsed += ms;
                if p.elapsed >= p.total { self.pulses.remove(i as usize); }
                i -= 1;
            }
        }
        let keys: Vec<i64> = self.opacity_actions.keys().copied().collect();
        for k in keys {
            let op = self.opacity_actions.get_mut(&k).unwrap();
            op.elapsed += ms;
            let t = (op.elapsed / op.duration).min(1.0);
            let v = op.from + (op.to - op.from) * t;
            self.opacity.insert(k, v);
            if t >= 1.0 { self.opacity_actions.remove(&k); }
        }
        let keys: Vec<i64> = self.actions.keys().copied().collect();
        for k in keys {
            let copy = self.actions[&k].copy;
            if let Some(c) = copy { let to = self.resolve_copy(&c, 0); self.actions.get_mut(&k).unwrap().to = to; }
            let a = self.actions.get_mut(&k).unwrap();
            a.step(ms);
            let (cur, done) = (a.current, a.done);
            self.colors.insert(k, cur);
            if done { self.actions.remove(&k); }
        }
    }

    pub fn get_color(&self, index: i64) -> Rgb { self.get_color_depth(index, 0) }
    fn get_color_depth(&self, index: i64, depth: u32) -> Rgb {
        let base = self.get_base_color(index, depth);
        if self.pulses.is_empty() { base } else { self.apply_pulses(index, base, depth) }
    }

    fn get_base_color(&self, index: i64, depth: u32) -> Rgb {
        if index == 1010 && !self.has_color(index) { return rgb(0.0, 0.0, 0.0); }
        if index == 1011 && !self.has_color(index) { return rgb(255.0, 255.0, 255.0); }
        if let Some(copy) = self.copies.get(&index) {
            if depth < 8 && !self.actions.get(&index).map_or(false, |a| !a.done) { return self.resolve_copy(copy, depth); }
        }
        if let Some(c) = self.colors.get(&index) { return *c; }
        match index {
            1005 => channel_hex_to_rgb(self.main_color),
            1006 => channel_hex_to_rgb(self.secondary_color),
            1007 => light_bg_color(self.get_color(FS), if depth < 8 { self.get_color_depth(1005, depth + 1) } else { channel_hex_to_rgb(self.main_color) }),
            1009 => self.get_color_depth(GS, depth + 1),
            _ => rgb(255.0, 255.0, 255.0),
        }
    }

    pub fn get_opacity(&self, index: i64) -> f64 { self.opacity.get(&index).copied().unwrap_or(1.0) }
    pub fn has_color(&self, index: i64) -> bool { self.colored.contains(&index) }
    pub fn get_hex(&self, index: i64) -> f64 {
        let c = self.get_color(index);
        (((c.r as i64) << 16) | ((c.g as i64) << 8) | (c.b as i64)) as u32 as f64
    }
}

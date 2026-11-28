//! What the triggers do to sprites (level.js: `_applyGroupedSpriteTransform`, toggle visibility, alpha, enter effects, invisible-block fade).
//! `Game::fx` owns the sprites; `trig.anchors` mirror them (same index) so the group centres the collider code uses stay in step.

use crate::background::Background;
use crate::game::Game;
use crate::ground::{Ground, GroundFrame};
use crate::triggers::Anchor;
use crate::visual::{Sprite, Val};
use std::collections::BTreeMap;

fn or0(v: f64) -> f64 { if v.is_nan() { 0.0 } else { v } }

#[derive(Clone, Debug)]
pub struct InvisibleEntry { pub x: f64, pub sprites: Vec<usize>, pub alpha: f64, pub removed: bool }

#[derive(Default, Debug)]
pub struct Fx {
    pub sprites: Vec<Sprite>,
    /// per sprite, for `sprite_hidden` without property lookups: -2 top layer (never hidden), -1 no section, else the section index
    pub hide_key: Vec<i64>,
    /// `_sections`: sprite indices per 400-wide section
    pub sections: BTreeMap<u64, Vec<usize>>,
    /// `_sections.length` (a JS array: only indices below 2^32 - 1 extend it)
    pub sections_len: u64,
    pub group_sprites: BTreeMap<i64, Vec<usize>>,
    /// per sprite: the unique positive group ids (`_getObjectGroupIds`)
    pub ids: Vec<Vec<i64>>,
    pub invisible: Vec<InvisibleEntry>,
    pub active_invisible: Vec<usize>,
    pub alpha_force: bool,
    /// identity of the `_groupOpacity` object last applied (0 = never)
    pub alpha_applied_gen: u64,
    pub opacity_gen: u64,
    pub alpha_applied_glow: Option<f64>,
    pub glow_multiplier: f64,
    pub portal_guide: bool,
    pub orb_guide: bool,
    pub player_x: f64,
    /// `_colorChannelSprites`
    pub channel_sprites: BTreeMap<i64, Vec<usize>>,
    /// per sprite: the channels it is registered to, ascending (`_eeTintChannels`)
    pub tint_channels: Vec<Vec<i64>>,
    /// `_sectionContainers.length`
    pub container_len: u64,
    pub emitters: Vec<usize>,
    /// sprites of the top layer (they are drawn whatever section they are in)
    pub top_sprites: Vec<usize>,
    pub lists: crate::visual::SpriteLists,
    /// the frames each animated sprite cycles through, as indices into `frames` (None = no such texture)
    pub anim_frames: BTreeMap<usize, Vec<Option<u32>>>,
    pub anim_timer: f64,
    pub pulse_meter: Option<f64>,
    /// the music's metering value the audio-reactive sprites follow (0 until audio is ported)
    pub audio_meter: f64,
    /// the analyser's loudest sample and the music volume, as the host last reported them
    pub audio_peak: (f64, f64),
    pub meter: crate::audio::Meter,
    /// when set, `animate_sprites` drives `audio_meter` from `meter` (otherwise the host sets `audio_meter` itself)
    pub meter_live: bool,
    /// main-menu mode: the level scrolls by behind the menu with a rainbow background and ground
    pub menu_mode: bool,
    /// level editor view: static level drawn through `ed_cam` (camera x, camera y, zoom); `ed_tint[si]` overrides a sprite's tint (-1 = none)
    pub editor_mode: bool,
    /// the editor shows glow sprites only when `showEditorGlow` is on
    pub ed_glow: bool,
    /// editor layer filter: sprites that do not match the active layer are drawn at a quarter of their opacity
    pub ed_dim: Vec<bool>,
    pub ed_cam: (f64, f64, f64),
    pub ed_tint: Vec<f64>,
    /// per level object: its sprites `start..end` (editor hit-testing and selection)
    pub obj_ranges: Vec<(u32, u32)>,
    pub menu_camera_x: f64,
    /// slide-in after pressing play: the slow ground/camera x while it runs
    pub slide: Option<f64>,
    pub slide_done: bool,
    pub menu_rainbow: f64,
    pub menu_colors: (f64, f64),
    // presentation state the renderer reads
    pub ground: Option<Ground>,
    pub ground_frame: GroundFrame,
    pub has_ground2: bool,
    pub bg: Background,
    /// (atlas, frame) pairs, and each sprite's index into them
    pub frames: Vec<(String, String)>,
    pub sprite_frame: Vec<u32>,
    pub render_buf: Vec<f64>,
    pub vis_min: i64,
    pub vis_max: i64,
    // the colour-apply cache of `_applyVisibleColorChannels`
    pub color_dirty: bool,
    pub color_applied: Option<(i64, i64)>,
    pub color_last: BTreeMap<i64, (f64, bool, f64)>,
    pub color_frames_since: u32,
    pub color_changed: Vec<i64>,
    pub visible_by_channel: Option<BTreeMap<i64, Vec<usize>>>,
    pub watch: Vec<i64>,
    pub index_built: bool,
    pub coin_tweens: Vec<crate::coins::CoinTween>,
    /// (coin sprite, collected) -> the animation frames of that look
    pub coin_frames: BTreeMap<(usize, bool), Vec<Option<u32>>>,
}

pub fn section_index(world_x: f64) -> u64 { (or0(world_x) / 400.0).floor().max(0.0) as u64 }

impl Game {
    /// Hands the spawned sprites to the game. `group_sprites` is `_groupSprites` (registration order).
    pub fn attach_sprites(&mut self, sprites: Vec<Sprite>, group_sprites: BTreeMap<i64, Vec<usize>>, invisible: Vec<(f64, Vec<usize>)>, channel_sprites: BTreeMap<i64, Vec<usize>>, emitters: Vec<usize>, lists: crate::visual::SpriteLists, anim_names: BTreeMap<usize, Vec<Option<(String, String)>>>) {
        let n = sprites.len();
        let mut sections: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
        let mut sections_len = 0u64;
        let mut ids: Vec<Vec<i64>> = vec![Vec::new(); n];
        let mut anchors: Vec<Anchor> = Vec::with_capacity(n);
        for (i, s) in sprites.iter().enumerate() {
            if let Some(si) = s.num("_eeSectionIndex") {
                let si = si as u64;
                if si < 4294967295 { sections_len = sections_len.max(si + 1); }
                sections.entry(si).or_default().push(i);
            }
            let mut gids: Vec<i64> = Vec::new();
            if let Some(Val::Arr(g)) = s.props.get("_eeGroups") {
                for v in g { let v = *v as i64; if v > 0 && !gids.contains(&v) { gids.push(v); } }
            }
            let x = s.num("_eeInitialWorldX").or_else(|| s.num("_eeWorldX")).unwrap_or(s.x);
            let y = s.num("_eeInitialBaseY").or_else(|| s.num("_eeBaseY")).unwrap_or(s.y);
            anchors.push(Anchor { x, y, groups: gids.clone(), toggled: Default::default() });
            ids[i] = gids;
        }
        // the group lists the trigger code walks: unique sprites in registration order
        let mut group_anchors: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
        for (g, list) in &group_sprites {
            let mut uniq: Vec<usize> = Vec::with_capacity(list.len());
            let mut seen: std::collections::HashSet<usize, std::hash::BuildHasherDefault<crate::fasthash::FastHasher>> = Default::default();
            for &si in list { if seen.insert(si) { uniq.push(si); } }
            group_anchors.insert(*g, uniq);
        }
        self.trig.anchors = anchors;
        self.trig.group_anchors = group_anchors;
        let mut tint_channels: Vec<Vec<i64>> = vec![Vec::new(); n];
        for (ch, list) in &channel_sprites { for &si in list { tint_channels[si].push(*ch); } }
        let container_len = sprites.iter().filter(|s| s.num("_eeLayer") != Some(2.0)).filter_map(|s| s.num("_eeSectionIndex")).fold(0u64, |m, v| if (v as u64) < 4294967295 { m.max(v as u64 + 1) } else { m });
        let fx = &mut self.fx;
        fx.channel_sprites = channel_sprites;
        fx.emitters = emitters;
        {
            // frame numbers: one entry per distinct (atlas, frame) pair; the keys borrow the sprites' names, so nothing is copied per sprite
            let mut ids: crate::fasthash::FastMap<(&str, &str), u32> = Default::default();
            let mut frames: Vec<(String, String)> = Vec::new();
            let mut sprite_frame = Vec::with_capacity(sprites.len());
            for s in &sprites {
                let key = (s.atlas.as_str(), s.frame.as_str());
                let id = match ids.get(&key) { Some(&id) => id, None => { frames.push((s.atlas.clone(), s.frame.clone())); let id = (frames.len() - 1) as u32; ids.insert(key, id); id } };
                sprite_frame.push(id);
            }
            let mut anim_frames: BTreeMap<usize, Vec<Option<u32>>> = BTreeMap::new();
            let mut extra: crate::fasthash::FastMap<(String, String), u32> = Default::default(); // names that no sprite uses by itself (animation frames)
            for (si, names) in anim_names {
                let ids2: Vec<Option<u32>> = names.into_iter().map(|n| n.map(|key| {
                    if let Some(&id) = ids.get(&(key.0.as_str(), key.1.as_str())) { return id; }
                    if let Some(&id) = extra.get(&key) { return id; }
                    frames.push(key.clone()); let id = (frames.len() - 1) as u32; extra.insert(key, id); id
                })).collect();
                anim_frames.insert(si, ids2);
            }
            fx.frames = frames;
            fx.sprite_frame = sprite_frame;
            fx.anim_frames = anim_frames;
        }
        fx.lists = lists;
        fx.anim_timer = 0.0;
        fx.pulse_meter = None;
        fx.tint_channels = tint_channels;
        fx.container_len = container_len;
        fx.vis_min = -1;
        fx.vis_max = -1;
        fx.color_dirty = false;
        fx.color_applied = None;
        fx.color_last.clear();
        fx.color_frames_since = 0;
        fx.color_changed.clear();
        fx.visible_by_channel = None;
        fx.watch.clear();
        fx.index_built = false;
        fx.top_sprites = sprites.iter().enumerate().filter(|(_, s)| s.num("_eeLayer") == Some(2.0)).map(|(i, _)| i).collect();
        fx.hide_key = sprites.iter().map(|s| if s.num("_eeLayer") == Some(2.0) { -2 } else { s.num("_eeSectionIndex").map_or(-1, |v| v as i64) }).collect();
        fx.sprites = sprites;
        fx.sections = sections;
        fx.sections_len = sections_len;
        fx.group_sprites = group_sprites;
        fx.ids = ids;
        let mut invisible = invisible;
        invisible.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal)); // `_invisibleBlockEntries.sort`, stable
        fx.invisible = invisible.into_iter().map(|(x, sprites)| InvisibleEntry { x, sprites, alpha: 0.0, removed: false }).collect();
        fx.active_invisible.clear();
        fx.glow_multiplier = 0.425;
        fx.portal_guide = true;
        fx.orb_guide = true;
        fx.opacity_gen = 1;
        fx.alpha_applied_gen = 0;
        fx.alpha_force = false;
        fx.alpha_applied_glow = None;
    }

    fn guide_hidden(&self, si: usize) -> bool {
        let s = &self.fx.sprites[si];
        (s.flag("_eePortalGuide") && !self.fx.portal_guide) || (s.flag("_eeOrbGuide") && !self.fx.orb_guide) || s.flag("_beastStaticHidden")
    }

    /// `_getGroupOpacityForSprite`
    pub fn group_opacity_for_sprite(&self, si: usize) -> f64 {
        let s = &self.fx.sprites[si];
        let mut op = 1.0;
        for g in &self.fx.ids[si] {
            if let Some(&v) = self.trig.group_opacity.get(g) { if v < op { op = v; } }
        }
        if let Some(ch) = s.num("_eeColorChannel") { if ch != 0.0 && !ch.is_nan() { op *= self.cm.get_opacity(ch as i64); } }
        if s.flag("_eeIsGlowSprite") { op *= self.fx.glow_multiplier; }
        else if let Some(oa) = s.num("_eeOrigAlpha") { if oa != 1.0 { op *= oa; } }
        if let Some(ap) = s.num("_eeApproachAlpha") { op *= ap; }
        op
    }

    /// `_setEffectScale`
    fn set_effect_scale(&mut self, si: usize, mult: f64) {
        let s = &mut self.fx.sprites[si];
        let base_u = s.num("_eeBaseScale").unwrap_or(f64::NAN);
        let bx = if base_u > 0.0 { base_u } else { s.num("_eeObjScaleX").unwrap_or(1.0) };
        let by = if base_u > 0.0 { base_u } else { s.num("_eeObjScaleY").unwrap_or(1.0) };
        let pulse = match s.num("_eePulseMult") { Some(p) if p != 0.0 && !p.is_nan() => p, _ => 1.0 };
        s.set_num("_eeEffectMult", mult);
        let (tx, ty) = (bx * mult * pulse, by * mult * pulse);
        if s.scale_x != tx || s.scale_y != ty { s.scale_x = tx; s.scale_y = ty; }
    }

    /// `sprite.setVisible(isVisible); sprite.active = isVisible` after a toggle
    pub(crate) fn sprite_toggle_group(&mut self, tg: i64) {
        self.fx.alpha_force = true;
        self.fx.color_dirty = true;
        if self.fx.sprites.is_empty() { return; }
        let list = match self.trig.group_anchors.get(&tg) { Some(l) => l.clone(), None => return };
        for si in list {
            let hidden_guide = self.guide_hidden(si);
            let vis = !hidden_guide && self.trig.anchors[si].toggled.is_empty();
            let s = &mut self.fx.sprites[si];
            s.visible = vis;
            s.active = vis;
        }
    }

    /// `resetToggleTriggers`, the sprite half
    pub(crate) fn sprite_reset_toggles(&mut self) {
        for si in 0..self.fx.sprites.len() {
            if self.fx.ids[si].is_empty() { continue; }
            let hidden_guide = self.guide_hidden(si);
            let s = &mut self.fx.sprites[si];
            s.visible = !hidden_guide;
            s.active = !hidden_guide;
        }
    }

    /// `_applyGroupedSpriteTransform`
    pub(crate) fn apply_sprite_transform(&mut self, si: usize) {
        if !self.fx.sprites[si].active { return; }
        self.fx.sprites[si].set_flag("_eeXformDirty", false);
        let ids = std::mem::take(&mut self.fx.ids[si]); // put back below: no copy per sprite per frame
        let (initial_x, initial_y, init_rot, vis_x0, vis_y0, beast) = {
            let s = &self.fx.sprites[si];
            let ix = s.num("_eeInitialWorldX").or_else(|| s.num("_origWorldX")).unwrap_or(s.x);
            let iy = s.num("_eeInitialBaseY").or_else(|| s.num("_origBaseY")).unwrap_or(s.y);
            (ix, iy, s.num("_eeInitialRotationRad"), s.num("_eeVisOffX").unwrap_or(0.0), s.num("_eeVisOffY").unwrap_or(0.0), s.flag("_beastManaged"))
        };
        let (mut move_x, mut move_y) = (0.0, 0.0);
        for g in &ids { if let Some(off) = self.trig.offsets.get(g) { move_x += or0(off.0); move_y += or0(off.1); } }
        let mut fx_ = initial_x + move_x;
        let mut fy = initial_y + move_y;
        let mut has_rot = false;
        let mut frot = init_rot.unwrap_or(0.0);
        for g in &ids {
            let Some(rd) = self.trig.rotations.get(g).cloned() else { continue };
            if rd.total_rad == 0.0 { continue; }
            has_rot = true;
            if !rd.lock_rotation { frot += rd.total_rad; }
            if rd.center > 0 {
                let (cx, cy) = self.center_cached(rd.center);
                let (dx, dy) = (fx_ - cx, fy - cy);
                let (c, s) = (crate::jsmath::cos(rd.total_rad), crate::jsmath::sin(rd.total_rad));
                fx_ = cx + dx * c - dy * s;
                fy = cy + dx * s + dy * c;
            }
        }
        self.fx.ids[si] = ids;
        let s = &mut self.fx.sprites[si];
        if !beast {
            let (mut vx, mut vy) = (vis_x0, vis_y0);
            if has_rot && (vx != 0.0 || vy != 0.0) {
                let d_rot = frot - init_rot.unwrap_or(0.0);
                let (c, sn) = (crate::jsmath::cos(d_rot), crate::jsmath::sin(d_rot));
                let rvx = vx * c - vy * sn;
                vy = vx * sn + vy * c;
                vx = rvx;
            }
            s.x = fx_ + vx;
            s.y = fy + vy;
            if has_rot { s.rotation = frot; }
        }
        s.set_num("_eeWorldX", fx_);
        s.set_num("_eeBaseY", fy);
        let new_sec = section_index(fx_);
        if s.num("_eeSectionIndex").map_or(true, |v| v as u64 != new_sec) { self.refresh_sprite_section(si); }
        let s = &mut self.fx.sprites[si];
        if s.num("_coinWorldX").is_some() { s.set_num("_coinWorldX", s.x / 2.0); }
        if s.num("_coinWorldY").is_some() { s.set_num("_coinWorldY", (460.0 - s.y) / 2.0); }
    }

    /// `_refreshSpriteSection`
    fn refresh_sprite_section(&mut self, si: usize) {
        let wx = match self.fx.sprites[si].num("_eeWorldX") { Some(v) => v, None => return };
        let next = section_index(wx);
        let prev = self.fx.sprites[si].num("_eeSectionIndex").map(|v| v as u64);
        if prev == Some(next) { return; }
        if let Some(p) = prev { if let Some(list) = self.fx.sections.get_mut(&p) { if let Some(pos) = list.iter().position(|&x| x == si) { list.remove(pos); } } }
        if next < 4294967295 && self.fx.sections_len < next + 1 { self.fx.sections_len = next + 1; }
        let list = self.fx.sections.entry(next).or_default();
        if !list.contains(&si) { list.push(si); }
        self.fx.sprites[si].set_num("_eeSectionIndex", next as f64);
        if let Some(k) = self.fx.hide_key.get_mut(si) { if *k != -2 { *k = next as i64; } }
        self.fx.color_dirty = true;
        // `_placeFlatSprite` puts a sprite of a hidden section into that section's (new) container
        if self.fx.sprites[si].num("_eeLayer") != Some(2.0) {
            let shown = self.fx.vis_min >= 0 && next as i64 >= self.fx.vis_min && next as i64 <= self.fx.vis_max;
            if !shown && next < 4294967295 && self.fx.container_len < next + 1 { self.fx.container_len = next + 1; }
        }
    }

    /// the sprite loop of `_applyGroupTransforms`
    pub(crate) fn apply_sprite_group_transforms(&mut self, groups: &[i64]) {
        if self.fx.sprites.is_empty() { return; }
        let mut seen = std::collections::BTreeSet::new();
        let mut order: Vec<usize> = Vec::new();
        for g in groups {
            if let Some(list) = self.trig.group_anchors.get(g) { for &si in list { if seen.insert(si) { order.push(si); } } }
        }
        let lazy = !self.ldm && self.fx.vis_min >= 0;
        for si in order {
            if lazy && self.skip_far_sprite_transform(si) { continue; }
            self.apply_sprite_transform(si);
        }
    }

    /// the sprite half of `resetMoveTriggers`
    pub(crate) fn sprite_reset_move(&mut self) {
        if self.fx.sprites.is_empty() { return; }
        let mut seen = std::collections::BTreeSet::new();
        let order: Vec<usize> = self.fx.group_sprites.values().flatten().copied().filter(|s| seen.insert(*s)).collect();
        for si in order {
            let s = &mut self.fx.sprites[si];
            let bx = s.num("_eeInitialWorldX").or_else(|| s.num("_origWorldX")).unwrap_or(s.x);
            let by = s.num("_eeInitialBaseY").or_else(|| s.num("_origBaseY")).unwrap_or(s.y);
            s.set_num("_eeMoveBaseWorldX", bx); s.set_num("_eeMoveBaseBaseY", by);
            s.set_num("_origWorldX", bx); s.set_num("_origBaseY", by);
            s.x = bx + s.num("_eeVisOffX").unwrap_or(0.0);
            s.y = by + s.num("_eeVisOffY").unwrap_or(0.0);
            s.set_num("_eeWorldX", bx); s.set_num("_eeBaseY", by);
            self.refresh_sprite_section(si);
        }
    }

    pub(crate) fn sprite_reset_group_scales(&mut self) {}

    /// the sprite half of `resetRotateTriggers`
    pub(crate) fn sprite_reset_rotate(&mut self) {
        if self.fx.sprites.is_empty() { return; }
        let mut seen = std::collections::BTreeSet::new();
        let order: Vec<usize> = self.fx.group_sprites.values().flatten().copied().filter(|s| seen.insert(*s)).collect();
        for si in order {
            let s = &mut self.fx.sprites[si];
            if !s.active { continue; }
            if let Some(ix) = s.num("_eeInitialWorldX") {
                s.x = ix + s.num("_eeVisOffX").unwrap_or(0.0);
                s.set_num("_eeWorldX", ix); s.set_num("_origWorldX", ix); s.set_num("_eeMoveBaseWorldX", ix);
            }
            if let Some(iy) = s.num("_eeInitialBaseY") {
                s.y = iy + s.num("_eeVisOffY").unwrap_or(0.0);
                s.set_num("_eeBaseY", iy); s.set_num("_origBaseY", iy); s.set_num("_eeMoveBaseBaseY", iy);
            }
            if let Some(r) = s.num("_eeInitialRotationRad") { s.rotation = r; }
            self.refresh_sprite_section(si);
        }
    }

    // ---- visibility, colour channels, pulses ----
    /// `updateVisibility`'s section range (cull distance 3)
    pub fn update_visibility(&mut self, camera_x: f64) {
        self.cull_emitters(camera_x);
        if self.fx.container_len == 0 { return; }
        let last = (self.fx.container_len - 1) as f64;
        let lo = 0f64.max(((camera_x - 200.0) / 400.0).floor());
        let hi = last.min(((camera_x + self.screen_width + 200.0) / 400.0).floor());
        let (old_lo, old_hi) = (self.fx.vis_min, self.fx.vis_max);
        let (lo, hi) = (lo as i64, hi as i64);
        let mut shown: Vec<i64> = Vec::new();
        if old_lo < 0 {
            shown.extend(lo..=hi);
        } else if lo != old_lo || hi != old_hi {
            if lo < old_lo { shown.extend(lo..=(old_lo - 1).min(hi)); }
            if hi > old_hi { shown.extend((old_hi + 1).max(lo)..=hi); }
        }
        self.fx.vis_min = lo;
        self.fx.vis_max = hi;
        for sec in shown { self.apply_stale_section(sec); }
    }

    fn apply_stale_section(&mut self, sec: i64) {
        if sec < 0 { return; }
        let Some(list) = self.fx.sections.get(&(sec as u64)) else { return };
        let stale: Vec<usize> = list.iter().copied().filter(|&si| self.fx.sprites[si].flag("_eeXformDirty")).collect();
        for si in stale { self.apply_sprite_transform(si); }
    }

    fn skip_far_sprite_transform(&mut self, si: usize) -> bool {
        let (lo, hi) = (self.fx.vis_min - 1, self.fx.vis_max + 1);
        let (x0, y0, cur) = {
            let s = &self.fx.sprites[si];
            if !s.active || s.flag("_beastManaged") || s.num("_coinWorldX").is_some() { return false; }
            let cur = match s.num("_eeSectionIndex") { Some(c) if c.is_finite() && c.fract() == 0.0 => c as i64, _ => return false };
            if cur >= lo && cur <= hi { return false; }
            (s.num("_eeInitialWorldX").or_else(|| s.num("_origWorldX")).unwrap_or(s.x), s.num("_eeInitialBaseY").or_else(|| s.num("_origBaseY")).unwrap_or(s.y), cur)
        };
        let ids = self.fx.ids[si].clone();
        let (mut x, mut y) = (x0, y0);
        for g in &ids { if let Some(off) = self.trig.offsets.get(g) { x += or0(off.0); y += or0(off.1); } }
        for g in &ids {
            let Some(rd) = self.trig.rotations.get(g).cloned() else { continue };
            if rd.total_rad == 0.0 || !(rd.center > 0) { continue; }
            let (cx, cy) = self.center_cached(rd.center);
            let (dx, dy) = (x - cx, y - cy);
            let (c, sn) = (crate::jsmath::cos(rd.total_rad), crate::jsmath::sin(rd.total_rad));
            x = cx + dx * c - dy * sn;
            y = cy + dx * sn + dy * c;
        }
        let sec = section_index(x) as i64;
        if sec >= lo && sec <= hi { return false; }
        let s = &mut self.fx.sprites[si];
        s.set_flag("_eeXformDirty", true);
        s.set_num("_eeWorldX", x);
        s.set_num("_eeBaseY", y);
        if sec != cur { self.refresh_sprite_section(si); }
        true
    }

    /// `_cullEmitters`
    fn cull_emitters(&mut self, camera_x: f64) {
        let (min_x, max_x) = (camera_x - 400.0, camera_x + self.screen_width + 400.0);
        for k in 0..self.fx.emitters.len() {
            let si = self.fx.emitters[k];
            let toggled_off = !self.trig.anchors[si].toggled.is_empty();
            let s = &mut self.fx.sprites[si];
            let x = s.num("_eeWorldX").unwrap_or(s.x);
            let want = !toggled_off && x >= min_x && x <= max_x;
            if s.active != want { s.active = want; }
        }
    }

    /// `anchorSprite.parentContainer.visible === false`
    pub fn sprite_container_hidden(&self, si: usize) -> bool { self.sprite_hidden(si) }
    fn sprite_hidden(&self, si: usize) -> bool {
        match self.fx.hide_key.get(si) {
            Some(&k) if k >= 0 => k < self.fx.vis_min || k > self.fx.vis_max,
            _ => false, // the top container is always shown; a sprite without a section is not culled
        }
    }

    /// `_applyChannelAlpha`
    fn apply_channel_alpha(&mut self, si: usize) {
        let s = &self.fx.sprites[si];
        if s.flag("_eeActive") || s.flag("_eeAudioScale") || s.flag("_beastManaged") { return; }
        let ch = match s.num("_eeColorChannel") { Some(c) if c != 0.0 && !c.is_nan() => c as i64, _ => return };
        let op = self.cm.get_opacity(ch);
        if op == 1.0 && !s.flag("_eeChannelAlphaApplied") { return; }
        self.fx.sprites[si].set_flag("_eeChannelAlphaApplied", true);
        let alpha = self.group_opacity_for_sprite(si);
        if self.fx.sprites[si].alpha != alpha { self.fx.sprites[si].alpha = alpha; }
    }

    /// `_ensureColorChannelIndex`: the channels to watch (ascending, each followed by the saw colours of its sprites)
    fn ensure_color_index(&mut self) {
        if self.fx.index_built { return; }
        self.fx.index_built = true;
        let mut watch: Vec<i64> = Vec::new();
        for (ch, list) in &self.fx.channel_sprites {
            if !watch.contains(ch) { watch.push(*ch); }
            for &si in list {
                if let Some(c) = self.fx.sprites[si].num("_SawColor") { let c = c as i64; if !watch.contains(&c) { watch.push(c); } }
            }
        }
        self.fx.watch = watch;
        self.fx.color_dirty = true;
    }

    /// `_colorsNeedApply`: returns `(dirty, full)`
    fn colors_need_apply(&mut self) -> (bool, bool) {
        let mut dirty = self.fx.color_dirty;
        if self.fx.color_applied != Some((self.fx.vis_min, self.fx.vis_max)) { dirty = true; }
        let mut full = dirty;
        self.fx.color_changed.clear();
        let watch = self.fx.watch.clone();
        for ch in watch {
            let hex = self.cm.get_hex(ch);
            let has = self.cm.has_color(ch);
            let op = self.cm.get_opacity(ch);
            let cur = (hex, has, op);
            let same = matches!(self.fx.color_last.get(&ch), Some(l) if l.0 == hex && l.1 == has && l.2 == op);
            if !same {
                self.fx.color_last.insert(ch, cur);
                dirty = true;
                self.fx.color_changed.push(ch);
            }
        }
        self.fx.color_frames_since += 1;
        if self.fx.color_frames_since >= 30 { dirty = true; full = true; }
        if self.fx.visible_by_channel.is_none() { full = true; }
        (dirty, full)
    }

    fn tint_sprite(&mut self, si: usize) {
        if !self.fx.sprites[si].active { return; }
        if self.sprite_hidden(si) { return; }
        let chans = self.fx.tint_channels[si].clone();
        for ch in chans {
            let hex = self.cm.get_hex(ch);
            if let Some(t) = self.fx.sprites[si].channel_tint(ch, hex, &self.cm) { self.fx.sprites[si].tint = Some(t); }
        }
        self.apply_channel_alpha(si);
    }

    /// `applyColorChannels(cm, onlyVisible = true)` (`_applyVisibleColorChannels`, with its dirty/partial-update cache)
    pub fn apply_color_channels(&mut self) {
        let (lo, hi) = (self.fx.vis_min, self.fx.vis_max);
        if lo < 0 || hi < lo { return; }
        self.ensure_color_index();
        let (dirty, full) = self.colors_need_apply();
        if !dirty { return; }
        self.fx.color_dirty = false;
        self.fx.color_frames_since = 0;
        self.fx.color_applied = Some((lo, hi));
        if !full {
            let by = self.fx.visible_by_channel.clone().unwrap_or_default();
            let changed = self.fx.color_changed.clone();
            let mut done = std::collections::BTreeSet::new();
            for ch in changed {
                let Some(list) = by.get(&ch) else { continue };
                for &si in list { if done.insert(si) { self.tint_sprite(si); } }
            }
            return;
        }
        let mut by: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
        for sec in lo..=hi {
            let list = match self.fx.sections.get(&(sec as u64)) { Some(l) => l.clone(), None => continue };
            for si in list {
                if self.fx.tint_channels[si].is_empty() || !self.fx.sprites[si].active { continue; }
                for &ch in &self.fx.tint_channels[si] { by.entry(ch).or_default().push(si); }
                if let Some(c) = self.fx.sprites[si].num("_SawColor") { by.entry(c as i64).or_default().push(si); }
                self.tint_sprite(si);
            }
        }
        self.fx.visible_by_channel = Some(by);
    }

    /// the sprite half of `stepPulseTriggers`: the tint of every sprite in a pulsed group (or its restoration)
    pub(crate) fn apply_pulse_ops(&mut self) {
        let ops = self.trig.group_ops.clone();
        if !ops.is_empty() { self.fx.color_dirty = true; }
        let mut base_by_channel: BTreeMap<i64, crate::colors::Rgb> = BTreeMap::new();
        for (gid, op) in ops {
            let list = match self.fx.group_sprites.get(&gid) { Some(l) => l.clone(), None => continue };
            for si in list {
                let s = &self.fx.sprites[si];
                if !s.active || s.flag("_cantColor") || s.flag("_isEmitter") { continue; }
                let ch = s.num("_eeColorChannel").filter(|c| *c != 0.0 && !c.is_nan()).map(|c| c as i64);
                match op {
                    None => {
                        if !s.flag("_eePulsed") { continue; }
                        self.fx.sprites[si].set_flag("_eePulsed", false);
                        if let Some(ch) = ch {
                            let hex = self.cm.get_hex(ch);
                            if let Some(t) = self.fx.sprites[si].channel_tint(ch, hex, &self.cm) { self.fx.sprites[si].tint = Some(t); }
                        } else if self.fx.sprites[si].flag("_isBlack") { self.fx.sprites[si].tint = Some(0.0); }
                        else { self.fx.sprites[si].tint = None; }
                    }
                    Some((ti, intensity)) => {
                        let base = match ch {
                            Some(ch) => *base_by_channel.entry(ch).or_insert_with(|| self.cm.get_color(ch)),
                            None => if s.flag("_isBlack") { crate::colors::Rgb { r: 0.0, g: 0.0, b: 0.0 } } else { crate::colors::Rgb { r: 255.0, g: 255.0, b: 255.0 } },
                        };
                        let t = &self.trig.l.pulses[ti];
                        let target = match t.hsv {
                            Some(h) => crate::colors::apply_channel_hsv(if t.copy_source > 0.0 { self.cm.get_color(t.copy_source as i64) } else { base }, Some(h)),
                            None => t.color,
                        };
                        let rd = |v: f64| crate::physics::js_round(v) as i64;
                        let (r, g, b) = (rd(base.r + (target.r - base.r) * intensity), rd(base.g + (target.g - base.g) * intensity), rd(base.b + (target.b - base.b) * intensity));
                        let hex = (((r as i32) << 16) | ((g as i32) << 8) | (b as i32)) as f64;
                        let s = &mut self.fx.sprites[si];
                        s.tint = Some(hex);
                        s.set_flag("_eePulsed", true);
                    }
                }
            }
        }
    }

    /// the sprite half of `resetPulseTriggers`
    pub(crate) fn sprite_reset_pulse(&mut self) {
        self.fx.color_dirty = true;
        // `resetObjects`: the music-scaled sprites go back to 0.1 (`gdApplyEffectScale(sprite, 0.1)`)
        for si in self.fx.lists.audio_scale.clone() {
            let s = &mut self.fx.sprites[si];
            let (nx, ny) = (0.1 * s.num("_eeBaseScaleX").unwrap_or(1.0), 0.1 * s.num("_eeBaseScaleY").unwrap_or(1.0));
            if s.scale_x != nx || s.scale_y != ny { s.scale_x = nx; s.scale_y = ny; }
        }
        for list in self.fx.group_sprites.values() {
            for &si in list {
                let s = &mut self.fx.sprites[si];
                if !s.flag("_eePulsed") { continue; }
                s.tint = None;
                s.set_flag("_eePulsed", false);
            }
        }
    }

    /// the sprite half of `setChannelBlending`
    pub(crate) fn sprite_channel_blending(&mut self, ch: i64, on: bool) {
        let list = match self.fx.channel_sprites.get(&ch) { Some(l) => l.clone(), None => return };
        for si in list {
            let s = &mut self.fx.sprites[si];
            s.set_flag("_eeChannelAdditive", on);
            s.blend = if on { 1 } else { s.num("_eeBaseBlend").unwrap_or(0.0) as i32 };
        }
    }

    // ---- alpha (group opacity) ----
    /// the sprite half of `stepAlphaTriggers`, run after the tweens advanced
    pub fn apply_alpha(&mut self, changed_by_tween: bool) {
        let mut changed = self.fx.alpha_force || self.fx.alpha_applied_gen != self.fx.opacity_gen || changed_by_tween;
        let glow = self.fx.glow_multiplier;
        if self.fx.alpha_applied_glow != Some(glow) { changed = true; }
        if !changed { return; }
        self.fx.alpha_force = false;
        self.fx.alpha_applied_gen = self.fx.opacity_gen;
        self.fx.alpha_applied_glow = Some(glow);
        let groups: Vec<(i64, f64)> = self.trig.group_opacity.iter().filter(|(k, _)| **k > 0).map(|(k, v)| (*k, *v)).collect();
        for (gid, op) in groups {
            let list = match self.fx.group_sprites.get(&gid) { Some(l) => l.clone(), None => continue };
            for si in list {
                let s = &mut self.fx.sprites[si];
                if !s.active || s.flag("_eeActive") { continue; }
                let base = s.num("_eeOrigAlpha").unwrap_or(1.0);
                let target = base * op * (if s.flag("_eeIsGlowSprite") { glow } else { 1.0 }) * s.num("_eeApproachAlpha").unwrap_or(1.0);
                s.set_alpha(target);
                if let Some(e) = s.num("_padEmitter") { let e = e as usize; self.fx.sprites[e].set_alpha(target); self.fx.sprites[e].visible = target > 0.01; }
            }
        }
    }

    /// the sprite half of `resetAlphaTriggers`
    pub(crate) fn sprite_reset_alpha(&mut self) {
        self.fx.alpha_force = true;
        self.fx.opacity_gen += 1;
        let glow = self.fx.glow_multiplier;
        let lists: Vec<Vec<usize>> = self.fx.group_sprites.values().cloned().collect();
        for list in lists {
            for si in list {
                let s = &mut self.fx.sprites[si];
                if !s.active || s.flag("_eeActive") { continue; }
                let base = s.num("_eeOrigAlpha").unwrap_or(1.0);
                let target = base * (if s.flag("_eeIsGlowSprite") { glow } else { 1.0 }) * s.num("_eeApproachAlpha").unwrap_or(1.0);
                s.set_alpha(target);
                if let Some(e) = s.num("_padEmitter") { let e = e as usize; self.fx.sprites[e].set_alpha(1.0); self.fx.sprites[e].visible = true; }
            }
        }
    }

    // ---- enter / exit effects ----
    /// `resetEnterEffectTriggers`, the sprite half
    pub(crate) fn sprite_reset_enter_effects(&mut self) {
        for si in 0..self.fx.sprites.len() {
            if self.fx.sprites[si].num("_eeSectionIndex").is_none() { continue; }
            self.fx.sprites[si].set_flag("_eeActive", false);
            let guide = self.guide_hidden(si);
            let editor_only = self.fx.sprites[si].flag("_eeEditorOnly");
            let beast = self.fx.sprites[si].flag("_beastManaged");
            self.fx.sprites[si].visible = !guide && !editor_only;
            if beast {
                let s = &mut self.fx.sprites[si];
                s.set_num("_eeEffectOffsetX", 0.0); s.set_num("_eeEffectOffsetY", 0.0); s.set_num("_eeEffectScale", 1.0);
                let a = self.group_opacity_for_sprite(si);
                self.fx.sprites[si].set_alpha(a);
                continue;
            }
            let (wx, by, ox, oy) = { let s = &self.fx.sprites[si]; (s.num("_eeWorldX").unwrap_or(f64::NAN), s.num("_eeBaseY").unwrap_or(f64::NAN), s.num("_eeVisOffX").unwrap_or(0.0), s.num("_eeVisOffY").unwrap_or(0.0)) };
            self.fx.sprites[si].x = wx + ox;
            self.fx.sprites[si].y = by + oy;
            if !self.fx.sprites[si].flag("_eeAudioScale") { self.set_effect_scale(si, 1.0); }
            let a = self.group_opacity_for_sprite(si);
            self.fx.sprites[si].set_alpha(a);
        }
    }

    /// `applyEnterEffects(cameraX)`
    pub fn apply_enter_effects(&mut self, camera_x: f64) {
        self.update_invisible_block_opacity();
        let (sec_w, fade, slide) = (400.0, 140.0, 200.0);
        let sw = self.screen_width;
        let (left, right, mid) = (camera_x, camera_x + sw, camera_x + sw / 2.0);
        let first = ((left - fade) / sec_w).floor().max(0.0);
        let last = ((self.fx.sections_len as f64) - 1.0).min(((right + fade) / sec_w).floor());
        let mut sec = first;
        while sec <= last {
            let secu = sec as u64;
            sec += 1.0;
            if !self.fx.sections.contains_key(&secu) { continue; }
            let sec_x = secu as f64 * sec_w;
            let interior = sec_x >= left + fade && sec_x + sec_w <= right - fade;
            let list = self.fx.sections[&secu].clone();
            for si in list {
                if interior {
                    if self.fx.sprites[si].flag("_eeActive") { self.leave_effect(si, true); }
                    continue;
                }
                let wx = self.fx.sprites[si].num("_eeWorldX").unwrap_or(f64::NAN);
                let entering = wx > mid;
                let k = if entering { 0f64.max(1f64.min((right - wx) / fade)) } else { 0f64.max(1f64.min((wx - left) / fade)) };
                let (dont_fade, dont_enter) = { let s = &self.fx.sprites[si]; (s.flag("_eeDontFade"), s.flag("_eeDontEnter")) };
                let fade_k = if dont_fade { 1.0 } else { k };
                let enter_k = if dont_enter { 1.0 } else { k };
                if fade_k >= 1.0 && enter_k >= 1.0 {
                    if self.fx.sprites[si].flag("_eeActive") { self.leave_effect(si, false); }
                    continue;
                }
                self.fx.sprites[si].set_flag("_eeActive", true);
                let effect = if entering { self.trig.active_enter } else { self.trig.active_exit };
                let t = 1.0 - enter_k;
                let (vis_x, vis_y, base_y, world_x, beast, audio) = { let s = &self.fx.sprites[si];
                    (s.num("_eeVisOffX").unwrap_or(0.0), s.num("_eeVisOffY").unwrap_or(0.0), s.num("_eeBaseY").unwrap_or(f64::NAN), s.num("_eeWorldX").unwrap_or(f64::NAN), s.flag("_beastManaged"), s.flag("_eeAudioScale")) };
                let mut ny = base_y + vis_y;
                let mut nx = world_x + vis_x;
                let mut a = fade_k;
                let mut scale = 1.0;
                match effect {
                    1 => ny = base_y + vis_y + slide * t,
                    2 => ny = base_y + vis_y - slide * t,
                    3 => nx = world_x + vis_x - slide * t,
                    4 => nx = world_x + vis_x + slide * t,
                    5 => { if !audio { scale = enter_k; } }
                    6 => { if !audio { scale = 1.0 + t * 0.75; } }
                    _ => {}
                }
                let _ = &mut a;
                if beast {
                    let s = &mut self.fx.sprites[si];
                    s.set_num("_eeEffectOffsetX", nx - world_x);
                    s.set_num("_eeEffectOffsetY", ny - base_y);
                    s.set_num("_eeEffectScale", scale);
                } else {
                    let s = &mut self.fx.sprites[si];
                    if s.x != nx { s.x = nx; }
                    if s.y != ny { s.y = ny; }
                    if !audio { self.set_effect_scale(si, scale); }
                }
                let go = self.group_opacity_for_sprite(si);
                let final_alpha = if a == 0.0 { 0.0 } else { a * go };
                let s = &mut self.fx.sprites[si];
                if s.alpha != final_alpha { s.alpha = final_alpha; }
            }
        }
    }

    /// `interior`: the sprite's whole section is well inside the screen (that branch only rescales when an effect had changed the scale)
    fn leave_effect(&mut self, si: usize, interior: bool) {
        self.fx.sprites[si].set_flag("_eeActive", false);
        if self.fx.sprites[si].flag("_beastManaged") {
            let s = &mut self.fx.sprites[si];
            s.set_num("_eeEffectOffsetX", 0.0); s.set_num("_eeEffectOffsetY", 0.0); s.set_num("_eeEffectScale", 1.0);
        } else {
            let (wx, by, ox, oy) = { let s = &self.fx.sprites[si]; (s.num("_eeWorldX").unwrap_or(f64::NAN), s.num("_eeBaseY").unwrap_or(f64::NAN), s.num("_eeVisOffX").unwrap_or(0.0), s.num("_eeVisOffY").unwrap_or(0.0)) };
            self.fx.sprites[si].y = by + oy;
            self.fx.sprites[si].x = wx + ox;
            let (audio, mult) = { let s = &self.fx.sprites[si]; (s.flag("_eeAudioScale"), s.num("_eeEffectMult")) };
            if !audio && (!interior || mult != Some(1.0)) { self.set_effect_scale(si, 1.0); }
        }
        let a = self.group_opacity_for_sprite(si);
        self.fx.sprites[si].set_alpha(a);
    }

    /// `_updateInvisibleBlockOpacity`
    fn update_invisible_block_opacity(&mut self) {
        if self.fx.invisible.is_empty() { return; }
        let px = self.fx.player_x;
        if !px.is_finite() { return; }
        let fade_distance = 1f64.max(self.screen_width / 2.5);
        let active_distance = fade_distance + 100.0;
        let (min_x, max_x) = (px - active_distance, px + active_distance);
        let active: Vec<usize> = self.fx.active_invisible.clone();
        for ei in active {
            let e = &self.fx.invisible[ei];
            if e.removed || e.x < min_x || e.x > max_x {
                self.set_invisible_opacity(ei, 0.0);
                self.fx.active_invisible.retain(|&v| v != ei);
            }
        }
        // entries are sorted by x in spawn order only if the level is; the original binary-searches, so mirror that exactly
        let n = self.fx.invisible.len();
        let (mut low, mut high) = (0usize, n);
        while low < high {
            let mid = (low + high) >> 1;
            if self.fx.invisible[mid].x < min_x { low = mid + 1 } else { high = mid }
        }
        let mut i = low;
        while i < n && self.fx.invisible[i].x <= max_x {
            if !self.fx.invisible[i].removed {
                let dist = (self.fx.invisible[i].x - px).abs();
                let opacity = 0f64.max(1f64.min(dist / fade_distance));
                if !self.fx.active_invisible.contains(&i) { self.fx.active_invisible.push(i); }
                self.set_invisible_opacity(i, opacity);
            }
            i += 1;
        }
    }

    fn set_invisible_opacity(&mut self, ei: usize, alpha: f64) {
        if (self.fx.invisible[ei].alpha - alpha).abs() < 0.001 { return; }
        self.fx.invisible[ei].alpha = alpha;
        let list = self.fx.invisible[ei].sprites.clone();
        for si in list {
            if !self.fx.sprites[si].active { continue; }
            self.fx.sprites[si].set_num("_eeApproachAlpha", alpha);
            let a = self.group_opacity_for_sprite(si);
            self.fx.sprites[si].set_alpha(a);
        }
    }
}

// ---- per-frame presentation: ground, background, and the buffer the renderer draws from ----
fn parse_fly(entry: &str) -> Option<(bool, f64, Option<f64>, Option<f64>)> {
    let inner = entry.strip_prefix("fly:[")?.strip_suffix(']')?;
    let parts: Vec<&str> = inner.split(',').collect();
    let enabled = parts.first()? == &"true";
    let y: f64 = parts.get(1)?.parse().ok()?;
    let h: Option<f64> = parts.get(2).and_then(|v| v.parse().ok());
    // with a height the log also has the (false) visual-only flag, then the camera height
    let cam: Option<f64> = parts.get(4).and_then(|v| v.parse().ok());
    Some((enabled, y, h, cam))
}

impl Game {
    /// Called once the host knows the textures: the ground tile width, whether there is a second ground layer, the background image height.
    pub fn render_setup(&mut self, ground_tile_w: f64, has_ground2: bool, bg_height: f64) {
        self.g.center_x = self.screen_width / 2.0 - 150.0; // config.js `centerX`
        self.fx.ground = Some(Ground::new(self.screen_width, if ground_tile_w > 0.0 { ground_tile_w } else { 1012.0 }));
        self.fx.has_ground2 = has_ground2;
        self.fx.bg = Background::new();
        self.fx.bg.apply_texture(bg_height);
        self.fx.bg.prev_camera_x = self.camera_x;
    }

    /// The `setFlyMode` calls the simulation logged, given to the ground the level layer draws.
    pub fn apply_fly_log(&mut self) {
        let ops: Vec<(bool, f64, Option<f64>, Option<f64>)> = self.log.iter().filter_map(|l| parse_fly(l)).collect();
        let cam_y = self.camera_y;
        let free = self.e.portal_free_mode; // `setFlyMode(true, ...)` does nothing while a free-mode portal is active
        if let Some(g) = self.fx.ground.as_mut() {
            for (en, y, h, cam) in ops {
                if en && free { continue; }
                let height = h.unwrap_or(600.0);
                g.set_fly_mode(en, y, height, false, cam.unwrap_or(height), cam_y);
            }
        }
    }

    /// The end of `update` for the sprite world: colours, enter effects, ground and background.
    pub fn render_step(&mut self, delta_ms: f64) {
        self.step_coin_tweens(delta_ms);
        if self.fx.sprites.is_empty() && self.fx.ground.is_none() { return; }
        self.fx.player_x = self.e.player_world_x;
        self.apply_color_channels();
        self.apply_enter_effects(self.camera_x);
        self.beast_update(delta_ms);
        let (cam_x, cam_y, sx, sy) = (self.camera_x, self.camera_y, self.trig.shake_off.0, self.trig.shake_off.1);
        let has2 = self.fx.has_ground2;
        self.apply_fly_log();
        if let Some(g) = self.fx.ground.as_mut() { g.step_animation(delta_ms / 1000.0); }
        let mp = self.mirror_progress();
        let shift = self.fx.bg.update(cam_x, cam_y, mp);
        let ground_cam = self.fx.slide.unwrap_or(cam_x); // during the slide-in the ground follows `_slideGroundX`
        if let Some(g) = self.fx.ground.as_mut() { if shift != 0.0 { g.shift(shift); } self.fx.ground_frame = g.update(ground_cam, cam_y, sx, sy, has2); }
    }

    pub fn render_restart(&mut self) {
        if let Some(g) = self.fx.ground.as_mut() { g.reset_state(); }
        self.fx.bg.tile_x = 0.0;
        self.fx.bg.prev_camera_x = self.camera_x;
    }

    /// `_spriteDepth`
    fn sprite_depth(&self, si: usize) -> f64 {
        let s = &self.fx.sprites[si];
        let z = s.num("_eeZDepth").unwrap_or(0.0);
        // beast parts have their depth set straight from `_eeZDepth` every frame
        if !s.flag("_eeChannelAdditive") || s.num("_beastPartOf").is_some() { return z; }
        match s.num("_eeLayerBase") { Some(base) => base - 0.5 + (z - base) * 0.24, None => z }
    }

    /// Everything the renderer needs for one frame, as flat numbers: a 40 slot header, the ground tile positions, then 12 per sprite
    /// (frame, x, y, rotation, scaleX, scaleY, alpha, tint or -1, blend, flip bits, top layer, reserved) in draw order.
    pub fn build_render(&mut self) -> usize {
        let mut order: Vec<(u8, f64, f64, usize)> = Vec::new();
        // the main menu hides the level's containers (game-scene.js: `container.setVisible(false)`): only background and ground show
        let hidden = self.fx.menu_mode && !self.fx.editor_mode;
        // the sprites that can be drawn: those of the visible sections plus the top layer (the editor shows everything)
        let cand: Vec<usize> = if self.fx.editor_mode { (0..self.fx.sprites.len()).filter(|&si| self.fx.ed_glow || !self.fx.sprites[si].flag("_eeIsGlowSprite")).collect() } else if hidden { Vec::new() } else {
            let mut c: Vec<usize> = Vec::new();
            if self.fx.vis_min >= 0 {
                for sec in self.fx.vis_min..=self.fx.vis_max {
                    if let Some(l) = self.fx.sections.get(&(sec as u64)) { c.extend(l.iter().copied().filter(|&si| self.fx.sprites[si].num("_eeLayer") != Some(2.0))); }
                }
            }
            c.extend(self.fx.top_sprites.iter().copied());
            c
        };
        for si in cand {
            let s = &self.fx.sprites[si];
            if !s.visible || s.alpha <= 0.0 || s.flag("_isEmitter") { continue; }
            let layer = s.num("_eeLayer").unwrap_or(1.0);
            let top = layer == 2.0;
            if !self.mirror_view(si).4 { continue; }
            if !top {
                let sec = match s.num("_eeSectionIndex") { Some(v) => v as i64, None => continue };
                if self.fx.editor_mode { order.push((0, self.sprite_depth(si), s.num("_eeSpawnOrder").unwrap_or(0.0), si)); continue; }
                if self.fx.vis_min < 0 || sec < self.fx.vis_min || sec > self.fx.vis_max { continue; }
            }
            order.push((top as u8, self.sprite_depth(si), s.num("_eeSpawnOrder").unwrap_or(0.0), si));
        }
        order.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)).then(a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)).then(a.3.cmp(&b.3))); // ties keep sprite order, as the stable sort over all sprites did
        let gf = self.fx.ground_frame.clone();
        let mut buf = std::mem::take(&mut self.fx.render_buf);
        buf.clear();
        buf.resize(40, 0.0);
        buf[0] = self.camera_x; buf[1] = self.camera_y; buf[2] = order.len() as f64;
        buf[3] = self.e.player_world_x; buf[4] = self.p.y; buf[5] = self.p.rotation; buf[6] = self.p.is_dead as u8 as f64;
        buf[7] = self.p.is_mini as u8 as f64; buf[8] = self.p.is_flying as u8 as f64; buf[9] = self.p.is_wave as u8 as f64; buf[10] = self.p.is_ball as u8 as f64;
        buf[11] = self.p.is_ufo as u8 as f64; buf[12] = self.p.is_spider as u8 as f64; buf[13] = self.p.is_robot as u8 as f64; buf[14] = self.p.gravity_flipped as u8 as f64;
        buf[15] = gf.ground_y; buf[16] = gf.ceiling_y; buf[17] = gf.ceiling_visible as u8 as f64; buf[18] = gf.tile_x.len() as f64; buf[19] = gf.line_x;
        buf[20] = self.fx.bg.tile_x; buf[21] = self.fx.bg.tile_y;
        buf[22] = self.cm.get_hex(1000); buf[23] = self.cm.get_hex(1001); buf[24] = self.cm.get_hex(1009);
        if self.fx.menu_mode { buf[22] = self.fx.menu_colors.0; buf[23] = self.fx.menu_colors.1; buf[24] = self.fx.menu_colors.1; }
        buf[25] = gf.ground2_visible as u8 as f64; buf[26] = gf.ceiling2_visible as u8 as f64;
        buf[27] = self.trig.player_hidden as u8 as f64; buf[28] = self.level_won as u8 as f64; buf[29] = self.cm.get_hex(1002);
        buf[30] = self.e.player_world_x / if self.end_x_pos > 0.0 { self.end_x_pos } else { 1.0 };
        buf[31] = self.trig.shake_off.0; buf[32] = self.trig.shake_off.1;
        if self.fx.editor_mode { buf[33] = self.fx.ed_cam.2; buf[34] = self.fx.ed_cam.0; buf[35] = self.fx.ed_cam.1; }
        else {
            let z = self.trig.gen.zoom;
            buf[38] = if z == 0.0 || z.is_nan() { 1.0 } else { z };
            buf[39] = if self.trig.gen.angle.is_nan() { 0.0 } else { self.trig.gen.angle };
        }
        buf[36] = self.mirror_progress(); buf[37] = self.p.mirrored as u8 as f64;
        buf.extend_from_slice(&gf.tile_x);
        for (top, _, _, si) in order {
            let s = &self.fx.sprites[si];
            let (mx, msx, mrot, malpha, _) = self.mirror_view(si);
            let edt = if self.fx.editor_mode { self.fx.ed_tint.get(si).copied().unwrap_or(-1.0) } else { -1.0 };
            buf.extend_from_slice(&[
                self.fx.sprite_frame[si] as f64, mx, s.y, mrot, msx, s.scale_y, if self.fx.editor_mode && self.fx.ed_dim.get(si).copied().unwrap_or(false) { malpha * 0.25 } else { malpha },
                if edt >= 0.0 { edt } else { s.tint.unwrap_or(-1.0) }, s.blend as f64, (s.flip_x as u8 | ((s.flip_y as u8) << 1)) as f64, top as f64, 0.0,
            ]);
        }
        let n = buf.len();
        self.fx.render_buf = buf;
        n
    }
}

// ---- per-frame sprite animation (game-scene.js `update`: texture swaps, saw spin, orb guides; level.js `updateAudioScale`) ----
impl Game {
    pub fn animate_sprites(&mut self, delta_ms: f64) {
        self.fx.anim_timer += delta_ms;
        if self.ldm { return; }
        let timer = self.fx.anim_timer;
        let animated = self.fx.lists.animated.clone();
        for si in animated {
            let s = &self.fx.sprites[si];
            if !s.active || !s.visible || self.sprite_hidden(si) { continue; }
            let last = match s.num("_lastAnimSwap") { Some(v) if v != 0.0 && !v.is_nan() => v, _ => 0.0 };
            let interval = s.num("_animInterval").unwrap_or(f64::NAN);
            let elapsed = timer - last;
            if elapsed >= interval {
                let len = match s.props.get("_animFrames") { Some(Val::List(l)) => l.len(), _ => 0 };
                if len == 0 { continue; }
                let idx = ((s.num("_animIdx").unwrap_or(0.0) as usize) + 1) % len;
                let s = &mut self.fx.sprites[si];
                s.set_num("_lastAnimSwap", timer - (elapsed % interval));
                s.set_num("_animIdx", idx as f64);
                if let Some(Some(fid)) = self.fx.anim_frames.get(&si).and_then(|v| v.get(idx)) {
                    let (a, fr) = self.fx.frames[*fid as usize].clone();
                    let s = &mut self.fx.sprites[si];
                    s.atlas = a; s.frame = fr;
                    self.fx.sprite_frame[si] = *fid;
                }
            }
        }
        let saw_t = timer / 1000.0;
        let saws = self.fx.lists.saws.clone();
        for si in saws {
            let s = &self.fx.sprites[si];
            if !s.active || !s.visible || self.sprite_hidden(si) { continue; }
            let mut speed = s.num("_Sawrotationspeed").unwrap_or(0.0034);
            if let Some(r1) = s.num("_SawRandom1") {
                let r2 = match s.num("_SawRandom2") { Some(v) if v != 0.0 && !v.is_nan() => v, _ => 0.0 };
                speed += crate::jsmath::sin(saw_t + r1) * r2;
            }
            self.fx.sprites[si].rotation += delta_ms * speed;
        }
        let grav_rot = if self.p.gravity_flipped { std::f64::consts::PI } else { 0.0 };
        let orbs = self.fx.lists.orbs.clone();
        for si in orbs {
            let s = &mut self.fx.sprites[si];
            if !s.active || !s.flag("_eeOrbGuide") || !s.flag("_OrbGuideGrav") { continue; }
            let base = match s.num("_OrbGuideRotation") { Some(v) if v.is_finite() => v, _ => 0.0 };
            s.rotation = base + grav_rot;
        }
        if self.fx.meter_live { let (pk, vol) = self.fx.audio_peak; self.fx.meter.update(delta_ms / 1000.0, pk, vol); self.fx.audio_meter = self.fx.meter.value; }
        self.update_audio_scale(self.fx.audio_meter, self.camera_x, self.screen_width);
    }

    /// `_markActivatedOrbSprites` (player.js): orb sprites at the collider's position start their click pulse
    pub fn mark_orb_sprites(&mut self, x: f64, y: f64) {
        let target_y = 460.0 - y;
        let now = self.trig.now_ms;
        for si in self.fx.lists.orbs.clone() {
            let s = &mut self.fx.sprites[si];
            let same = match s.num("_eeWorldX") { Some(wx) => (wx - x).abs() < 10.0 && s.num("_eeBaseY").map_or(true, |by| (by - target_y).abs() < 10.0), None => false };
            if same { s.set_num("_hitTime", now); }
        }
    }

    /// `updateAudioScale`
    pub fn update_audio_scale(&mut self, meter: f64, camera_x: f64, viewport: f64) {
        let meter = if meter.is_finite() { meter } else { 0.0 };
        let max_scale: f64 = 1.5;
        let target = 1.2f64.min(0f64.max(meter));
        let smooth = match self.fx.pulse_meter { None => target, Some(pm) => pm + (target - pm) * (if target > pm { 0.5 } else { 0.3 }) };
        self.fx.pulse_meter = Some(smooth);
        let tas = (max_scale.min(0f64.max(smooth * 0.8)) * 1000.0).round() / 1000.0;
        let (min_x, max_x) = (camera_x - 200.0, camera_x + viewport + 200.0);
        let off = |s: &Sprite| match s.num("_eeWorldX") { Some(wx) if wx.is_finite() => wx < min_x || wx > max_x, _ => false };
        for si in self.fx.lists.audio_scale.clone() {
            let s = &mut self.fx.sprites[si];
            if !s.active || off(s) { continue; }
            if s.num("_eeLastAudioScale") != Some(tas) {
                let (nx, ny) = (tas * s.num("_eeBaseScaleX").unwrap_or(1.0), tas * s.num("_eeBaseScaleY").unwrap_or(1.0));
                if s.scale_x != nx || s.scale_y != ny { s.scale_x = nx; s.scale_y = ny; }
                s.set_num("_eeLastAudioScale", tas);
            }
        }
        let pulse = ((0.82 + smooth * 0.25) * 1000.0).round() / 1000.0;
        for si in self.fx.lists.audio_pulse.clone() {
            let s = &mut self.fx.sprites[si];
            if !s.active || off(s) { continue; }
            if s.num("_eePulseMult") == Some(pulse) { continue; }
            s.set_num("_eePulseMult", pulse);
            let effect = s.num("_eeEffectMult").unwrap_or(1.0);
            s.scale_x = s.num("_eeObjScaleX").unwrap_or(1.0) * effect * pulse;
            s.scale_y = s.num("_eeObjScaleY").unwrap_or(1.0) * effect * pulse;
        }
        let pulse_mult = 2f64.min(1.0 + smooth * 0.2);
        for si in self.fx.lists.orbs.clone() {
            let s = &mut self.fx.sprites[si];
            if !s.active || off(s) { continue; }
            let base = match s.num("_eeBaseScale") { Some(v) if v > 0.0 => v, _ => 0.75 };
            let base_scale = max_scale.min(base * pulse_mult);
            let mut target = base_scale;
            if let Some(ht) = s.num("_hitTime").filter(|t| *t != 0.0) {
                let (click, shrink) = (2.0, 250.0);
                let elapsed = self.trig.now_ms - ht;
                if elapsed < 80.0 { target = base_scale + base_scale * (click - 1.0) * (elapsed / 80.0); }
                else if elapsed < 80.0 + shrink { let peak = base_scale * click; target = peak + (base_scale - peak) * ((elapsed - 80.0) / shrink); }
                else { s.set_num("_hitTime", 0.0); }
            }
            if s.num("_eeLastOrbScale") != Some(target) {
                s.scale_x = target; s.scale_y = target;
                s.set_num("_eeLastOrbScale", target);
            }
        }
    }
}

/// Phaser's `HSVToRGB(h, s, v).color`
fn phaser_hsv(h: f64, s: f64, v: f64) -> f64 {
    let (r, g, b);
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s); let q = v * (1.0 - f * s); let t = v * (1.0 - (1.0 - f) * s);
    match (i as i64).rem_euclid(6) { 0 => { r = v; g = t; b = p; } 1 => { r = q; g = v; b = p; } 2 => { r = p; g = v; b = t; } 3 => { r = p; g = q; b = v; } 4 => { r = t; g = p; b = v; } _ => { r = v; g = p; b = q; } }
    (((r * 255.0).floor() as i64) << 16 | ((g * 255.0).floor() as i64) << 8 | (b * 255.0).floor() as i64) as f64
}

impl Game {
    /// The level running behind the main menu (game-scene.js `update`, menu branch).
    pub fn menu_step(&mut self, delta_ms: f64) {
        let menu_delta = (delta_ms / 1000.0 * 60.0).min(2.0);
        let player_speed = 11.540004; // ONE_TIMES (the menu never changes it)
        self.fx.menu_camera_x += menu_delta * player_speed * 0.9 * 0.85;
        let cam = self.fx.menu_camera_x;
        self.camera_x = cam;
        self.fx.bg.update(cam, self.camera_y, 0.0);
        self.update_visibility(cam);
        self.fx.player_x = cam + self.g.center_x;
        self.apply_color_channels();
        self.apply_enter_effects(cam);
        let has2 = self.fx.has_ground2;
        let cy = self.camera_y;
        if let Some(g) = self.fx.ground.as_mut() { g.step_animation(delta_ms / 1000.0); self.fx.ground_frame = g.update(cam, cy, 0.0, 0.0, has2); }
        self.fx.menu_rainbow += delta_ms / 1000.0;
        let hue = (self.fx.menu_rainbow * 15.0) % 360.0;
        let c = phaser_hsv(hue / 360.0, 0.85, 1.0);
        self.fx.menu_colors = (c, c);
    }
}

// ---- level editor view ----
impl Game {
    /// One editor frame of the sprite world: everything visible, camera from the editor (`cam_y` is Phaser's `_cameraY`), colours applied.
    pub fn editor_view(&mut self, cam_x: f64, cam_y: f64, zoom: f64) {
        self.fx.editor_mode = true;
        self.fx.ed_cam = (cam_x, cam_y, zoom);
        self.camera_x = cam_x; self.camera_y = cam_y;
        self.fx.vis_min = 0; self.fx.vis_max = self.fx.sections.keys().next_back().map_or(0, |&k| k as i64); // every section (the colour update walks this range)
        self.apply_color_channels();
        self.fx.bg.tile_x = cam_x * 0.1;
        self.fx.bg.tile_y = self.fx.bg.init_y + cam_y * 0.1;
        if self.fx.bg.mirror_tile_h > 0.0 { let h = self.fx.bg.mirror_tile_h; self.fx.bg.tile_y = ((self.fx.bg.tile_y % h) + h) % h; }
    }

    /// Selected objects are tinted; `sel` are object indexes.
    /// the objects the layer filter dims (`_applyEditorLayerFilter`)
    pub fn editor_set_dim(&mut self, dim: &[usize]) {
        self.fx.ed_dim = vec![false; self.fx.sprites.len()];
        for &o in dim { if let Some(&(a, b)) = self.fx.obj_ranges.get(o) { for si in a..b { if let Some(t) = self.fx.ed_dim.get_mut(si as usize) { *t = true; } } } }
    }

    pub fn editor_set_selection(&mut self, sel: &[usize], tint: f64) {
        self.fx.ed_tint = vec![-1.0; self.fx.sprites.len()];
        for &o in sel { if let Some(&(a, b)) = self.fx.obj_ranges.get(o) { for si in a..b { if let Some(t) = self.fx.ed_tint.get_mut(si as usize) { *t = tint; } } } }
    }
}

impl Game {
    /// The texture frames of both looks of every coin sprite (`names[si] = (uncollected, collected)`), resolved by the host side.
    pub fn attach_coin_frames(&mut self, names: Vec<(usize, Vec<Option<(String, String)>>, Vec<Option<(String, String)>>)>) {
        for (si, a, b) in names {
            for (collected, list) in [(false, a), (true, b)] {
                let ids: Vec<Option<u32>> = list.into_iter().map(|n| n.map(|key| match self.fx.frames.iter().position(|f| *f == key) { Some(p) => p as u32, None => { self.fx.frames.push(key); (self.fx.frames.len() - 1) as u32 } })).collect();
                self.fx.coin_frames.insert((si, collected), ids);
            }
        }
    }
}

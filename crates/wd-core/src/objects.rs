//! Object definitions (from assets/scripts/game/allObjects.js) and the slope tables from level.js.
pub use crate::objects_data::OBJECTS;
pub use crate::slopes_data::{SLOPES, SLOPE_DECO_RANGES};

#[derive(Debug, Clone, Copy)]
pub struct ObjectDef {
    pub ty: Option<&'static str>,
    pub grid_w: Option<f64>,
    pub grid_h: Option<f64>,
    pub sprite_w: Option<f64>,
    pub sprite_h: Option<f64>,
    pub hitbox_scale_x: Option<f64>,
    pub hitbox_scale_y: Option<f64>,
    pub hitbox_radius: Option<f64>,
    pub sub: Option<&'static str>,
    pub portal_particle_color: Option<f64>,
}

/// `getObjectFromId`.
pub fn object_from_id(id: i64) -> Option<&'static ObjectDef> {
    if id < 0 { return None; }
    OBJECTS.binary_search_by_key(&(id as u32), |e| e.0).ok().map(|i| &OBJECTS[i].1)
}

pub fn is_trigger_branch(id: i64) -> bool {
    matches!(object_from_id(id), Some(d) if d.ty == Some("trigger")) && !matches!(id, 1813 | 2069 | 3645)
}

#[derive(Debug, Clone, Copy)]
pub struct SlopeData { pub gw: f64, pub gh: f64, pub angle: f64, pub sq: bool }

pub fn slope_data(id: i64) -> Option<SlopeData> {
    if id < 0 { return None; }
    SLOPES.binary_search_by_key(&(id as u32), |e| e.0).ok().map(|i| {
        let e = SLOPES[i];
        SlopeData { gw: e.1, gh: e.2, angle: e.3, sq: e.4 }
    })
}

pub fn is_decoration_slope_id(id: i64) -> bool {
    SLOPE_DECO_RANGES.iter().any(|&(lo, hi)| id >= lo as i64 && id <= hi as i64)
}

//! DESIGN.md primitives shared by the native journal and text input.
//! Saans is not bundled; Arial is the documented system sans fallback.
pub const CANVAS: u32 = 0xffffff;
pub const INK: u32 = 0x141414;
pub const MUTED: u32 = 0x707070;
pub const FAINT: u32 = 0xadadad;
pub const FIELD: u32 = 0xf0f0f0;
pub const SOFT: u32 = 0xf3f3f3;
pub const HAIRLINE: u32 = 0xe0e0e0;
pub const HAIRLINE_SOFT: u32 = 0xf0f0f0;
pub const FONT_FAMILY: &str = "Arial";
pub const HEADING_WEIGHT: f32 = 652.;
pub const BODY_WEIGHT: f32 = 456.;
pub const LEAD_WEIGHT: f32 = 300.;
pub const CONTROL_HEIGHT: f32 = 44.;
pub const INPUT_RADIUS: f32 = 16.;
pub const CARD_RADIUS: f32 = 24.;
pub const MIN_WINDOW_WIDTH: f32 = 600.;
pub const COMPACT_BREAKPOINT: f32 = 768.;
pub const SIDEBAR_BREAKPOINT: f32 = 1024.;
pub const ASIDE_BREAKPOINT: f32 = 1180.;

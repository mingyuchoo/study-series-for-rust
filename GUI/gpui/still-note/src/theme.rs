//! DESIGN.md primitives shared by the native journal and text input.
pub const CANVAS: u32 = 0x0a0a0a;
pub const INK: u32 = 0xffffff;
pub const BODY: u32 = 0xcccccc;
pub const BODY_STRONG: u32 = 0xe6e6e6;
pub const MUTED: u32 = 0x888888;
pub const FAINT: u32 = 0x5a5a5a;
pub const FIELD: u32 = 0x1a1a1a;
pub const CARD: u32 = FIELD;
pub const SOFT: u32 = 0x121212;
pub const ELEVATED: u32 = 0x242424;
pub const HAIRLINE: u32 = 0x2a2a2a;
pub const HAIRLINE_SOFT: u32 = HAIRLINE_STRONG;
pub const HAIRLINE_STRONG: u32 = 0x3a3a3a;
pub const PRIMARY: u32 = 0xfaff69;
pub const PRIMARY_ACTIVE: u32 = 0xe6eb52;
pub const PRIMARY_DISABLED: u32 = 0x3a3a1f;
pub const ON_PRIMARY: u32 = CANVAS;
pub const FONT_FAMILY: &str = "Pretendard";
pub const CODE_FONT_FAMILY: &str = FONT_FAMILY;
pub const HEADING_WEIGHT: f32 = 700.;
pub const BODY_WEIGHT: f32 = 400.;
pub const LEAD_WEIGHT: f32 = BODY_WEIGHT;
pub const CONTROL_WEIGHT: f32 = 600.;
pub const NAV_WEIGHT: f32 = 500.;
pub const STAT_SIZE: f32 = 56.;
pub const STAT_WEIGHT: f32 = 700.;
pub const HEADING_TRACKING: f32 = -1.;
pub const STAT_TRACKING: f32 = -1.5;
pub const CONTROL_HEIGHT: f32 = 40.;
pub const CONTROL_RADIUS: f32 = 8.;
pub const INPUT_RADIUS: f32 = CONTROL_RADIUS;
pub const CARD_RADIUS: f32 = 12.;
pub const NAV_HEIGHT: f32 = 64.;
pub const MIN_WINDOW_WIDTH: f32 = 600.;
pub const MIN_WINDOW_HEIGHT: f32 = 400.;
pub const COMPACT_BREAKPOINT: f32 = 768.;
pub const SIDEBAR_BREAKPOINT: f32 = 1024.;
pub const ASIDE_BREAKPOINT: f32 = 1180.;

/// Register full Korean Pretendard 1.3.9 under the SIL Open Font License.
/// Static weights make the requested 400/500/600/700 faces explicit on every
/// native text backend, without depending on system-installed fonts or a CDN.
pub fn register_fonts(cx: &gpui::App) {
    cx.text_system()
        .add_fonts(vec![
            std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Pretendard-Regular.otf")),
            std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Pretendard-Medium.otf")),
            std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Pretendard-SemiBold.otf")),
            std::borrow::Cow::Borrowed(include_bytes!("../assets/fonts/Pretendard-Bold.otf")),
        ])
        .expect("Unable to register bundled Pretendard fonts");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub canvas: u32,
    pub ink: u32,
    pub body: u32,
    pub body_strong: u32,
    pub muted: u32,
    pub faint: u32,
    pub field: u32,
    pub card: u32,
    pub soft: u32,
    pub elevated: u32,
    pub hairline: u32,
    pub hairline_soft: u32,
    pub hairline_strong: u32,
    pub primary: u32,
    pub primary_active: u32,
    pub primary_disabled: u32,
    pub on_primary: u32,
    pub selection: u32,
}
impl Palette {
    pub const DARK: Self = Self {
        canvas: CANVAS,
        ink: INK,
        body: BODY,
        body_strong: BODY_STRONG,
        muted: MUTED,
        faint: FAINT,
        field: FIELD,
        card: CARD,
        soft: SOFT,
        elevated: ELEVATED,
        hairline: HAIRLINE,
        hairline_soft: HAIRLINE_SOFT,
        hairline_strong: HAIRLINE_STRONG,
        primary: PRIMARY,
        primary_active: PRIMARY_ACTIVE,
        primary_disabled: PRIMARY_DISABLED,
        on_primary: ON_PRIMARY,
        selection: 0xfaff6940,
    };
    pub const LIGHT: Self = Self {
        canvas: 0xf7f7f5,
        ink: 0x191919,
        body: 0x333333,
        body_strong: 0x242424,
        muted: 0x595959,
        faint: 0x606060,
        field: 0xffffff,
        card: 0xffffff,
        soft: 0xebebe7,
        elevated: 0xe1e1db,
        hairline: 0xb5b5ad,
        hairline_soft: 0xb5b5ad,
        hairline_strong: 0x898980,
        primary: 0x555a00,
        primary_active: 0x454a00,
        primary_disabled: 0xd0d0c0,
        on_primary: 0xffffff,
        selection: 0x555a0040,
    };
    pub fn for_theme(mode: crate::settings::ThemeMode) -> Self {
        if mode == crate::settings::ThemeMode::Light { Self::LIGHT } else { Self::DARK }
    }
}

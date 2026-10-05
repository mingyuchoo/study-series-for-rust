//! DESIGN.md("Cal.com-design-analysis") 디자인 토큰을 iced 스타일로 옮긴 모듈.
//!
//! 화이트 캔버스(canvas) 위에 검정색 프라이머리 CTA(primary #111111)와
//! 라이트 그레이 카드(surface-card #f5f5f5), 파스텔 뱃지/아바타 액센트,
//! 그리고 화면 하단을 닫아주는 다크 푸터(surface-dark #101010)로 구성된 모던
//! SaaS 디자인 시스템이다.
//!
//! 라이트/다크 두 가지 [`Palette`] 를 제공하며, 모든 스타일 함수는 전달받은
//! [`Theme`] 의 명암(`is_dark`)에 따라 알맞은 팔레트를 선택한다. 따라서
//! [`ThemeMode::System`] 일 때 iced 가 OS 테마를 따라 `Theme` 을 바꾸면 화면도
//! 자동으로 따라간다.

#![allow(dead_code)]

use crate::i18n::Strings;
use iced::{Background,
           Border,
           Color,
           Shadow,
           Theme,
           Vector,
           font::{Family,
                  Stretch,
                  Style as FontStyle,
                  Weight},
           widget::{button,
                    container,
                    text,
                    text_input}};

// ───────────────────────── Colors ─────────────────────────

/// u8 RGB 를 불투명 [`Color`] 로 변환한다(const 문맥에서 사용 가능).
const fn rgb(r: u8, g: u8, b: u8) -> Color { Color::from_rgba8(r, g, b, 1.0) }

// Brand & Action
pub const PRIMARY: Color = rgb(0x11, 0x11, 0x11);
pub const PRIMARY_ACTIVE: Color = rgb(0x24, 0x24, 0x24);
pub const PRIMARY_DISABLED: Color = rgb(0xe5, 0xe7, 0xeb);
pub const BRAND_ACCENT: Color = rgb(0x3b, 0x82, 0xf6);

// Surface
pub const CANVAS: Color = rgb(0xff, 0xff, 0xff);
pub const SURFACE_SOFT: Color = rgb(0xf8, 0xf9, 0xfa);
pub const SURFACE_CARD: Color = rgb(0xf5, 0xf5, 0xf5);
pub const SURFACE_STRONG: Color = rgb(0xe5, 0xe7, 0xeb);
pub const SURFACE_DARK: Color = rgb(0x10, 0x10, 0x10);
pub const SURFACE_DARK_ELEVATED: Color = rgb(0x1a, 0x1a, 0x1a);

// Borders & Lines
pub const HAIRLINE: Color = rgb(0xe5, 0xe7, 0xeb);
pub const HAIRLINE_SOFT: Color = rgb(0xf3, 0xf4, 0xf6);

// Text & Content
pub const INK: Color = rgb(0x11, 0x11, 0x11);
pub const BODY: Color = rgb(0x37, 0x41, 0x51);
pub const MUTED: Color = rgb(0x6b, 0x72, 0x80);
pub const MUTED_SOFT: Color = rgb(0x89, 0x89, 0x89);
pub const ON_PRIMARY: Color = rgb(0xff, 0xff, 0xff);
pub const ON_DARK: Color = rgb(0xff, 0xff, 0xff);
pub const ON_DARK_SOFT: Color = rgb(0xa1, 0xa1, 0xaa);

// Semantic
pub const SUCCESS: Color = rgb(0x10, 0xb9, 0x81);
pub const WARNING: Color = rgb(0xf5, 0x9e, 0x0b);
pub const ERROR: Color = rgb(0xef, 0x44, 0x44);

// Badge & Avatar Pastels
pub const BADGE_ORANGE: Color = rgb(0xfb, 0x92, 0x3c);
pub const BADGE_PINK: Color = rgb(0xec, 0x48, 0x99);
pub const BADGE_VIOLET: Color = rgb(0x8b, 0x5c, 0xf6);
pub const BADGE_EMERALD: Color = rgb(0x34, 0xd3, 0x99);

// Interaction & Banner tints
pub const SELECTION: Color = Color::from_rgba8(0x11, 0x11, 0x11, 0.15);
pub const ERROR_SOFT: Color = Color::from_rgba8(0xef, 0x44, 0x44, 0.08);
pub const ERROR_BORDER: Color = Color::from_rgba8(0xef, 0x44, 0x44, 0.25);

// Compatibility aliases
pub const SURFACE: Color = CANVAS;
pub const INK_SECONDARY: Color = BODY;
pub const INK_MUTED: Color = MUTED;
pub const INK_FAINT: Color = MUTED_SOFT;
pub const ACCENT_ORANGE_DEEP: Color = ERROR;

// ───────────────────────── Palettes (Light / Dark) ─────────────────────────

/// 테마 명암에 따라 달라지는 색상 토큰 묶음.
///
/// 시맨틱(에러) 색과 파스텔 뱃지 색, 다크 푸터 텍스트처럼 명암과 무관한 토큰은
/// 위의 상수를 그대로 사용한다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub primary: Color,
    pub primary_active: Color,
    pub primary_disabled: Color,
    pub on_primary: Color,
    pub canvas: Color,
    pub surface_soft: Color,
    pub surface_card: Color,
    pub surface_strong: Color,
    pub hairline: Color,
    pub hairline_soft: Color,
    pub ink: Color,
    pub body: Color,
    pub muted: Color,
    pub muted_soft: Color,
    /// nav-pill-group 래퍼 배경.
    pub pill_group: Color,
    /// nav-pill-group 안에서 활성 category-tab 배경.
    pub tab_active: Color,
    /// 다크 푸터 배경.
    pub footer: Color,
    /// 다크 푸터 테두리(다크 테마에서 캔버스와 구분하기 위함).
    pub footer_border: Color,
    pub selection: Color,
}

/// DESIGN.md 원본 토큰 그대로의 라이트 팔레트.
pub const LIGHT: Palette = Palette {
    primary: PRIMARY,
    primary_active: PRIMARY_ACTIVE,
    primary_disabled: PRIMARY_DISABLED,
    on_primary: ON_PRIMARY,
    canvas: CANVAS,
    surface_soft: SURFACE_SOFT,
    surface_card: SURFACE_CARD,
    surface_strong: SURFACE_STRONG,
    hairline: HAIRLINE,
    hairline_soft: HAIRLINE_SOFT,
    ink: INK,
    body: BODY,
    muted: MUTED,
    muted_soft: MUTED_SOFT,
    pill_group: SURFACE_SOFT,
    tab_active: CANVAS,
    footer: SURFACE_DARK,
    footer_border: Color::TRANSPARENT,
    selection: SELECTION,
};

/// DESIGN.md 의 surface-dark / on-dark 토큰을 바탕으로 반전한 다크 팔레트.
///
/// 모노크롬 액션 레이어 원칙을 유지하기 위해 primary CTA 는 흰색 표면 + 잉크
/// 텍스트로 반전한다.
pub const DARK: Palette = Palette {
    primary: ON_DARK,
    primary_active: SURFACE_STRONG,
    primary_disabled: rgb(0x2a, 0x2a, 0x2a),
    on_primary: PRIMARY,
    canvas: SURFACE_DARK,
    surface_soft: rgb(0x16, 0x16, 0x16),
    surface_card: SURFACE_DARK_ELEVATED,
    surface_strong: rgb(0x2a, 0x2a, 0x2a),
    hairline: rgb(0x2a, 0x2a, 0x2a),
    hairline_soft: rgb(0x1f, 0x1f, 0x1f),
    ink: ON_DARK,
    body: rgb(0xd4, 0xd4, 0xd8),
    muted: ON_DARK_SOFT,
    muted_soft: rgb(0x71, 0x71, 0x7a),
    pill_group: SURFACE_DARK_ELEVATED,
    tab_active: rgb(0x2e, 0x2e, 0x2e),
    footer: rgb(0x00, 0x00, 0x00),
    footer_border: rgb(0x2a, 0x2a, 0x2a),
    selection: Color::from_rgba8(0xff, 0xff, 0xff, 0.2),
};

/// 현재 iced [`Theme`] 의 명암에 맞는 팔레트를 고른다.
pub fn palette(theme: &Theme) -> &'static Palette { if theme.extended_palette().is_dark { &DARK } else { &LIGHT } }

// ───────────────────────── Theme mode ─────────────────────────

/// 사용자가 고른 테마 모드.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// OS 의 라이트/다크 설정을 따른다.
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    /// 토글 버튼에 나열할 순서.
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];

    /// iced 에 넘길 테마. `None` 이면 iced 가 시스템 테마를 사용하고
    /// OS 설정이 바뀔 때도 자동으로 갱신한다.
    pub fn to_theme(self) -> Option<Theme> {
        match self {
            | ThemeMode::System => None,
            | ThemeMode::Light => Some(Theme::Light),
            | ThemeMode::Dark => Some(Theme::Dark),
        }
    }

    /// 현재 언어의 토글 라벨.
    pub fn label(self, strings: &Strings) -> &'static str {
        match self {
            | ThemeMode::System => strings.theme_system,
            | ThemeMode::Light => strings.theme_light,
            | ThemeMode::Dark => strings.theme_dark,
        }
    }
}

// ───────────────────────── Typography ─────────────────────────

pub const DISPLAY_XL: f32 = 64.0;
pub const DISPLAY_LG: f32 = 48.0;
pub const DISPLAY_MD: f32 = 36.0;
pub const DISPLAY_SM: f32 = 28.0;
pub const TITLE_LG: f32 = 22.0;
pub const TITLE_MD: f32 = 18.0;
pub const TITLE_SM: f32 = 16.0;
pub const BODY_MD: f32 = 16.0;
pub const BODY_SM: f32 = 14.0;
pub const CAPTION: f32 = 13.0;
pub const CODE: f32 = 14.0;
pub const BUTTON_TEXT: f32 = 14.0;
pub const NAV_LINK: f32 = 14.0;

// Legacy typography aliases
pub const HEADING_1: f32 = DISPLAY_SM;
pub const HEADING_2: f32 = TITLE_LG;
pub const TITLE: f32 = TITLE_MD;
pub const EYEBROW: f32 = CAPTION;

/// 디자인 시스템 폰트 패밀리(Cal Sans 대안 Inter / Noto Sans KR).
const fn font(weight: Weight) -> iced::Font {
    iced::Font {
        family: Family::SansSerif,
        weight,
        stretch: Stretch::Normal,
        style: FontStyle::Normal,
    }
}

pub const REGULAR: iced::Font = font(Weight::Normal);
pub const MEDIUM: iced::Font = font(Weight::Medium);
pub const SEMIBOLD: iced::Font = font(Weight::Semibold);
pub const BOLD: iced::Font = font(Weight::Bold);

// ───────────────────────── Spacing ─────────────────────────

pub const SP_XXS: f32 = 4.0;
pub const SP_XS: f32 = 8.0;
pub const SP_SM: f32 = 12.0;
pub const SP_MD: f32 = 16.0;
pub const SP_LG: f32 = 24.0;
pub const SP_XL: f32 = 32.0;
pub const SP_XXL: f32 = 48.0;
pub const SP_SECTION: f32 = 96.0;

// ───────────────────────── Radius ─────────────────────────

pub const R_XS: f32 = 4.0;
pub const R_SM: f32 = 6.0;
pub const R_MD: f32 = 8.0;
pub const R_LG: f32 = 12.0;
pub const R_XL: f32 = 16.0;
pub const R_PILL: f32 = 9999.0;
pub const R_FULL: f32 = 9999.0;

/// 콘텐츠 컬럼 최대 너비(centered SaaS container).
pub const CONTENT_MAX_WIDTH: f32 = 820.0;

// ───────────────────────── Application ─────────────────────────

/// 윈도우 전체 배경(캔버스)과 기본 텍스트 색.
pub fn application(_state: &crate::app::AddressBook, theme: &Theme) -> iced::theme::Style {
    let p = palette(theme);
    iced::theme::Style {
        background_color: p.canvas,
        text_color: p.ink,
    }
}

// ───────────────────────── Text ─────────────────────────

/// 헤드라인/주요 텍스트(ink).
pub fn text_ink(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(palette(theme).ink),
    }
}

/// 본문 텍스트(body).
pub fn text_body(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(palette(theme).body),
    }
}

/// 보조 텍스트(muted).
pub fn text_muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(palette(theme).muted),
    }
}

/// 3차 텍스트(muted-soft).
pub fn text_muted_soft(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(palette(theme).muted_soft),
    }
}

// ───────────────────────── Containers ─────────────────────────

/// 상단 네비게이션 바 컨테이너(64px 높이, 캔버스, 1px 하단 테두리).
pub fn top_nav(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.canvas)),
        text_color: Some(p.ink),
        border: Border {
            color: p.hairline,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// 캔버스 표면 + 헤어라인 + 12px 라운드의 제품 목업/입력 폼 카드.
pub fn card(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.canvas)),
        text_color: Some(p.ink),
        border: Border {
            color: p.hairline,
            width: 1.0,
            radius: R_LG.into(),
        },
        ..container::Style::default()
    }
}

/// 라이트 그레이(surface-card) + 12px 라운드의 연락처 카드.
pub fn contact_card(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.surface_card)),
        text_color: Some(p.ink),
        border: Border {
            color: p.hairline,
            width: 1.0,
            radius: R_LG.into(),
        },
        ..container::Style::default()
    }
}

/// 에러 배너(소프트 레드 틴트 + 8px 라운드).
pub fn error_banner(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(ERROR_SOFT)),
        text_color: Some(ERROR),
        border: Border {
            color: ERROR_BORDER,
            width: 1.0,
            radius: R_MD.into(),
        },
        ..container::Style::default()
    }
}

/// badge-pill (surface-card 표면 + ink 텍스트 + 알약 모양).
pub fn badge_pill(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.surface_card)),
        text_color: Some(p.ink),
        border: Border {
            color: p.hairline,
            width: 1.0,
            radius: R_PILL.into(),
        },
        ..container::Style::default()
    }
}

/// 레거시 호환용 badge 함수.
pub fn badge(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.surface_card)),
        text_color: Some(p.primary),
        border: Border {
            color: p.hairline,
            width: 1.0,
            radius: R_PILL.into(),
        },
        ..container::Style::default()
    }
}

/// nav-pill-group: 토글(category-tab) 세그먼트를 감싸는 알약형 래퍼.
///
/// DESIGN.md `{component.nav-pill-group}` — surface-soft 배경, pill 라운드.
pub fn nav_pill_group(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.pill_group)),
        text_color: Some(p.ink),
        border: Border {
            color: p.hairline,
            width: 1.0,
            radius: R_PILL.into(),
        },
        ..container::Style::default()
    }
}

/// 이름 기반으로 일관된 Cal.com 파스텔 아바타 색상을 선택한다.
pub fn avatar_color_for_name(name: &str) -> Color {
    let hash = name.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
    match hash % 4 {
        | 0 => BADGE_ORANGE,
        | 1 => BADGE_PINK,
        | 2 => BADGE_VIOLET,
        | _ => BADGE_EMERALD,
    }
}

/// 파스텔 배경의 원형 아바타 컨테이너 스타일.
pub fn avatar_circle(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(Background::Color(color)),
        text_color: Some(ON_PRIMARY),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: R_FULL.into(),
        },
        ..container::Style::default()
    }
}

/// 다크 푸터 컨테이너(surface-dark #101010, 다크 테마에서는 더 깊은 블랙).
pub fn footer(theme: &Theme) -> container::Style {
    let p = palette(theme);
    container::Style {
        background: Some(Background::Color(p.footer)),
        text_color: Some(ON_DARK_SOFT),
        border: Border {
            color: p.footer_border,
            width: if p.footer_border == Color::TRANSPARENT { 0.0 } else { 1.0 },
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

// ───────────────────────── Buttons ─────────────────────────

/// 단일 primary CTA (라이트: 검정 #111111 / 다크: 흰색, 8px 라운드).
pub fn primary_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    let (background, text_color) = match status {
        | button::Status::Hovered | button::Status::Pressed => (p.primary_active, p.on_primary),
        | button::Status::Disabled => (p.primary_disabled, p.muted),
        | _ => (p.primary, p.on_primary),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: R_MD.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// 보조 secondary CTA (캔버스 + 잉크 텍스트 + 헤어라인 + 8px 라운드).
pub fn secondary_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    let background = match status {
        | button::Status::Hovered | button::Status::Pressed => p.surface_soft,
        | _ => p.canvas,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: p.ink,
        border: Border {
            color: p.hairline,
            width: 1.0,
            radius: R_MD.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// 유틸리티 버튼(캔버스 표면 + 8px 라운드). Edit 같은 행내 액션에 사용.
pub fn utility_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    let background = match status {
        | button::Status::Hovered | button::Status::Pressed => p.surface_card,
        | _ => p.canvas,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: p.ink,
        border: Border {
            color: p.hairline,
            width: 1.0,
            radius: R_MD.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// 삭제 버튼(에러 컬러 액센트 + 8px 라운드).
pub fn delete_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = palette(theme);
    let (background, text_color, border_color) = match status {
        | button::Status::Hovered | button::Status::Pressed => (ERROR_SOFT, ERROR, ERROR_BORDER),
        | _ => (p.canvas, p.muted, p.hairline),
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: R_MD.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// category-tab / category-tab-active: nav-pill-group 안의 토글 세그먼트.
///
/// 활성 세그먼트는 캔버스 알약 + 잉크 텍스트 + 미세한 드롭 섀도(pill-in-pill),
/// 비활성 세그먼트는 투명 배경 + muted 텍스트. DESIGN.md 의 no-hover 정책에
/// 따라 호버 상태는 별도로 스타일링하지 않는다.
pub fn category_tab(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme: &Theme, _status: button::Status| {
        let p = palette(theme);
        if active {
            button::Style {
                background: Some(Background::Color(p.tab_active)),
                text_color: p.ink,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: R_PILL.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba8(0x00, 0x00, 0x00, 0.08),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                },
                snap: true,
            }
        } else {
            button::Style {
                background: None,
                text_color: p.muted,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: R_PILL.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            }
        }
    }
}

// ───────────────────────── Inputs ─────────────────────────

/// 텍스트 입력(캔버스 + 8px 라운드, 포커스 시 INK 테두리).
pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let p = palette(theme);
    let border_color = match status {
        | text_input::Status::Focused {
            ..
        } => p.ink,
        | _ => p.hairline,
    };
    text_input::Style {
        background: Background::Color(p.canvas),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: R_MD.into(),
        },
        icon: p.muted_soft,
        placeholder: p.muted_soft,
        value: p.ink,
        selection: p.selection,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn container_and_application_styles_use_design_tokens() {
        let theme = Theme::Light;
        let app = crate::update::tests::app_for_theme_test();
        assert_eq!(application(&app, &theme).background_color, CANVAS);
        assert_eq!(card(&theme).background, Some(Background::Color(CANVAS)));
        assert_eq!(contact_card(&theme).background, Some(Background::Color(SURFACE_CARD)));
        assert_eq!(error_banner(&theme).text_color, Some(ERROR));
        assert_eq!(badge_pill(&theme).text_color, Some(INK));
        assert_eq!(footer(&theme).background, Some(Background::Color(SURFACE_DARK)));
    }

    #[test]
    fn dark_theme_switches_to_dark_palette() {
        let theme = Theme::Dark;
        let app = crate::update::tests::app_for_theme_test();
        assert_eq!(palette(&theme), &DARK);
        assert_eq!(palette(&Theme::Light), &LIGHT);
        assert_eq!(application(&app, &theme).background_color, DARK.canvas);
        assert_eq!(card(&theme).background, Some(Background::Color(DARK.canvas)));
        assert_eq!(text_ink(&theme).color, Some(DARK.ink));
        assert_eq!(primary_button(&theme, button::Status::Active).text_color, DARK.on_primary);
    }

    #[test]
    fn theme_mode_maps_to_iced_theme() {
        assert_eq!(ThemeMode::System.to_theme(), None);
        assert_eq!(ThemeMode::Light.to_theme(), Some(Theme::Light));
        assert_eq!(ThemeMode::Dark.to_theme(), Some(Theme::Dark));
        assert_eq!(ThemeMode::default(), ThemeMode::System);
    }

    #[test]
    fn category_tab_distinguishes_active_segment() {
        let theme = Theme::Light;
        let active = category_tab(true)(&theme, button::Status::Active);
        let inactive = category_tab(false)(&theme, button::Status::Active);
        assert_eq!(active.background, Some(Background::Color(LIGHT.tab_active)));
        assert_eq!(active.text_color, INK);
        assert_eq!(inactive.background, None);
        assert_eq!(inactive.text_color, MUTED);
    }

    #[test]
    fn button_styles_cover_active_and_idle_states() {
        let theme = Theme::Light;
        assert_eq!(primary_button(&theme, button::Status::Active).background, Some(Background::Color(PRIMARY)));
        assert_eq!(
            primary_button(&theme, button::Status::Hovered).background,
            Some(Background::Color(PRIMARY_ACTIVE))
        );
        assert_eq!(secondary_button(&theme, button::Status::Active).background, Some(Background::Color(CANVAS)));
        assert_eq!(
            secondary_button(&theme, button::Status::Pressed).background,
            Some(Background::Color(SURFACE_SOFT))
        );
        assert_eq!(utility_button(&theme, button::Status::Active).background, Some(Background::Color(CANVAS)));
        assert_eq!(
            utility_button(&theme, button::Status::Hovered).background,
            Some(Background::Color(SURFACE_CARD))
        );
        assert_eq!(delete_button(&theme, button::Status::Active).text_color, MUTED);
        assert_eq!(delete_button(&theme, button::Status::Hovered).text_color, ERROR);
    }

    #[test]
    fn input_style_highlights_focus() {
        let theme = Theme::Light;
        assert_eq!(input(&theme, text_input::Status::Active).border.color, HAIRLINE);
        assert_eq!(
            input(
                &theme,
                text_input::Status::Focused {
                    is_hovered: false
                }
            )
            .border
            .color,
            INK
        );
    }

    #[test]
    fn avatar_color_rotates_consistently() {
        let c1 = avatar_color_for_name("Alice");
        let c2 = avatar_color_for_name("Alice");
        assert_eq!(c1, c2);
    }
}

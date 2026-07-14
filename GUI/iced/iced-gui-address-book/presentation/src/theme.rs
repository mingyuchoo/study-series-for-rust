//! DESIGN.md("Notion Analysis") 디자인 토큰을 iced 스타일로 옮긴 모듈.
//!
//! 따뜻한 paper-soft 캔버스 위에 흰색 카드와 하나의 확신에 찬 파란색(primary)을
//! 두는 절제된 시스템이다. 색상·타이포·간격·라운드 토큰과, 위젯별 스타일 함수를
//! 한곳에 모아 뷰 코드가 토큰만 참조하도록(낮은 결합) 한다.

use iced::{Background,
           Border,
           Color,
           Shadow,
           Theme,
           font::{Family,
                  Stretch,
                  Style as FontStyle,
                  Weight},
           widget::{button,
                    container,
                    text_input}};

// ───────────────────────── Colors ─────────────────────────

/// u8 RGB 를 불투명 [`Color`] 로 변환한다(const 문맥에서 사용 가능).
const fn rgb(r: u8, g: u8, b: u8) -> Color { Color::from_rgba8(r, g, b, 1.0) }

/// 따뜻한 종이색 페이지 캔버스(canvas-soft).
pub const CANVAS: Color = rgb(0xf6, 0xf5, 0xf4);
/// 카드·입력 필드 표면(surface).
pub const SURFACE: Color = rgb(0xff, 0xff, 0xff);
/// 본문·제목용 near-black.
pub const INK: Color = rgb(0x00, 0x00, 0x00);
/// 보조 본문(warm charcoal).
pub const INK_SECONDARY: Color = rgb(0x31, 0x30, 0x2e);
/// 약한 보조 텍스트(stone).
pub const INK_MUTED: Color = rgb(0x61, 0x5d, 0x59);
/// 캡션·플레이스홀더(ash).
pub const INK_FAINT: Color = rgb(0xa3, 0x9e, 0x98);
/// 1px 테두리·구분선.
pub const HAIRLINE: Color = rgb(0xe6, 0xe6, 0xe6);
/// 유일한 구조적 강조색(CTA·링크·포커스) — Notion blue.
pub const PRIMARY: Color = rgb(0x00, 0x75, 0xde);
/// primary 의 press 상태.
pub const PRIMARY_ACTIVE: Color = rgb(0x00, 0x5b, 0xab);
/// primary 위의 텍스트.
pub const ON_PRIMARY: Color = rgb(0xff, 0xff, 0xff);
/// 짙은 오렌지(에러 텍스트) — 상태 표시용 스티커 팔레트.
pub const ACCENT_ORANGE_DEEP: Color = rgb(0x79, 0x34, 0x00);

/// primary 의 반투명 선택(selection) 색.
const SELECTION: Color = Color::from_rgba8(0x00, 0x75, 0xde, 0.20);
/// 에러 배너의 옅은 배경.
const ERROR_SOFT: Color = Color::from_rgba8(0xdd, 0x5b, 0x00, 0.08);
/// 에러 배너의 테두리.
const ERROR_BORDER: Color = Color::from_rgba8(0xdd, 0x5b, 0x00, 0.25);

// ───────────────────────── Typography ─────────────────────────

pub const HEADING_1: f32 = 40.0;
pub const HEADING_2: f32 = 26.0;
pub const TITLE: f32 = 20.0;
pub const BODY_MD: f32 = 16.0;
pub const BODY_SM: f32 = 15.0;
pub const CAPTION: f32 = 14.0;
pub const EYEBROW: f32 = 12.0;

/// 디자인 시스템 폰트 패밀리(NotionInter ≈ Inter, 한글은 Noto Sans KR 로 대체).
/// 주어진 weight 로 패밀리 폰트를 만든다.
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

pub const SP_XS: f32 = 8.0;
pub const SP_SM: f32 = 12.0;
pub const SP_MD: f32 = 16.0;
pub const SP_LG: f32 = 24.0;
pub const SP_XXL: f32 = 32.0;

// ───────────────────────── Radius ─────────────────────────

pub const R_XS: f32 = 4.0;
pub const R_MD: f32 = 8.0;
pub const R_LG: f32 = 12.0;
pub const R_FULL: f32 = 9999.0;

/// 콘텐츠 컬럼 최대 너비(centred container).
pub const CONTENT_MAX_WIDTH: f32 = 760.0;

// ───────────────────────── Application ─────────────────────────

/// 윈도우 전체 배경(따뜻한 캔버스)과 기본 텍스트 색.
pub fn application(_state: &crate::app::AddressBook, _theme: &Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: CANVAS,
        text_color: INK,
    }
}

// ───────────────────────── Containers ─────────────────────────

/// 흰색 표면 + 하어라인 + 12px 라운드의 기본 카드(feature-card).
///
/// DESIGN.md 의 기본 카드는 "Level 0 — Flat: hairline border, no shadow" 이다.
/// 그림자는 tiny-skia(소프트웨어 렌더러)에서 매 redraw 마다 픽셀당 SDF/blur 를
/// 계산해 입력 지연을 유발하므로 사용하지 않는다(하어라인만으로 표현).
pub fn card(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        text_color: Some(INK),
        border: Border {
            color: HAIRLINE,
            width: 1.0,
            radius: R_LG.into(),
        },
        ..container::Style::default()
    }
}

/// 에러 배너(스티커 오렌지 기반 상태 표시, 구조색 아님).
pub fn error_banner(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(ERROR_SOFT)),
        text_color: Some(ACCENT_ORANGE_DEEP),
        border: Border {
            color: ERROR_BORDER,
            width: 1.0,
            radius: R_MD.into(),
        },
        ..container::Style::default()
    }
}

/// eyebrow 배지 칩(흰 표면 + primary 텍스트 + 알약 모양).
pub fn badge(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(SURFACE)),
        text_color: Some(PRIMARY),
        border: Border {
            color: HAIRLINE,
            width: 1.0,
            radius: R_FULL.into(),
        },
        ..container::Style::default()
    }
}

// ───────────────────────── Buttons ─────────────────────────

/// 단일 파란 CTA(알약 모양). Add/Update 에 사용.
pub fn primary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        | button::Status::Hovered | button::Status::Pressed => PRIMARY_ACTIVE,
        | _ => PRIMARY,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: ON_PRIMARY,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: R_FULL.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// 보조 CTA(흰 표면 + 잉크 텍스트 + 하어라인 + 알약). Cancel 에 사용.
pub fn secondary_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        | button::Status::Hovered | button::Status::Pressed => CANVAS,
        | _ => SURFACE,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: INK,
        border: Border {
            color: HAIRLINE,
            width: 1.0,
            radius: R_FULL.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// 유틸리티 버튼(흰 표면 + 8px 라운드). Edit/Delete 같은 행내 액션에 사용.
pub fn utility_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        | button::Status::Hovered | button::Status::Pressed => CANVAS,
        | _ => SURFACE,
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: INK_SECONDARY,
        border: Border {
            color: HAIRLINE,
            width: 1.0,
            radius: R_MD.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

// ───────────────────────── Inputs ─────────────────────────

/// 텍스트 입력(흰 표면 + 4px 라운드, 포커스 시 primary 테두리).
pub fn input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        | text_input::Status::Focused {
            ..
        } => PRIMARY,
        | _ => HAIRLINE,
    };
    text_input::Style {
        background: Background::Color(SURFACE),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: R_XS.into(),
        },
        icon: INK_FAINT,
        placeholder: INK_FAINT,
        value: INK,
        selection: SELECTION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_and_application_styles_use_design_tokens() {
        let theme = Theme::Light;
        let app = crate::update::tests::app_for_theme_test();
        assert_eq!(application(&app, &theme).background_color, CANVAS);
        assert_eq!(card(&theme).background, Some(Background::Color(SURFACE)));
        assert_eq!(error_banner(&theme).text_color, Some(ACCENT_ORANGE_DEEP));
        assert_eq!(badge(&theme).text_color, Some(PRIMARY));
    }

    #[test]
    fn button_styles_cover_active_and_idle_states() {
        let theme = Theme::Light;
        assert_eq!(primary_button(&theme, button::Status::Active).background, Some(Background::Color(PRIMARY)));
        assert_eq!(
            primary_button(&theme, button::Status::Hovered).background,
            Some(Background::Color(PRIMARY_ACTIVE))
        );
        assert_eq!(secondary_button(&theme, button::Status::Active).background, Some(Background::Color(SURFACE)));
        assert_eq!(secondary_button(&theme, button::Status::Pressed).background, Some(Background::Color(CANVAS)));
        assert_eq!(utility_button(&theme, button::Status::Active).background, Some(Background::Color(SURFACE)));
        assert_eq!(utility_button(&theme, button::Status::Hovered).background, Some(Background::Color(CANVAS)));
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
            PRIMARY
        );
    }
}

#[allow(unused_imports)]
use ratatui::style::Color;
use ratatui::text::{Line,
                    Text};
use termimad::{FmtText,
               MadSkin};

/// Markdown 문자열을 ratatui Text로 렌더링
pub fn render_to_text(markdown: &str) -> Text<'static> {
    if markdown.is_empty() {
        return Text::from(vec![Line::from("")]);
    }

    // termimad를 사용하여 마크다운 파싱 및 렌더링
    let skin = create_skin();

    // 에러 처리: 파싱 실패 시 원본 텍스트를 그대로 반환
    let fmt_text = match std::panic::catch_unwind(|| FmtText::from(&skin, markdown, None)) {
        | Ok(text) => text,
        | Err(_) => {
            // 파싱 실패 시 plain text로 fallback
            let lines: Vec<Line> = markdown.lines().map(|line| Line::from(line.to_string())).collect();
            return Text::from(lines);
        },
    };

    // termimad의 FmtText를 ratatui Text로 변환
    convert_fmt_text_to_ratatui(fmt_text)
}

/// 커스텀 MadSkin 생성
fn create_skin() -> MadSkin {
    use termimad::crossterm::style::{Attribute,
                                     Color as TermColor};

    let mut skin = MadSkin::default();

    // 헤더 스타일링
    skin.headers[0].compound_style.set_fg(TermColor::Yellow);
    skin.headers[0].compound_style.add_attr(Attribute::Bold);
    skin.headers[1].compound_style.set_fg(TermColor::Cyan);
    skin.headers[1].compound_style.add_attr(Attribute::Bold);
    skin.headers[2].compound_style.set_fg(TermColor::Green);
    skin.headers[2].compound_style.add_attr(Attribute::Bold);

    // Bold, Italic 스타일링 (Bold는 속성만, 색상 없음)
    skin.bold.add_attr(Attribute::Bold);
    skin.italic.add_attr(Attribute::Italic);

    // 인라인 코드 스타일링
    skin.inline_code.set_fg(TermColor::Green);

    // 코드 블록 스타일링
    skin.code_block.compound_style.set_fg(TermColor::Green);
    skin.code_block.compound_style.set_bg(TermColor::DarkGrey);

    skin
}

/// termimad FmtText를 ratatui Text로 변환
fn convert_fmt_text_to_ratatui(fmt_text: FmtText) -> Text<'static> {
    // FmtText를 문자열로 렌더링
    let rendered = format!("{}", fmt_text);

    // 렌더링된 텍스트를 라인별로 파싱하여 ratatui Text로 변환
    // 현재는 기본 텍스트 변환만 수행 (스타일 정보는 나중에 개선)
    let lines: Vec<Line> = rendered
        .lines()
        .map(|line| {
            // ANSI 이스케이프 시퀀스 제거
            let bytes = strip_ansi_escapes::strip(line);
            let clean_line = String::from_utf8(bytes).unwrap_or_else(|_| line.to_string());

            Line::from(clean_line)
        })
        .collect();

    Text::from(lines)
}

/// termimad Color를 ratatui Color로 변환
pub fn to_ratatui_color(term_color: termimad::crossterm::style::Color) -> Color {
    use termimad::crossterm::style::Color as TermColor;

    match term_color {
        | TermColor::Black => Color::Black,
        | TermColor::DarkGrey => Color::DarkGray,
        | TermColor::Red => Color::Red,
        | TermColor::DarkRed => Color::Red,
        | TermColor::Green => Color::Green,
        | TermColor::DarkGreen => Color::Green,
        | TermColor::Yellow => Color::Yellow,
        | TermColor::DarkYellow => Color::Yellow,
        | TermColor::Blue => Color::Blue,
        | TermColor::DarkBlue => Color::Blue,
        | TermColor::Magenta => Color::Magenta,
        | TermColor::DarkMagenta => Color::Magenta,
        | TermColor::Cyan => Color::Cyan,
        | TermColor::DarkCyan => Color::Cyan,
        | TermColor::White => Color::White,
        | TermColor::Grey => Color::Gray,
        | TermColor::Rgb {
            r,
            g,
            b,
        } => Color::Rgb(r, g, b),
        | TermColor::AnsiValue(v) => Color::Indexed(v),
        | TermColor::Reset => Color::Reset,
    }
}

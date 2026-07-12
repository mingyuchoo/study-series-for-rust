//! 한글(CJK) 폰트 설치.
//!
//! egui 기본 폰트에는 한글 글리프가 없으므로, 시스템 한글 폰트를 로드해
//! 입력·표시가 깨지지 않도록 한다. eframe/winit IME 조합 이벤트는 프레임워크가
//! 처리하며, 여기서는 글리프 렌더링용 폰트만 등록한다.

use egui::{FontData,
           FontDefinitions,
           FontFamily,
           FontId,
           TextStyle,
           Theme};
use std::{path::{Path,
                 PathBuf},
          sync::Arc};

/// 폰트 후보: (경로, TTC/OTC face index).
/// Noto Sans CJK OTC에서 KR proportional = 1, mono KR = 6.
fn korean_font_candidates() -> Vec<(PathBuf, u32)> {
    let mut candidates = Vec::new();

    // 사용자 홈 (예: ~/.local/share/fonts)
    if let Some(home) = std::env::var_os("HOME") {
        let local = Path::new(&home).join(".local/share/fonts");
        for name in ["ChosunSg.TTF", "ChosunGu.TTF", "ChosunNm.ttf", "NanumGothic.ttf", "NanumBarunGothic.ttf"] {
            candidates.push((local.join(name), 0));
        }
    }

    // Linux 시스템 경로
    for (path, index) in [
        // Noto Sans CJK KR (index 1 in standard OTC)
        ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 1),
        ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", 1),
        ("/usr/share/fonts/opentype/noto-cjk/NotoSansCJK-Regular.ttc", 1),
        ("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", 1),
        // Nanum
        ("/usr/share/fonts/truetype/nanum/NanumGothic.ttf", 0),
        ("/usr/share/fonts/truetype/nanum/NanumBarunGothic.ttf", 0),
        // Windows
        (r"C:\Windows\Fonts\malgun.ttf", 0),
        (r"C:\Windows\Fonts\malgunbd.ttf", 0),
        // macOS
        ("/System/Library/Fonts/AppleSDGothicNeo.ttc", 0),
        ("/Library/Fonts/AppleGothic.ttf", 0),
        ("/System/Library/Fonts/Supplemental/AppleGothic.ttf", 0),
    ] {
        candidates.push((PathBuf::from(path), index));
    }

    candidates
}

fn load_first_available_korean_font() -> Option<(Vec<u8>, u32, String)> {
    for (path, index) in korean_font_candidates() {
        if !path.is_file() {
            continue;
        }
        match std::fs::read(&path) {
            | Ok(bytes) if !bytes.is_empty() => {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("korean").to_owned();
                eprintln!("[fonts] Korean/CJK font loaded: {} (face index {})", path.display(), index);
                return Some((bytes, index, name));
            },
            | Ok(_) => {
                eprintln!("[fonts] Skipping empty font file: {}", path.display());
            },
            | Err(err) => {
                eprintln!("[fonts] Failed to read {}: {err}", path.display());
            },
        }
    }
    None
}

/// egui 컨텍스트에 한글 폰트를 등록한다.
///
/// 기본 라틴 폰트는 유지하고, 누락 글리프(한글 등)에 대해 CJK 폰트를
/// fallback으로 붙인다.
pub fn install_korean_fonts(ctx: &egui::Context) {
    let Some((bytes, index, _label)) = load_first_available_korean_font() else {
        eprintln!(
            "[fonts] No Korean/CJK font found. Hangul may show as tofu (□). \
             Install e.g. fonts-noto-cjk or Nanum fonts."
        );
        return;
    };

    let mut fonts = FontDefinitions::default();

    let mut font_data = FontData::from_owned(bytes);
    font_data.index = index;

    fonts.font_data.insert("korean".to_owned(), Arc::new(font_data));

    // Fallback: 기본 폰트에 없는 한글 글리프를 이 폰트로 렌더
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.push("korean".to_owned());
    }
    if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
        family.push("korean".to_owned());
    }

    ctx.set_fonts(fonts);

    // 다크/라이트 테마 모두에 텍스트 스타일 적용 (0.35는 테마별 스타일)
    for theme in [Theme::Dark, Theme::Light] {
        let mut style = (*ctx.style_of(theme)).clone();
        style.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Button, FontId::new(14.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace));
        style.text_styles.insert(TextStyle::Small, FontId::new(11.0, FontFamily::Proportional));
        ctx.set_style_of(theme, style);
    }
}

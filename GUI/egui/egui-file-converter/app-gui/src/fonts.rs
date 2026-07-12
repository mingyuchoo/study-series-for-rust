//! CJK (Korean / Japanese / Chinese) font setup for egui.
//!
//! egui's default fonts do not include Hangul glyphs, so Korean UI text and
//! IME composition appear as empty boxes (tofu) until a system CJK font is
//! registered as a fallback.

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

/// Install system CJK fonts as fallbacks so Hangul (and other CJK) glyphs render.
///
/// Also enables proper visual feedback while typing Korean via IME, because
/// composed Hangul syllables must exist in a loaded font face.
pub fn configure_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    let mut loaded_any = false;

    if let Some((label, path, index, bytes)) = load_best_korean_font() {
        log::info!(
            "Loaded Korean/CJK font '{label}' from {} (face index {index})",
            path.display()
        );
        install_font_fallback(&mut fonts, "cjk_primary", bytes, index);
        loaded_any = true;

        // Broaden coverage: if primary came from a Noto CJK collection, also
        // expose the Japanese face (for labels like "日本語") without another disk read
        // of a different file — we re-read once if needed for a second face index.
        if is_noto_cjk_collection(&path) {
            if let Ok(bytes) = fs::read(&path) {
                // KR face (in case primary was a different face)
                if index != 1 {
                    install_font_fallback(&mut fonts, "cjk_kr", bytes.clone(), 1);
                }
                // JP face
                if index != 0 {
                    install_font_fallback(&mut fonts, "cjk_jp", bytes, 0);
                }
            }
        }
    }

    // Always try Noto as an extra fallback when primary was a non-Noto font
    // (e.g. a decorative Hangul face without full CJK coverage).
    if let Some(noto_path) = first_existing_path(&[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
    ]) {
        if fonts.font_data.get("cjk_primary").is_none()
            || !fonts
                .font_data
                .keys()
                .any(|k| k.starts_with("cjk_kr") || k.starts_with("cjk_jp"))
        {
            if let Ok(bytes) = fs::read(&noto_path) {
                log::info!("Loaded Noto Sans CJK fallback from {}", noto_path.display());
                // Prefer KR for Hangul, then JP for remaining CJK.
                install_font_fallback(&mut fonts, "cjk_kr", bytes.clone(), 1);
                install_font_fallback(&mut fonts, "cjk_jp", bytes, 0);
                loaded_any = true;
            }
        }
    }

    if !loaded_any {
        log::warn!(
            "No CJK fonts found. Korean text may not display correctly. \
             Install a Hangul font such as 'fonts-noto-cjk' (Debian/Ubuntu) \
             or 'noto-fonts-cjk' (Arch/Fedora)."
        );
    }

    ctx.set_fonts(fonts);
}

fn is_noto_cjk_collection(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.to_ascii_lowercase().contains("notosanscjk"))
}

fn first_existing_path(paths: &[&str]) -> Option<PathBuf> {
    paths
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

/// Find and load the best available Korean-capable font.
fn load_best_korean_font() -> Option<(String, PathBuf, u32, Vec<u8>)> {
    for (label, path, index) in korean_font_candidates() {
        match fs::read(&path) {
            | Ok(bytes) if bytes.len() >= 100 => {
                return Some((label, path, index, bytes));
            }
            | Ok(_) => {
                log::debug!("Skipping too-small font file: {}", path.display());
            }
            | Err(err) => {
                log::debug!("Could not read font {}: {err}", path.display());
            }
        }
    }
    None
}

fn korean_font_candidates() -> Vec<(String, PathBuf, u32)> {
    let mut candidates = Vec::new();

    // 1) fontconfig — best match for Korean on Linux/BSD when available.
    if let Some((path, index)) = fontconfig_match(":lang=ko") {
        candidates.push(("fontconfig-ko".to_owned(), path, index));
    }

    // 2) Well-known packaged / OS fonts.
    // Noto Sans CJK Regular TTC face order is typically:
    // JP=0, KR=1, SC=2, TC=3, HK=4, ...
    let known: &[(&str, &str, u32)] = &[
        (
            "noto-sans-cjk-kr",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            1,
        ),
        (
            "noto-sans-cjk-kr",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            1,
        ),
        (
            "noto-sans-cjk-kr",
            "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            1,
        ),
        (
            "nanum-gothic",
            "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
            0,
        ),
        (
            "nanum-gothic",
            "/usr/share/fonts/nanum/NanumGothic.ttf",
            0,
        ),
        ("malgun", r"C:\Windows\Fonts\malgun.ttf", 0),
        ("malgun-bd", r"C:\Windows\Fonts\malgunbd.ttf", 0),
        ("gulim", r"C:\Windows\Fonts\gulim.ttc", 0),
        (
            "apple-sd-gothic",
            "/System/Library/Fonts/AppleSDGothicNeo.ttc",
            0,
        ),
        (
            "apple-gothic",
            "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
            0,
        ),
    ];

    for (name, path, index) in known {
        candidates.push(((*name).to_owned(), PathBuf::from(path), *index));
    }

    // 3) User-local fonts
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        for dir in [
            home.join(".local/share/fonts"),
            home.join("Library/Fonts"),
            home.join(".fonts"),
        ] {
            push_user_font_dir(&mut candidates, &dir);
        }
    }

    candidates
}

fn push_user_font_dir(candidates: &mut Vec<(String, PathBuf, u32)>, dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let ext = ext.to_ascii_lowercase();
        if !matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_owned();
        let name_lower = file_name.to_ascii_lowercase();

        let looks_cjk = [
            "nanum", "noto", "cjk", "malgun", "gothic", "myungjo", "hangul", "korean", "chosun",
            "조선",
        ]
        .iter()
        .any(|k| name_lower.contains(k) || file_name.contains(k));

        if looks_cjk {
            candidates.push((format!("user-{file_name}"), path, 0));
        }
    }
}

fn fontconfig_match(pattern: &str) -> Option<(PathBuf, u32)> {
    let output = Command::new("fc-match")
        .args(["-f", "%{file}\n%{index}\n", pattern])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let file = lines.next()?.trim();
    if file.is_empty() || file == "/dev/null" {
        return None;
    }
    let index = lines
        .next()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);

    let path = PathBuf::from(file);
    path.is_file().then_some((path, index))
}

fn install_font_fallback(fonts: &mut FontDefinitions, name: &str, bytes: Vec<u8>, index: u32) {
    if fonts.font_data.contains_key(name) {
        return;
    }

    let mut data = FontData::from_owned(bytes);
    data.index = index;
    fonts.font_data.insert(name.to_owned(), Arc::new(data));

    // Append as fallback so Latin glyphs keep using egui defaults,
    // while Hangul/CJK codepoints fall through to this font.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            if !list.iter().any(|f| f == name) {
                list.push(name.to_owned());
            }
        }
    }
}

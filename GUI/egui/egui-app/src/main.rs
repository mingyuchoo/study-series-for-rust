use eframe::egui;
use std::sync::Arc;

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            // egui 기본 폰트에는 한글 글리프가 없어 입력/표시가 비어 보인다.
            // 시스템 CJK 폰트를 fallback으로 등록한다.
            configure_korean_fonts(&cc.egui_ctx);

            Ok(Box::<MyApp>::default())
        }),
    )
}

/// 한글(및 CJK) 글리프를 가진 시스템 폰트를 egui에 등록한다.
///
/// egui 기본 폰트(Ubuntu 등)는 라틴/일부 기호만 포함하므로,
/// 한글을 입력해도 글리프를 찾지 못해 화면에 아무것도 그려지지 않는다.
fn configure_korean_fonts(ctx: &egui::Context) {
    let Some((path, index)) = find_korean_font() else {
        eprintln!(
            "warning: no Korean font found; Hangul will not render. \
             Install e.g. fonts-noto-cjk (Noto Sans CJK KR)."
        );
        return;
    };

    let font_bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("warning: failed to read Korean font {path}: {err}");
            return;
        }
    };

    let mut fonts = egui::FontDefinitions::default();
    let mut font_data = egui::FontData::from_owned(font_bytes);
    // TTC(예: NotoSansCJK-*.ttc)는 face index로 언어별 글꼴을 고른다.
    font_data.index = index;

    fonts
        .font_data
        .insert("korean".to_owned(), Arc::new(font_data));

    // 기본 라틴 폰트를 유지하고, 없는 글리프(한글)만 이 폰트로 fallback.
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("korean".to_owned());
    }

    ctx.set_fonts(fonts);
    eprintln!("info: loaded Korean font from {path} (face index {index})");
}

/// 플랫폼별 흔한 한글 지원 폰트 경로를 순서대로 탐색한다.
///
/// 반환값: (파일 경로, TTC face index). 단일 TTF/OTF는 index 0.
fn find_korean_font() -> Option<(String, u32)> {
    // (path, face index). Noto Sans CJK Super OTC: 0=JP, 1=KR, 2=SC, 3=TC, 4=HK
    let mut candidates: Vec<(String, u32)> = vec![
        (
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc".into(),
            1, // Noto Sans CJK KR
        ),
        (
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc".into(),
            1,
        ),
        (
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc".into(),
            1,
        ),
        // 단일 파일 한글 폰트 (index 0)
        ("/usr/share/fonts/truetype/nanum/NanumGothic.ttf".into(), 0),
        (
            "/usr/share/fonts/truetype/nanum/NanumBarunGothic.ttf".into(),
            0,
        ),
        (
            "/System/Library/Fonts/AppleSDGothicNeo.ttc".into(),
            0, // macOS
        ),
        ("C:\\Windows\\Fonts\\malgun.ttf".into(), 0), // Windows 맑은 고딕
    ];

    // 사용자 로컬 폰트 (예: ~/.local/share/fonts)
    if let Some(home) = std::env::var_os("HOME") {
        let local = std::path::Path::new(&home).join(".local/share/fonts");
        for name in [
            "ChosunSm.TTF",
            "ChosunCentennial_ttf.ttf",
            "NanumGothic.ttf",
            "NotoSansKR-Regular.otf",
            "NotoSansCJKkr-Regular.otf",
        ] {
            candidates.push((local.join(name).display().to_string(), 0));
        }
    }

    candidates
        .into_iter()
        .find(|(path, _)| std::path::Path::new(path).is_file())
}

struct MyApp {
    name: String,
    age: u32,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: "추명우".to_owned(),
            age: 42,
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("My egui Application");
            ui.horizontal(|ui| {
                let name_label = ui.label("이름 / Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            if ui.button("Increment").clicked() {
                self.age += 1;
            }
            ui.label(format!("안녕하세요 '{}', age {}", self.name, self.age));
        });
    }
}

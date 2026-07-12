use eframe::egui;

mod app;
mod fonts;

use app::FileConverterApp;

fn main() -> Result<(), eframe::Error> {
    // 로깅 초기화
    env_logger::init();

    // eframe 옵션 설정
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Egui File Converter"),
        ..Default::default()
    };

    // 애플리케이션 실행
    eframe::run_native(
        "Egui File Converter",
        options,
        Box::new(|cc| {
            // 한글/CJK 글리프 표시·IME 입력 렌더링을 위한 시스템 폰트 등록
            fonts::configure_cjk_fonts(&cc.egui_ctx);

            // egui 스타일 설정
            cc.egui_ctx.set_visuals(egui::Visuals::default());

            let mut app = FileConverterApp::new(cc);

            // 텍스트 변환 플러그인 등록
            let text_plugin = text_converter::TextConverterPlugin::new();
            if let Err(e) = app.register_plugin(Box::new(text_plugin)) {
                log::error!("Failed to register text converter plugin: {}", e);
            } else {
                log::info!("Text converter plugin registered successfully");
            }

            Ok(Box::new(app))
        }),
    )
}

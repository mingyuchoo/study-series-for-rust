use anyhow::anyhow;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Root, TitleBar, h_flex, v_flex};
use rust_embed::Embed;
use std::borrow::Cow;

#[derive(Embed)]
#[folder = "./assets"]
#[include = "icons/**/*.svg"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> { Ok(Self::iter().filter_map(|p| p.starts_with(path).then(|| p.into())).collect()) }
}

pub struct WindowApp;
impl Render for WindowApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .child(
                TitleBar::new().child(
                    h_flex()
                        .w_full()
                        .pr_2()
                        .justify_between()
                        .font_family("NanumGothic")
                        .child("사용자 정의 타이틀바를 가진 앱")
                        .child("우측 아이템"),
                ),
            )
            .child(
                div()
                    .id("window-body")
                    .p_5()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .font_family("NanumGothic")
                    .child("안녕?")
                    .child(Button::new("ok").primary().label("클릭하세요!").on_click(|_, _, _| println!("클릭되었습니다."))),
            )
    }
}

fn main() {
    let app = Application::new().with_assets(Assets);
    app.run(|cx: &mut App| {
        // Load Korean fonts for proper rendering on Linux
        // Try common Korean fonts available on Fedora
        let font_paths = vec![
            "/usr/share/fonts/google-noto-sans-mono-cjk-vf-fonts/NotoSansMonoCJK-VF.ttc",
            "/usr/share/fonts/google-noto-serif-cjk-vf-fonts/NotoSerifCJK-VF.ttc",
            "/usr/share/fonts/naver-nanum-gothic-fonts/NanumGothic.ttf",
            "/usr/share/fonts/naver-nanum-barun-gothic-fonts/NanumBarunGothic.ttf",
        ];

        for font_path in font_paths {
            match (std::path::Path::new(font_path).exists(), std::fs::read(font_path)) {
                (true, Ok(font_data)) => {
                    cx.text_system().add_fonts(vec![font_data.into()]).ok();
                    break;
                }
                _ => continue,
            }
        }

        // Initialize gpui-component before using any components
        gpui_component::init(cx);

        cx.spawn(async move |cx| {
            let window_options = WindowOptions {
                // Setup GPUI to use custom title bar
                titlebar: Some(TitleBar::title_bar_options()),
                ..Default::default()
            };
            cx.open_window(window_options, |window, cx| {
                let view = cx.new(|_| WindowApp);
                cx.new(|cx| Root::new(view, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}

use gpui::*;
use gpui_component::button::*;
use gpui_component::*;

pub struct HelloWorld;
impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child("Hello, World!")
            .child(Button::new("ok").primary().label("Let's Go!").on_click(|_, _, _| println!("Clicked!")))
    }
}

fn main() {
    Application::new().run(move |cx| {
        gpui_component::init(cx);
        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| HelloWorld);
                cx.new(|cx| Root::new(view.into(), window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}

use async_trait::async_trait;
use axum::{Extension, Router as AxumRouter};
use fluent_templates::{ArcLoader, FluentLoader};
use loco_rs::{
    app::{AppContext, Initializer},
    controller::views::{engines, ViewEngine},
    Error, Result,
};
use tracing::info;

const I18N_DIR: &str = "assets/i18n";
// 언어 ID로 해석되지 않는 이름을 사용해 공통 리소스의 중복 로딩을 방지한다.
const I18N_SHARED: &str = "assets/i18n/_shared.ftl";
#[allow(clippy::module_name_repetitions)]
pub struct ViewEngineInitializer;

fn load_locales() -> Result<ArcLoader> {
    ArcLoader::builder(&I18N_DIR, unic_langid::langid!("en-US"))
        .shared_resources(Some(&[I18N_SHARED.into()]))
        .customize(|bundle| bundle.set_use_isolating(false))
        .build()
        .map_err(|e| Error::string(&format!("Failed to load locales: {e:?}")))
}

#[async_trait]
impl Initializer for ViewEngineInitializer {
    fn name(&self) -> String {
        "view-engine".to_string()
    }

    async fn after_routes(&self, router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        let tera_engine = if std::path::Path::new(I18N_DIR).exists() {
            let arc = std::sync::Arc::new(load_locales()?);
            info!("locales loaded");

            engines::TeraView::build()?.post_process(move |tera| {
                tera.register_function("t", FluentLoader::new(arc.clone()));
                Ok(())
            })?
        } else {
            engines::TeraView::build()?
        };

        Ok(router.layer(Extension(ViewEngine::from(tera_engine))))
    }
}

#[cfg(test)]
mod tests {
    use super::load_locales;
    use fluent_templates::Loader;
    use unic_langid::langid;

    #[test]
    fn locales_load_with_shared_terms_and_fallback() {
        let loader = load_locales().expect("locales should load without duplicate resources");
        let mut locales: Vec<_> = loader.locales().map(ToString::to_string).collect();
        locales.sort();
        assert_eq!(locales, ["de-DE", "en-US"]);
        assert_eq!(
            loader.lookup(&langid!("en-US"), "hello-world"),
            "Hello World!"
        );
        assert_eq!(
            loader.lookup(&langid!("de-DE"), "hello-world"),
            "Hallo Welt!"
        );
        for locale in [langid!("en-US"), langid!("de-DE")] {
            assert_eq!(
                loader.lookup(&locale, "reference"),
                "simple text with a reference: foo"
            );
        }
    }
}

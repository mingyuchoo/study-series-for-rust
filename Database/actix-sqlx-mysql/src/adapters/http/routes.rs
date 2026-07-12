use super::handlers::{create_member,
                      delete_member,
                      get_all_members,
                      get_member,
                      get_member_count,
                      health_check,
                      update_member};
use actix_web::web;

/// HTTP 라우트를 구성합니다.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api").route("/health", web::get().to(health_check)).service(
            web::scope("/members")
                .route("", web::post().to(create_member))
                .route("", web::get().to(get_all_members))
                .route("/count", web::get().to(get_member_count))
                .route("/{id}", web::get().to(get_member))
                .route("/{id}", web::put().to(update_member))
                .route("/{id}", web::delete().to(delete_member)),
        ),
    );
}

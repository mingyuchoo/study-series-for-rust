use actix_sqlx_mysql::{adapters::http::configure_routes,
                       application::MemberUseCase,
                       infra::{database::{create_pool,
                                          get_database_url,
                                          initialize_database},
                               repositories::MySqlMemberRepository}};
use actix_web::{App,
                HttpServer,
                web};
use std::sync::Arc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 환경 변수 로드 (선택사항)
    dotenv::dotenv().ok();

    // 로깅 초기화
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // 데이터베이스 연결 풀 생성
    let database_url = get_database_url();
    log::info!("데이터베이스 연결 중: {}", database_url);

    let pool = create_pool(&database_url).await.expect("데이터베이스 연결 풀 생성 실패");

    log::info!("데이터베이스 연결 성공");

    // 데이터베이스 초기화 (테이블 생성 및 샘플 데이터 삽입)
    initialize_database(&pool).await.expect("데이터베이스 초기화 실패");

    // 의존성 주입: 리포지토리 -> 유스케이스
    let member_repository = Arc::new(MySqlMemberRepository::new(pool));
    let member_usecase = Arc::new(MemberUseCase::new(member_repository));

    // 서버 주소 설정
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8000".to_string())
        .parse::<u16>()
        .expect("PORT는 유효한 숫자여야 합니다");

    log::info!("서버 시작: {}:{}", host, port);

    // HTTP 서버 시작
    HttpServer::new(move || App::new().app_data(web::Data::new(member_usecase.clone())).configure(configure_routes))
        .bind((host.as_str(), port))?
        .run()
        .await
}

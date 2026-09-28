use loco_rs::cli;
use migration::Migrator;
use pi_manager::app::App;

#[tokio::main]
async fn main() -> loco_rs::Result<()> {
    // .env 파일에서 환경변수 로드
    dotenvy::dotenv().ok();

    cli::main::<App, Migrator>().await
}

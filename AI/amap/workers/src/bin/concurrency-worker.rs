use amap_domain::VerificationKind;
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "concurrency-worker",
    about = "AMAP verification worker (Concurrency)"
)]
struct Args {
    /// gRPC listen address (defaults to AMAP_WORKER_LISTEN / config)
    #[arg(long)]
    listen: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    amap_workers::serve(
        "concurrency-worker",
        vec![VerificationKind::Concurrency],
        args.listen,
    )
    .await
}

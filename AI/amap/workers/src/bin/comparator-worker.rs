use amap_domain::VerificationKind;
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "comparator-worker",
    about = "AMAP verification worker (GoldenReplay,Differential,State)"
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
        "comparator-worker",
        vec![
            VerificationKind::GoldenReplay,
            VerificationKind::Differential,
            VerificationKind::State,
        ],
        args.listen,
    )
    .await
}

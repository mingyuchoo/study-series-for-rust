use amap_domain::VerificationKind;
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "replay-worker",
    about = "AMAP verification worker (GoldenReplay,Differential,State,Boundary,Property,Adversarial,Static)"
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
        "replay-worker",
        vec![
            VerificationKind::GoldenReplay,
            VerificationKind::Differential,
            VerificationKind::State,
            VerificationKind::Boundary,
            VerificationKind::Property,
            VerificationKind::Adversarial,
            VerificationKind::Static,
        ],
        args.listen,
    )
    .await
}

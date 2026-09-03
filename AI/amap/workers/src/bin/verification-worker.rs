use amap_domain::VerificationKind;
use clap::Parser;

#[derive(Parser)]
#[command(name = "verification-worker", about = "AMAP verification worker (ALL)")]
struct Args {
    /// gRPC listen address (defaults to AMAP_WORKER_LISTEN / config)
    #[arg(long)]
    listen: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    amap_workers::serve("verification-worker", VerificationKind::ALL.to_vec(), args.listen).await
}

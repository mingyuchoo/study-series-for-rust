use amap_domain::VerificationKind;
use clap::Parser;

#[derive(Parser)]
#[command(name = "fault-worker", about = "AMAP verification worker (Fault)")]
struct Args {
    /// gRPC listen address (defaults to AMAP_WORKER_LISTEN / config)
    #[arg(long)]
    listen: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    amap_workers::serve("fault-worker", vec![VerificationKind::Fault], args.listen).await
}

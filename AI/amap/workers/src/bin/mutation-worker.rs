use amap_domain::VerificationKind;
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "mutation-worker",
    about = "AMAP verification worker (Mutation)"
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
        "mutation-worker",
        vec![VerificationKind::Mutation],
        args.listen,
    )
    .await
}

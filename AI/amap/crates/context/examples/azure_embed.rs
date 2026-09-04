//! Smoke test for Azure OpenAI embeddings: `cargo run -p amap-context --example azure_embed`.
use amap_context::OpenAiEmbeddingProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let p = OpenAiEmbeddingProvider::from_env().ok_or("embedding env vars not set")?;
    println!("endpoint={}", p.endpoint());
    let rows = p
        .embed(&["loan installment".into(), "late fee".into()])
        .await?;
    println!(
        "rows={} dims={:?}",
        rows.len(),
        rows.iter().map(|r| r.len()).collect::<Vec<_>>()
    );
    Ok(())
}

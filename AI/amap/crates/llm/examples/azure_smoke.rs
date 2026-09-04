//! Smoke test for the Azure OpenAI provider: `cargo run -p amap-llm --example azure_smoke`.
//! Requires AZURE_OPENAI_API_KEY, AZURE_OPENAI_ENDPOINT and AZURE_OPENAI_DEPLOYMENT.
use amap_domain::AgentRole;
use amap_llm::{AzureOpenAiProvider, Effort, LlmClient, LlmRequest, TaskKind};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let p = AzureOpenAiProvider::from_env().ok_or("AZURE_OPENAI_* env vars not set")?;
    for effort in [Effort::Low, Effort::Medium, Effort::High] {
        let req = LlmRequest::new(
            TaskKind::CodeReview,
            AgentRole::Reviewer,
            "Reply with a single word.",
            "Say pong.",
        )
        .with_effort(effort);
        let r = p.complete(req).await?;
        println!(
            "effort={effort:?} provider={:?} model={} in={} out={} stop={} text={:?}",
            r.provider, r.model, r.input_tokens, r.output_tokens, r.stop_reason, r.text
        );
    }
    let schema = json!({
        "type": "object",
        "properties": { "answer": { "type": "string" }, "confidence": { "type": "number" } },
        "required": ["answer", "confidence"],
        "additionalProperties": false
    });
    let req = LlmRequest::new(
        TaskKind::DomainClassification,
        AgentRole::Discovery,
        "Classify the domain of the described system.",
        "A system that computes monthly loan installments and late fees.",
    )
    .with_schema(schema);
    let r = p.complete(req).await?;
    println!(
        "structured model={} json={}",
        r.model,
        r.json.ok_or("no JSON parsed")?
    );
    Ok(())
}

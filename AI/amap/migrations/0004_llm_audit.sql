-- LLM call audit ledger (stack §13, §17). Append-only like `evidence`: every prompt/answer an
-- agent used is attributable to a prompt version, provider, model and cost, and per-run token
-- budgets are computed from this table so they survive restarts and are shared by replicas.
CREATE TABLE IF NOT EXISTS llm_audit (
    audit_id       TEXT PRIMARY KEY,
    content_hash   TEXT NOT NULL,
    run_id         TEXT,
    role           TEXT NOT NULL,
    task           TEXT NOT NULL,
    provider       TEXT NOT NULL,
    model          TEXT NOT NULL,
    input_tokens   BIGINT NOT NULL,
    output_tokens  BIGINT NOT NULL,
    cached         BOOLEAN NOT NULL,
    cost_usd       DOUBLE PRECISION NOT NULL,
    created_at     TIMESTAMPTZ NOT NULL,
    doc            JSONB NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_llm_audit_run     ON llm_audit (run_id, created_at);
CREATE INDEX IF NOT EXISTS idx_llm_audit_created ON llm_audit (created_at);

CREATE OR REPLACE FUNCTION reject_llm_audit_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'llm_audit is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS llm_audit_append_only ON llm_audit;
CREATE TRIGGER llm_audit_append_only
BEFORE UPDATE OR DELETE ON llm_audit
FOR EACH ROW EXECUTE FUNCTION reject_llm_audit_mutation();

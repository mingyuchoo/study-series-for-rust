-- AMAP canonical knowledge store (PostgreSQL system of record).
-- Typed key columns + JSONB documents; the graph is projected from `relationships`.
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS business_functions (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS requirements       (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS business_rules     (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS source_units       (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS db_entities        (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS interfaces         (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS behaviors          (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS test_cases         (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS architecture_decisions (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());
CREATE TABLE IF NOT EXISTS review_requests    (id TEXT PRIMARY KEY, function_id TEXT, doc JSONB NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT now());

CREATE TABLE IF NOT EXISTS relationships (
    from_id TEXT NOT NULL,
    to_id   TEXT NOT NULL,
    kind    TEXT NOT NULL,
    PRIMARY KEY (from_id, to_id, kind)
);

-- Evidence metadata (bulk payloads live in the Parquet evidence lake / object store).
CREATE TABLE IF NOT EXISTS evidence (
    id          BIGSERIAL PRIMARY KEY,
    run_id      TEXT NOT NULL,
    function_id TEXT NOT NULL,
    kind        TEXT NOT NULL,
    scenario_id TEXT NOT NULL,
    priority    TEXT NOT NULL,
    passed      BOOLEAN NOT NULL,
    explained   BOOLEAN NOT NULL,
    producer    TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL,
    doc         JSONB NOT NULL
);

-- Semantic retrieval layer (pgvector). Swap for Qdrant at billion-embedding scale.
CREATE TABLE IF NOT EXISTS embeddings (
    id        TEXT PRIMARY KEY,
    kind      TEXT NOT NULL,
    text      TEXT NOT NULL,
    embedding vector(1536)
);

CREATE INDEX IF NOT EXISTS idx_rules_function     ON business_rules (function_id);
CREATE INDEX IF NOT EXISTS idx_behaviors_function ON behaviors (function_id);
CREATE INDEX IF NOT EXISTS idx_tests_function     ON test_cases (function_id);
CREATE INDEX IF NOT EXISTS idx_evidence_function  ON evidence (function_id, kind);
CREATE INDEX IF NOT EXISTS idx_relationships_from ON relationships (from_id);
CREATE INDEX IF NOT EXISTS idx_relationships_to   ON relationships (to_id);

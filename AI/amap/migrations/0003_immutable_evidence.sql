ALTER TABLE evidence ADD COLUMN IF NOT EXISTS evidence_id TEXT;
ALTER TABLE evidence ADD COLUMN IF NOT EXISTS content_hash TEXT;
ALTER TABLE evidence ADD COLUMN IF NOT EXISTS payload_uri TEXT;
UPDATE evidence SET evidence_id = 'EV-LEGACY-' || id::text WHERE evidence_id IS NULL;
UPDATE evidence SET content_hash = '' WHERE content_hash IS NULL;
ALTER TABLE evidence ALTER COLUMN evidence_id SET NOT NULL;
ALTER TABLE evidence ALTER COLUMN content_hash SET NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_evidence_id ON evidence (evidence_id);

CREATE OR REPLACE FUNCTION reject_evidence_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'evidence is append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS evidence_append_only ON evidence;
CREATE TRIGGER evidence_append_only
BEFORE UPDATE OR DELETE ON evidence
FOR EACH ROW EXECUTE FUNCTION reject_evidence_mutation();

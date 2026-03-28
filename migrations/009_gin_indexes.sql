-- GIN indexes for JSONB columns to support future queries on metadata
CREATE INDEX idx_entries_metadata ON entries USING GIN (metadata);
CREATE INDEX idx_tagging_rules_rule ON tagging_rules USING GIN (rule);

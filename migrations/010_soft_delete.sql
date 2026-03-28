ALTER TABLE entries ADD COLUMN deleted_at TIMESTAMPTZ;
CREATE INDEX idx_entries_deleted ON entries (deleted_at) WHERE deleted_at IS NOT NULL;

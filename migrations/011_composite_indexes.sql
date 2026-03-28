-- Drop old indexes from 003_create_entries.sql that are replaced by partial indexes
DROP INDEX IF EXISTS idx_entries_user_created;
DROP INDEX IF EXISTS idx_entries_user_archived;
DROP INDEX IF EXISTS idx_entries_user_starred;

-- Unread list (highest frequency query)
CREATE INDEX idx_entries_user_unread ON entries (user_id, created_at DESC)
    WHERE deleted_at IS NULL AND is_archived = false;

-- Archived list
CREATE INDEX idx_entries_user_archived_v2 ON entries (user_id, archived_at DESC)
    WHERE deleted_at IS NULL AND is_archived = true;

-- Starred list
CREATE INDEX idx_entries_user_starred_v2 ON entries (user_id, starred_at DESC)
    WHERE deleted_at IS NULL AND is_starred = true;

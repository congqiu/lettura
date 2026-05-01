DO $$ BEGIN
    CREATE TYPE audit_action AS ENUM (
        'register', 'login', 'logout', 'refresh_token', 'change_password',
        'regenerate_feed_token', 'create_pat', 'delete_pat',
        'create_entry', 'update_entry', 'soft_delete_entry', 'restore_entry',
        'permanent_delete_entry', 'archive_entry', 'unarchive_entry',
        'star_entry', 'unstar_entry', 'refetch_entry',
        'create_tag', 'delete_tag', 'add_tag_to_entry', 'remove_tag_from_entry',
        'create_annotation', 'update_annotation', 'delete_annotation',
        'create_memo', 'delete_memo', 'promote_memo',
        'create_tagging_rule', 'update_tagging_rule', 'delete_tagging_rule',
        'create_site_rule', 'update_site_rule', 'delete_site_rule',
        'import_wallabag', 'import_browser', 'export_all',
        'create_page', 'update_page', 'delete_page', 'restore_page',
        'admin_backup', 'admin_restore', 'admin_reindex', 'admin_list_users',
        'bulk_tag_add', 'bulk_untag', 'bulk_archive', 'bulk_star',
        'upload_page_files'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE audit_resource_type AS ENUM (
        'user', 'entry', 'tag', 'annotation', 'memo', 'tagging_rule',
        'site_rule', 'page', 'pat', 'system'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS audit_logs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID REFERENCES users(id) ON DELETE SET NULL,
    auth_source     VARCHAR(10) NOT NULL CHECK (auth_source IN ('jwt', 'pat')),

    action          audit_action NOT NULL,
    resource_type   audit_resource_type,
    resource_id     UUID,

    status          VARCHAR(10) NOT NULL CHECK (status IN ('success', 'failure', 'forbidden')),
    details         JSONB NOT NULL DEFAULT '{}',
    error_message   TEXT,

    ip_address      TEXT,
    user_agent      TEXT,
    request_id      UUID,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_created
    ON audit_logs (user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_action_created
    ON audit_logs (action, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_resource
    ON audit_logs (resource_type, resource_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_audit_logs_status_created
    ON audit_logs (status, created_at DESC)
    WHERE status != 'success';

CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at
    ON audit_logs (created_at DESC);

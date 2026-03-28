CREATE TABLE entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    given_url TEXT NOT NULL,
    hashed_url VARCHAR(40) NOT NULL,
    hashed_given_url VARCHAR(40) NOT NULL,
    title TEXT,
    content TEXT,
    text_content TEXT,
    content_type VARCHAR(20) NOT NULL DEFAULT 'article',
    extract_method VARCHAR(20) NOT NULL DEFAULT 'pending',
    is_content_edited BOOLEAN NOT NULL DEFAULT false,
    language VARCHAR(20),
    http_status SMALLINT,
    reading_time INT,
    preview_picture TEXT,
    domain_name VARCHAR(255),
    published_by TEXT,
    metadata JSONB DEFAULT '{}',
    is_archived BOOLEAN NOT NULL DEFAULT false,
    archived_at TIMESTAMPTZ,
    is_starred BOOLEAN NOT NULL DEFAULT false,
    starred_at TIMESTAMPTZ,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX idx_entries_user_hashed_url ON entries (user_id, hashed_url);
CREATE INDEX idx_entries_user_hashed_given_url ON entries (user_id, hashed_given_url);
CREATE INDEX idx_entries_user_created ON entries (user_id, created_at DESC);
CREATE INDEX idx_entries_user_archived ON entries (user_id, is_archived, archived_at DESC);
CREATE INDEX idx_entries_user_starred ON entries (user_id, is_starred, starred_at DESC);
CREATE INDEX idx_entries_domain ON entries (domain_name, user_id);
CREATE INDEX idx_entries_user_language ON entries (user_id, language);

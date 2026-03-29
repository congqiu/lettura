CREATE TABLE pages (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        VARCHAR(12) NOT NULL UNIQUE,
    user_id     UUID NOT NULL REFERENCES users(id),
    title       VARCHAR(500) NOT NULL,
    description TEXT,
    entry_file  VARCHAR(500) NOT NULL,
    password    VARCHAR(255),
    status      VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    file_count  INTEGER NOT NULL DEFAULT 0,
    deleted_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_pages_slug ON pages(slug) WHERE deleted_at IS NULL;
CREATE INDEX idx_pages_user ON pages(user_id) WHERE deleted_at IS NULL;

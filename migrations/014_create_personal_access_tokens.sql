CREATE TABLE personal_access_tokens (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    token_hash   TEXT NOT NULL UNIQUE,
    token_prefix TEXT NOT NULL,
    scope        TEXT NOT NULL CHECK (scope IN ('read', 'write')),
    last_used_at TIMESTAMPTZ,
    expires_at   TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_pat_user ON personal_access_tokens(user_id);
CREATE INDEX idx_pat_hash ON personal_access_tokens(token_hash);

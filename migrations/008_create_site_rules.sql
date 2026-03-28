CREATE TABLE site_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    domain VARCHAR(255) NOT NULL,
    content_selector TEXT NOT NULL,
    title_selector TEXT,
    strip_selectors TEXT[],
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_site_rules_domain ON site_rules (domain);

-- Add token family column for replay detection.
-- When a refresh token is reused (already deleted), all tokens in the same
-- family are revoked to prevent token theft from going undetected.
ALTER TABLE refresh_tokens ADD COLUMN family UUID NOT NULL DEFAULT gen_random_uuid();

CREATE INDEX idx_refresh_tokens_family ON refresh_tokens (family);

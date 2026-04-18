-- Add share expiration for pages (default: never expires)
ALTER TABLE pages ADD COLUMN expires_at TIMESTAMPTZ;

-- Image processing job queue for async image processing
CREATE TYPE image_process_status AS ENUM ('pending', 'processing', 'completed', 'failed');

CREATE TABLE image_process_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entry_id UUID NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    original_html TEXT NOT NULL,
    status image_process_status NOT NULL DEFAULT 'pending',
    error_message TEXT,
    retry_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for fetching pending jobs
CREATE INDEX idx_image_process_jobs_status ON image_process_jobs(status)
    WHERE status IN ('pending', 'processing');

-- Index for looking up jobs by entry
CREATE INDEX idx_image_process_jobs_entry ON image_process_jobs(entry_id);

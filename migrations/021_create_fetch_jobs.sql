CREATE TYPE fetch_job_status AS ENUM ('pending', 'running', 'failed', 'dead');

CREATE TABLE fetch_jobs (
    id             UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entry_id       UUID NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    user_id        UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    url            TEXT NOT NULL,
    status         fetch_job_status NOT NULL DEFAULT 'pending',
    priority       SMALLINT NOT NULL DEFAULT 0,
    attempts       SMALLINT NOT NULL DEFAULT 0,
    max_attempts   SMALLINT NOT NULL DEFAULT 5,
    run_after      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    leased_until   TIMESTAMPTZ,
    leased_by      TEXT,
    last_error     TEXT,
    last_error_at  TIMESTAMPTZ,
    -- Set when a user clicks refetch while this job is in 'running' status.
    -- complete() checks this to decide DELETE vs reset-to-pending, avoiding
    -- overloading the priority column with two meanings.
    refetch_requested_at TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_fetch_jobs_dispatch
    ON fetch_jobs (status, run_after, priority DESC)
    WHERE status IN ('pending', 'failed');

CREATE INDEX idx_fetch_jobs_user_created
    ON fetch_jobs (user_id, created_at DESC);

CREATE INDEX idx_fetch_jobs_entry ON fetch_jobs (entry_id);

CREATE UNIQUE INDEX uq_fetch_jobs_active_entry
    ON fetch_jobs (entry_id)
    WHERE status IN ('pending', 'running', 'failed');

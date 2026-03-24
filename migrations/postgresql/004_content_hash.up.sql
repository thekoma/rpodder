-- Add content_hash to podcasts and episodes to skip unnecessary updates.
-- Add etag/http_last_modified to podcasts for conditional HTTP fetching.

DO $$ BEGIN
    ALTER TABLE podcasts ADD COLUMN content_hash BIGINT NOT NULL DEFAULT 0;
EXCEPTION WHEN duplicate_column THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE podcasts ADD COLUMN etag TEXT;
EXCEPTION WHEN duplicate_column THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE podcasts ADD COLUMN http_last_modified TEXT;
EXCEPTION WHEN duplicate_column THEN NULL;
END $$;

DO $$ BEGIN
    ALTER TABLE episodes ADD COLUMN content_hash BIGINT NOT NULL DEFAULT 0;
EXCEPTION WHEN duplicate_column THEN NULL;
END $$;

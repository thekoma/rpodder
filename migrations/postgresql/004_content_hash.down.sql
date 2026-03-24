ALTER TABLE podcasts DROP COLUMN IF EXISTS content_hash;
ALTER TABLE podcasts DROP COLUMN IF EXISTS etag;
ALTER TABLE podcasts DROP COLUMN IF EXISTS http_last_modified;
ALTER TABLE episodes DROP COLUMN IF EXISTS content_hash;

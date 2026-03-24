-- SQLite does not support DROP COLUMN before 3.35.0; these are best-effort.
ALTER TABLE podcasts DROP COLUMN content_hash;
ALTER TABLE podcasts DROP COLUMN etag;
ALTER TABLE podcasts DROP COLUMN http_last_modified;
ALTER TABLE episodes DROP COLUMN content_hash;

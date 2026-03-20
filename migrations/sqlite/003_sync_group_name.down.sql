-- SQLite doesn't support DROP COLUMN before 3.35.0, but we target modern versions
ALTER TABLE sync_groups DROP COLUMN name;

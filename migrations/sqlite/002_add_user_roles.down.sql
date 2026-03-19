-- SQLite doesn't support DROP COLUMN in older versions, but modern SQLite does
ALTER TABLE users DROP COLUMN is_admin;

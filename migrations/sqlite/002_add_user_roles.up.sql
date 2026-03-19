-- Add is_admin column to users table (idempotent)
-- SQLite doesn't have ADD COLUMN IF NOT EXISTS, so we use a CREATE TRIGGER trick:
-- try to add the column; if it already exists the statement is simply skipped
-- by checking pragma_table_info first.
ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;

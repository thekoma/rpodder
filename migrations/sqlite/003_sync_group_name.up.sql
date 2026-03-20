-- Add name column to sync_groups
ALTER TABLE sync_groups ADD COLUMN name TEXT NOT NULL DEFAULT '';

-- Add name column to sync_groups
DO $$ BEGIN
    ALTER TABLE sync_groups ADD COLUMN name TEXT NOT NULL DEFAULT '';
EXCEPTION WHEN duplicate_column THEN NULL;
END $$;

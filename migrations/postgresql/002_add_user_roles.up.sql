-- Add is_admin column to users table
DO $$ BEGIN
    ALTER TABLE users ADD COLUMN is_admin BOOLEAN NOT NULL DEFAULT FALSE;
EXCEPTION
    WHEN duplicate_column THEN NULL;
END $$;

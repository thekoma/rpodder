-- Add content_hash to podcasts and episodes to skip unnecessary updates.
-- Add etag/http_last_modified to podcasts for conditional HTTP fetching.

ALTER TABLE podcasts ADD COLUMN content_hash INTEGER NOT NULL DEFAULT 0;
ALTER TABLE podcasts ADD COLUMN etag TEXT;
ALTER TABLE podcasts ADD COLUMN http_last_modified TEXT;
ALTER TABLE episodes ADD COLUMN content_hash INTEGER NOT NULL DEFAULT 0;

-- Rebuild FTS index after schema change (content=podcasts needs fresh rowid mapping)
DROP TRIGGER IF EXISTS trg_podcasts_fts_insert;
DROP TRIGGER IF EXISTS trg_podcasts_fts_update;
DROP TRIGGER IF EXISTS trg_podcasts_fts_delete;
DROP TABLE IF EXISTS podcasts_fts;
CREATE VIRTUAL TABLE IF NOT EXISTS podcasts_fts USING fts5(title, description, author, content=podcasts, content_rowid=rowid);
CREATE TRIGGER IF NOT EXISTS trg_podcasts_fts_insert AFTER INSERT ON podcasts BEGIN
    INSERT INTO podcasts_fts(rowid, title, description, author) VALUES (NEW.rowid, NEW.title, NEW.description, NEW.author);
END;
CREATE TRIGGER IF NOT EXISTS trg_podcasts_fts_update AFTER UPDATE ON podcasts BEGIN
    INSERT INTO podcasts_fts(podcasts_fts, rowid, title, description, author) VALUES('delete', OLD.rowid, OLD.title, OLD.description, OLD.author);
    INSERT INTO podcasts_fts(rowid, title, description, author) VALUES (NEW.rowid, NEW.title, NEW.description, NEW.author);
END;
CREATE TRIGGER IF NOT EXISTS trg_podcasts_fts_delete AFTER DELETE ON podcasts BEGIN
    INSERT INTO podcasts_fts(podcasts_fts, rowid, title, description, author) VALUES('delete', OLD.rowid, OLD.title, OLD.description, OLD.author);
END;
INSERT INTO podcasts_fts(podcasts_fts) VALUES('rebuild');

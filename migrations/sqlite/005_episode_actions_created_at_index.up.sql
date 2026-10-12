-- Episode-action sync filters on created_at (server receive time), not the
-- client-supplied timestamp, so late uploads still reach other devices.
CREATE INDEX IF NOT EXISTS idx_epactions_user_created ON episode_actions (user_id, created_at);

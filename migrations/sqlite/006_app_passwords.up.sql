-- Per-client credentials for HTTP Basic. Only the SHA-256 of each token is stored.
CREATE TABLE IF NOT EXISTS app_passwords (
    id           TEXT PRIMARY KEY NOT NULL,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    token_hash   TEXT NOT NULL UNIQUE,
    created_at   TEXT NOT NULL,
    last_used_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_app_passwords_user ON app_passwords (user_id);

-- Nextcloud Login Flow v2 requests.
CREATE TABLE IF NOT EXISTS login_flows (
    id               TEXT PRIMARY KEY NOT NULL,
    poll_token_hash  TEXT NOT NULL UNIQUE,
    login_token_hash TEXT NOT NULL UNIQUE,
    client_name      TEXT NOT NULL,
    user_id          TEXT REFERENCES users(id) ON DELETE CASCADE,
    redeemed_at      TEXT,
    expires_at       TEXT NOT NULL,
    created_at       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_login_flows_expires ON login_flows (expires_at);
CREATE INDEX IF NOT EXISTS idx_login_flows_user ON login_flows (user_id);

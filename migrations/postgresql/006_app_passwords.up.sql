-- Per-client credentials for HTTP Basic. Only the SHA-256 of each token is stored.
-- Keep the hashes VARCHAR. sqlx binds text, and a CHAR column would be compared
-- through a cast that skips the unique index.
CREATE TABLE IF NOT EXISTS app_passwords (
    id           UUID PRIMARY KEY,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         VARCHAR(128) NOT NULL,
    token_hash   VARCHAR(64) NOT NULL UNIQUE,
    created_at   TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_app_passwords_user ON app_passwords (user_id);

-- Nextcloud Login Flow v2 requests.
CREATE TABLE IF NOT EXISTS login_flows (
    id               UUID PRIMARY KEY,
    poll_token_hash  VARCHAR(64) NOT NULL UNIQUE,
    login_token_hash VARCHAR(64) NOT NULL UNIQUE,
    client_name      VARCHAR(128) NOT NULL,
    user_id          UUID REFERENCES users(id) ON DELETE CASCADE,
    redeemed_at      TIMESTAMPTZ,
    expires_at       TIMESTAMPTZ NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_login_flows_expires ON login_flows (expires_at);
CREATE INDEX IF NOT EXISTS idx_login_flows_user ON login_flows (user_id);

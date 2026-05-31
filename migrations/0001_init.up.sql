CREATE TABLE IF NOT EXISTS users (
    discord_id TEXT PRIMARY KEY,
    discord_access_token TEXT NOT NULL,
    discord_refresh_token TEXT NOT NULL,
    jellyfin_url TEXT,
    jellyfin_user_id TEXT,
    jellyfin_access_token TEXT,
    last_active_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

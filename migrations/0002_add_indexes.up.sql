CREATE INDEX IF NOT EXISTS idx_users_last_active_at ON users (last_active_at);
CREATE INDEX IF NOT EXISTS idx_users_updated_at ON users (updated_at);

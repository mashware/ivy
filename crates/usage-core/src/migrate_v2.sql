ALTER TABLE usage_sources ADD COLUMN provider INTEGER NOT NULL DEFAULT 0;
ALTER TABLE usage_observations ADD COLUMN provider INTEGER NOT NULL DEFAULT 0;
ALTER TABLE usage_observations ADD COLUMN reasoning_output INTEGER;
ALTER TABLE usage_canonical ADD COLUMN provider INTEGER NOT NULL DEFAULT 0;
ALTER TABLE usage_canonical ADD COLUMN reasoning_output INTEGER;
ALTER TABLE usage_meta ADD COLUMN codex_last_success_at_ms INTEGER;

PRAGMA user_version = 2;

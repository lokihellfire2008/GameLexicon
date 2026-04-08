ALTER TABLE games ADD COLUMN queue_position    INTEGER;
ALTER TABLE games ADD COLUMN queue_is_playing  INTEGER NOT NULL DEFAULT 0;
ALTER TABLE games ADD COLUMN queue_started_at  TEXT;
ALTER TABLE games ADD COLUMN queue_finished_at TEXT;

CREATE INDEX IF NOT EXISTS idx_games_queue_position ON games(queue_position);
CREATE INDEX IF NOT EXISTS idx_games_queue_playing  ON games(queue_is_playing);

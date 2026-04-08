-- Soft-hide rows from normal library views while keeping import identity.
ALTER TABLE games
  ADD COLUMN hidden_at TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_games_hidden_at ON games(hidden_at);

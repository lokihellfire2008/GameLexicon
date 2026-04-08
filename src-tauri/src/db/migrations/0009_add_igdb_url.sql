ALTER TABLE games ADD COLUMN igdb_url TEXT;

CREATE INDEX IF NOT EXISTS idx_games_igdb_id ON games(igdb_id);

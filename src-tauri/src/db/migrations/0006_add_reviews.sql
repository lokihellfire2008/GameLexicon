ALTER TABLE games
  ADD COLUMN review_rating INTEGER
  CHECK (review_rating IS NULL OR review_rating BETWEEN 0 AND 5);

ALTER TABLE games
  ADD COLUMN review_text TEXT;

CREATE INDEX IF NOT EXISTS idx_games_review_rating ON games(review_rating);

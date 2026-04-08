-- Track whether a row has ever received IGDB enrichment.
ALTER TABLE games
  ADD COLUMN enriched_at TEXT NULL;

-- Backfill previously enriched rows (legacy data) using IGDB-specific fields only.
UPDATE games
SET enriched_at = COALESCE(enriched_at, updated_at)
WHERE igdb_id IS NOT NULL
   OR (summary IS NOT NULL AND TRIM(summary) <> '')
   OR (storyline IS NOT NULL AND TRIM(storyline) <> '')
   OR agg_rating IS NOT NULL
   OR user_rating IS NOT NULL
   OR ttb_main IS NOT NULL
   OR ttb_extra IS NOT NULL
   OR ttb_complete IS NOT NULL
   OR ttb_count IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_games_enriched_at ON games(enriched_at);

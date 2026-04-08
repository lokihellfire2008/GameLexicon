-- 0002_add_igdb_details.sql
PRAGMA foreign_keys = ON;

ALTER TABLE games ADD COLUMN igdb_id        INTEGER;
ALTER TABLE games ADD COLUMN summary        TEXT;
ALTER TABLE games ADD COLUMN storyline      TEXT;
ALTER TABLE games ADD COLUMN agg_rating     REAL;   -- critic aggregate (0-100)
ALTER TABLE games ADD COLUMN user_rating    REAL;   -- user rating (0-100)

-- Time-to-beat (seconds) from IGDB "game_time_to_beats"
ALTER TABLE games ADD COLUMN ttb_main       INTEGER;
ALTER TABLE games ADD COLUMN ttb_extra      INTEGER;
ALTER TABLE games ADD COLUMN ttb_complete   INTEGER;
ALTER TABLE games ADD COLUMN ttb_count      INTEGER;

-- User notes we store locally
ALTER TABLE games ADD COLUMN notes          TEXT;

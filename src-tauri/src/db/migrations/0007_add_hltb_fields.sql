ALTER TABLE games ADD COLUMN hltb_id             INTEGER;
ALTER TABLE games ADD COLUMN hltb_title          TEXT;
ALTER TABLE games ADD COLUMN hltb_url            TEXT;
ALTER TABLE games ADD COLUMN hltb_main           INTEGER;
ALTER TABLE games ADD COLUMN hltb_extra          INTEGER;
ALTER TABLE games ADD COLUMN hltb_complete       INTEGER;
ALTER TABLE games ADD COLUMN hltb_all            INTEGER;
ALTER TABLE games ADD COLUMN hltb_main_count     INTEGER;
ALTER TABLE games ADD COLUMN hltb_extra_count    INTEGER;
ALTER TABLE games ADD COLUMN hltb_complete_count INTEGER;
ALTER TABLE games ADD COLUMN hltb_all_count      INTEGER;
ALTER TABLE games ADD COLUMN hltb_platforms      TEXT;
ALTER TABLE games ADD COLUMN hltb_genres         TEXT;
ALTER TABLE games ADD COLUMN hltb_summary        TEXT;
ALTER TABLE games ADD COLUMN hltb_match_method   TEXT;
ALTER TABLE games ADD COLUMN hltb_updated_at     TEXT;

CREATE INDEX IF NOT EXISTS idx_games_hltb_id ON games(hltb_id);

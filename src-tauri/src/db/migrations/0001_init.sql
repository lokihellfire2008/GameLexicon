PRAGMA foreign_keys = ON;

-- Full schema for dev (in-memory DB each run)
CREATE TABLE games (
  -- primary key (string id generated in Rust)
  id               TEXT PRIMARY KEY,

  -- canonical details
  canonical_title  TEXT    NOT NULL,
  alt_titles       TEXT    NOT NULL DEFAULT '[]',  -- JSON array as text
  platform         TEXT    NOT NULL,

  -- storefront identity for imports (Steam appid, Epic offer id, GOG product id, etc.)
  storefront_id    TEXT,   -- NULL for manual entries

  -- provenance / status
  owned_source     TEXT    NOT NULL,               -- 'Manual', 'Steam API/Community', etc.
  beaten           INTEGER,                        -- store as 0/1

  -- enrichment / metadata
  genre            TEXT,
  cover_url        TEXT,
  release_year     INTEGER,

  -- telemetry
  playtime_minutes INTEGER DEFAULT 0,
  last_played_at   TEXT,                           -- ISO 8601 string

  -- housekeeping
  created_at       DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at       DATETIME DEFAULT CURRENT_TIMESTAMP,

  -- used by INSERT ... ON CONFLICT(platform, storefront_id) in the importer
  UNIQUE(platform, storefront_id)
);

-- helpful indexes (fresh DB each run, so no IF NOT EXISTS needed)
CREATE INDEX idx_games_title    ON games(canonical_title);
CREATE INDEX idx_games_platform ON games(platform);

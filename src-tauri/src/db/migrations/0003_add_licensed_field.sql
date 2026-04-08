-- license_type: owned | subscription | free | trial
ALTER TABLE games
  ADD COLUMN license_type TEXT NOT NULL DEFAULT 'owned'
  CHECK (license_type IN ('owned','subscription','free','trial'));

-- optional human label: "PC Game Pass", "Humble Choice", "Meta Quest+"
ALTER TABLE games
  ADD COLUMN license_source TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_games_license_type ON games(license_type);
CREATE UNIQUE INDEX IF NOT EXISTS uq_games_platform_storefront ON games(platform, storefront_id);

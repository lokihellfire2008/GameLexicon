# Changelog

All notable changes to this project will be documented in this file.

## 2026-04-02

### Added

- Added storefront-only `Time Sync Only` actions for Steam and GOG so existing connected rows can refresh play time without running a full metadata enrich or full library import.
- Added Steam credential persistence in the OS keyring so the saved Steam API key and profile can be reused instead of being re-entered each session.

### Changed

- Changed the Steam and GOG import modals so each flow now clearly separates `Time Sync Only` from the full import action.
- Changed Steam importer messaging to describe the API-key path as the recommended route and to explain that the no-key Steam Community XML path is a less reliable fallback.
- Changed Steam startup behavior so saved Steam credentials are loaded automatically into the importer fields and are re-saved after successful Steam import or time-sync runs.

### Fixed

- Fixed the Steam Community XML fallback URL handling so plain SteamID or Steam profile inputs build a more reliable `/games/?tab=all&xml=1` request instead of quietly returning empty results in some cases.
- Fixed no-key Steam fallback failures to report a clearer error when Steam Community does not actually expose a visible games list, instead of behaving like a successful sync of `0` games.

## 2026-03-28

### Added

- Added a library title index bar with `All`, `#`, and `A-Z` filters, where the `#` bucket groups titles whose first significant character is a number or symbol.
- Added `Select All Shown` and `Select All Matching` library actions so bulk operations can target either the current page or the full filtered result set.
- Added user-managed platform and genre option lists in Settings, with those lists now included as metadata in library backup CSV exports so they can be restored later.

### Changed

- Changed title-based sorting throughout the app to ignore leading English articles such as `A`, `An`, and `The`, while also comparing embedded numbers numerically so names like `10 Monkeys` sort after `4 Crazy Monsters`.
- Changed manual add so new games now attempt an immediate best-match HLTB enrichment pass, which can populate the saved HLTB ID and time-to-beat fields right away.
- Changed manual add, inline edit, and library filter dropdowns to read from the saved platform/genre option lists instead of fixed hardcoded values alone.

## 2026-03-27

### Added

- Added rolling queue completion trackers for games finished in the last 7 days, 30 days, and 365 days on the queue screen.
- Added clickable queue tracker cards that open a modal listing the specific finished games behind each rolling completion window, with quick `Open Card` actions.
- Added default-browser external link actions on the game card for IGDB, HowLongToBeat, and GameFAQs.
- Added exact `igdb_url` persistence in the database so IGDB links can point to the real game page slug instead of guessing from the numeric ID.
- Added a GameFAQs search action that builds a URL-encoded search from the game title so titles with symbols such as `#IDARB` resolve correctly.
- Added persistent backend text logs under `%LOCALAPPDATA%\\GameLexicon\\logs\\` for enrichment activity, backup activity, and database change auditing.
- Added a new `Customization Pass` section in Settings with live color pickers for buttons, text, game card backgrounds, and major section surfaces.
- Added support for uploading custom app background images into `%LOCALAPPDATA%\\GameLexicon\\backgrounds\\` and selecting the active background from Settings.

### Changed

- Changed queue completion tracking so a game's saved `queue_finished_at` metadata stays attached to that game after it leaves the active queue, while still only displaying the rollup stats on the queue page.
- Changed IGDB enrichment and manual IGDB ID apply flows to save the exact IGDB page URL alongside the numeric ID.
- Changed the game card IGDB link behavior to use the stored IGDB page URL, with homepage fallback only when no saved URL exists yet.
- Changed the game card HLTB link behavior to prefer the stored HLTB page URL while keeping homepage fallback behavior available.
- Changed enrichment, backup, and core game mutation commands to append timestamped status lines so users can review what happened after the fact instead of relying on the transient command window.
- Changed the repo cleanup baseline by removing stale backup/temp files, dead placeholder modules, old debug hooks, and unused frontend/backend references that were no longer part of the current app.
- Changed the app shell, queue/library panels, stats modal, enrichment chooser, and game card surfaces to read from the saved customization theme instead of fixed grey/dark colors.

### Fixed

- Fixed broken IGDB game-card links for titles whose real IGDB page URL does not match a simple numeric-ID route.

## 2026-03-20

### Added

- Added a `Backups` toolbar section with separate `Library Backup` and `Queue Backup` flows.
- Added library CSV export so the tracked library can be downloaded as a backup with core manual fields such as title, platform, playtime, notes, review data, license data, and hidden state.
- Added tolerant library CSV import that accepts backups with only game titles required while treating the rest of the fields as optional restore data that can be enriched later.
- Added queue CSV export and restore support so the queue order, playing state, started/finished dates, and tracked playtime can be backed up and restored separately from the main library.
- Added queue restore behavior that recreates missing queued titles as manual library entries when needed so queue backups remain restorable even if the library is incomplete.
- Added a separate `CSV Import` workflow for bringing new games into the library from general spreadsheet data with `Title` + `Platform` required and optional library fields applied when present.

### Changed

- Changed queue restore to replace the current queue order and queue state from the selected CSV so it behaves like a true queue backup/restore flow.
- Changed library and queue CSV exports to use an explicit editable save path, defaulting to a generated file under the user's Downloads location so the backup destination is visible before export.
- Changed the Amazon importer back to a dedicated local Amazon Games SQLite flow, while generic spreadsheet imports now live under the separate `CSV Import` tool.
- Changed CSV-style game importing to skip only exact `title + platform` matches so the same game title can still be imported for another platform.

### Fixed

- Fixed library and queue backup serialization so exported CSVs now include snake_case fields such as playtime, queue position, playing state, and queue dates instead of leaving those columns blank.

## 2026-03-19

### Added

- Added a dedicated game queue with persistent queue position, playing state, started date, and finished date tracking.
- Added queue actions from both the main library table and the game detail card so games can be added or removed from the queue from either view.
- Added a separate queue table with queue number editing, drag reordering, keyboard-friendly up/down reordering buttons, playing controls, started/finished date pickers, time played totals, and a `Time to Beat` column.
- Added automatic queue date behavior:
  - `Started Playing` is seeded from a game's `last played` date when available.
  - Marking a queued game as `Playing` auto-fills `Started Playing` with today's date if no start date exists yet.
- Added queue time tracking that starts from the library playtime value when a game is first queued.
- Added an `Add Time Played` dialog that accepts hours and minutes, converts the entry into tracked playtime, and updates the queue, library table, and downstream stats together.
- Added queue-level stats including queued count, playing count, total time played, and a `Time to Clear` estimate based on preferred Time to Beat values.
- Added a database migration and backend commands to support queue persistence, queue reordering, and queue playtime updates.

### Changed

- Updated the queue `Time to Clear` display to show hours and minutes instead of converting large values into days.
- Updated the queue hero stat label from `Tracked Time` to `Time Played` so it matches the queue table wording.
- Refined the queue header layout so the main hero area now contains the queue guidance text and TTB explanation, while the smaller `Play Next` section stays focused on compact queue stats.
- Kept the queue screen styling aligned with the existing library view while removing library-only controls such as enrich and delete from the queue page.

### Fixed

- Fixed queue row reordering on the Tauri frontend by replacing the unreliable native row drag/drop behavior with a more reliable custom drag interaction.
- Fixed the queue `Add Time Played` action to send the correct Tauri command argument name so time additions save successfully.

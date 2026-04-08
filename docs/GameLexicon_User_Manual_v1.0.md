# GameLexicon User Manual

- Manual version: 1.0
- Document date: 2026-03-22
- App coverage baseline: features verified from the current codebase and changelog through 2026-03-20

[TOC]

## Document Versioning

This section tracks updates to the manual itself so future revisions can be managed without losing history.

| Version | Date | Author | Summary of Changes |
| --- | --- | --- | --- |
| 1.0 | 2026-03-22 | Codex | Initial complete user manual created from the current GameLexicon application codebase. |
| 1.1 | YYYY-MM-DD |  |  |
| 1.2 | YYYY-MM-DD |  |  |

[[SCREENSHOT: Cover Page Or Title Page|Optional: capture the app logo or the main library header for the first page.|Use this if you want the manual to feel branded from page one.]]

## 1. Introduction

GameLexicon is a local-first desktop application for tracking your game library, importing owned titles from multiple sources, organizing a play queue, enriching metadata, recording notes and reviews, and backing up your data to CSV.

The app is organized around two main views:

- `Library`: your full tracked collection.
- `Queue`: the games you plan to play next, in a specific order.

Core capabilities include:

- Manual entry for any game.
- Imports from Steam, Amazon Games, Epic via Legendary, GOG owned titles, Xbox owned titles, and general CSV files.
- Metadata enrichment from IGDB with HLTB time-to-beat fallback.
- Notes, personal star ratings, and written reviews.
- Library and queue backups to CSV.
- Random game selection and library statistics.

## 2. Before You Start

GameLexicon stores your main database locally. In the current build, the default database path is:

- `%LOCALAPPDATA%\GameLexicon\gamelexicon.sqlite`

Credentials are also stored locally:

- Twitch Client ID and Twitch Client Secret are stored in your system keychain.

Some features need internet access or external tools:

- IGDB enrichment needs Twitch credentials.
- Steam import may use the Steam API or the public Steam Community XML fallback.
- Epic import needs the `Legendary` CLI.
- Xbox owned import uses Microsoft device-code sign-in.

[[SCREENSHOT: Main Window Overview|Capture the main GameLexicon window in Library view with the top toolbar visible.|This screenshot works well near the start of the guide because it introduces the app layout.]]

## 3. Understanding the Main Layout

At the top of the window, GameLexicon shows:

- The current view label: `Library` or `Queue`.
- Key summary stats for the active view.
- A two-tab switcher for `Library` and `Queue`.
- A top toolbar divided into `Imports`, `Backups`, and `Library Tools`.

The toolbar includes these buttons:

- `Steam Import`
- `Amazon Games`
- `CSV Import`
- `Epic (Legendary)`
- `GOG (Owned)`
- `Xbox (Owned)`
- `Library Backup`
- `Queue Backup`
- `Enrich Missing`
- `Enrich Unenriched`
- `My Stats`
- `Random Game`
- `Settings`

When enrichment is running, a progress bar appears at the bottom of the window. You can collapse it and reopen it later while the job is still in progress.

[[SCREENSHOT: Top Toolbar Groups|Capture the top toolbar showing Imports, Backups, and Library Tools.|Make sure the button labels are readable so users can map the manual to the interface quickly.]]

## 4. Settings

Open `Settings` from the top toolbar to manage saved credentials.

The current settings window includes:

- `Twitch Client ID`
- `Microsoft (Entra) Client ID`
- `Twitch Client Secret`

To save settings:

1. Click `Settings`.
2. Enter or update the available credential fields.
3. Click `Save`.

Important notes:

- Twitch credentials are used for IGDB lookups and enrichment.
- Twitch credentials are stored securely in your system keychain.
- The Xbox owned import flow opens from the `Xbox (Owned)` toolbar button and uses Microsoft device-code sign-in.
- The Xbox sign-in dialog explicitly states that no client secret is required for that flow.

[[SCREENSHOT: Settings Dialog|Capture the Settings window with all fields visible.|This helps users understand where Twitch credentials are entered and saved.]]

## 5. Adding Games Manually

Use the `Manual Entry` panel in Library view to add a game yourself.

Available fields:

- Title
- Platform
- Genre
- `Beaten` checkbox
- License type: `Owned`, `Subscription`, `Free`, or `Trial`
- License source field when `Subscription` is selected

Supported quick-add platforms in the current UI:

- Steam
- Epic
- GOG
- EA
- Ubisoft
- Amazon
- Xbox
- Nintendo
- PlayStation
- Manual

To add a manual game:

1. Stay in the `Library` view.
2. Enter the game title in `Add game title`.
3. Choose a platform.
4. Optionally choose a genre.
5. Optionally mark the game as `Beaten`.
6. Choose the license type.
7. If you chose `Subscription`, enter a source such as `PC Game Pass`.
8. Click `Add Manual`.

What happens next:

- The game is added immediately.
- If Twitch credentials are available, GameLexicon may look up IGDB candidates for that title and open the match chooser.
- If credentials are not saved yet, the app may prompt you for them during this process.

[[SCREENSHOT: Manual Entry Panel|Capture the Manual Entry section with a sample title, platform, genre, and subscription source filled in.|This is the clearest place to show how license type and manual entry work together.]]

## 6. Importing Games

### 6.1 Steam Import

Open `Steam Import` from the toolbar.

Fields:

- `Steam API Key` (optional)
- `Profile` (required)

Accepted profile formats:

- SteamID64
- Vanity name
- Full Steam profile URL

How to import from Steam:

1. Click `Steam Import`.
2. Enter your Steam profile identifier.
3. Optionally enter a Steam API key.
4. Click `Run Import`.

How the import works:

- If you provide an API key, GameLexicon resolves the profile and imports owned games through the Steam API.
- If you leave the API key blank, the app uses the public Steam Community XML fallback.

Important note:

- When using the XML fallback, the profile and game details must be public.

Imported data can include:

- Title
- Steam cover image
- Playtime
- Last played date

[[SCREENSHOT: Steam Import Dialog|Capture the Steam import dialog showing both the optional API key field and the required profile field.|Use a mock profile example so the expected input format is obvious.]]

### 6.2 Amazon Games Import

Open `Amazon Games` from the toolbar.

This importer reads directly from a local Amazon Games SQLite database.

How to import from Amazon Games:

1. Click `Amazon Games`.
2. Leave the database path blank to let the app auto-detect a likely Amazon Games SQLite location, or paste a full SQLite path manually.
3. Click `Load from SQLite`.
4. Review the detected titles.
5. Use `Select all` or choose individual rows.
6. Leave `Skip duplicates` enabled if you want to avoid importing games that already exist as Amazon entries.
7. Click `Import Selected`.

What this importer can apply:

- Title
- Release year when present
- Genre list when present
- Amazon platform label

Important notes:

- Imported entries are added with platform `Amazon`.
- If auto-detect fails, paste the full SQLite path manually.
- If your source is a normal spreadsheet and not an Amazon SQLite database, use `CSV Import` instead.

[[SCREENSHOT: Amazon Import Load Screen|Capture the Amazon Games dialog before loading, with the optional SQLite path field visible.|This shows users when to leave the path blank versus when to paste one.]]

[[SCREENSHOT: Amazon Import Review Table|Capture the Amazon Games dialog after rows are loaded, including Select all, Skip duplicates, and the preview table.|This is the best screenshot for explaining the review and selection step.]]

### 6.3 CSV Import

Open `CSV Import` from the toolbar when you want to add new library entries from spreadsheet data.

This feature is different from `Library Backup` import:

- `CSV Import` is create-only and skips exact `title + platform` duplicates.
- `Library Backup` import restores or merges backup data into the existing library.

Required columns:

- `Title`
- `Platform`

Supported optional data includes:

- Genre
- Beaten
- Playtime Minutes
- Playtime Hours
- Last Played
- Release Year
- Release Date
- Cover URL
- Hidden
- Notes
- Review Rating
- Review Text
- License Type
- License Source

Common header aliases supported by the current build include:

- `System` or `Console` for `Platform`
- `Genres` for `Genre`
- `Playtime Hours` or `Hours Played`
- `Release Date` for release year parsing

How to use CSV Import:

1. Click `CSV Import`.
2. Choose a `.csv` file.
3. Review the parsed rows in the preview list.
4. Use the checkboxes to include or exclude rows.
5. Click `Import Selected`.

How duplicates are handled:

- Existing matches are checked by exact normalized `title + platform`.
- The same title can still be imported for a different platform.
- This importer does not merge into existing rows.

[[SCREENSHOT: CSV Import File Selection|Capture the CSV Import dialog before a file is loaded, including the explanatory text.|This screenshot helps explain the difference between required and optional columns.]]

[[SCREENSHOT: CSV Import Preview Grid|Capture the CSV Import preview table after loading a file, including row status and selected counts.|This is the best place to show users how the pre-import review step works.]]

### 6.4 Epic Import via Legendary

Open `Epic (Legendary)` from the toolbar.

This importer depends on the external `Legendary` CLI.

Before using it:

1. Install Legendary.
2. Run `legendary auth` at least once outside the app.

How to import Epic titles:

1. Click `Epic (Legendary)`.
2. If Legendary is already in your system `PATH`, leave the path field as-is.
3. If not, paste the full path to `legendary.exe`.
4. Click `Fetch Owned`.
5. Review the title list.
6. Use `Select all` or choose individual rows.
7. Leave `Skip duplicates` enabled if you want to skip titles already present as Epic entries.
8. Click `Import Selected`.

Important notes:

- The app first tries `legendary list-games --json`.
- If JSON output is unavailable, it falls back to parsing plaintext output.
- Imported titles are added with platform `Epic`.
- Hidden Epic entries are skipped during Epic import.

[[SCREENSHOT: Epic Import Fetch Screen|Capture the Epic import dialog before fetching, including the Legendary path field and the note about `legendary auth`.|This screenshot makes the prerequisite easy to understand.]]

[[SCREENSHOT: Epic Import Selection Table|Capture the Epic import preview table with a few selected rows and the Skip duplicates option visible.|This shows the row-review step clearly.]]

### 6.5 GOG Owned Import

Open `GOG (Owned)` from the toolbar.

This importer reads owned GOG titles from the local Galaxy SQLite database and excludes connected-platform entries.

How to import GOG owned titles:

1. Click `GOG (Owned)`.
2. Leave the path blank to use auto-detection, or paste a full path to `galaxy-2.0.db`.
3. Click `Import Owned GOG Games`.

Important notes:

- The default auto-detect target is `C:\ProgramData\GOG.com\Galaxy\storage\galaxy-2.0.db`.
- The importer only includes true GOG-owned entries.
- Connected-platform entries such as Steam, Epic, or Xbox-linked records are excluded.
- The importer can also bring in playtime, last played date, genre, and release year when available.

[[SCREENSHOT: GOG Import Dialog|Capture the GOG import dialog with the optional Galaxy SQLite path field visible.|This is the screenshot most users will need if auto-detection does not work.]]

### 6.6 Xbox Owned Import

Open `Xbox (Owned)` from the toolbar.

This feature uses Microsoft device-code sign-in.

How to import Xbox owned titles:

1. Click `Xbox (Owned)`.
2. In the Microsoft sign-in dialog, note the displayed code and URL.
3. Open the provided Microsoft URL.
4. Enter the displayed code and complete sign-in.
5. Return to GameLexicon.
6. Click `I’ve signed in - Continue`.

What to expect:

- The app imports or updates owned Xbox titles.
- The dialog states that no client secret is required.

[[SCREENSHOT: Xbox Device Code Dialog|Capture the Microsoft sign-in dialog showing the code, URL, and Continue button.|This screenshot is essential because the device-code flow is time-sensitive and unfamiliar to many users.]]

## 7. Library View

The `Library` tab is where you browse, search, filter, edit, enrich, and manage your collection.

The Library view contains:

- Manual Entry
- Library Filters
- The main game table
- Pagination controls

### 7.1 Search and Filters

Available filters in the current build:

- Search title
- Platform
- Beaten status
- Genre
- Minimum hours played
- License type
- Enrichment status
- Review status
- `Show hidden`

How to use filters:

1. Enter or choose any combination of filters.
2. Watch the `shown` count update.
3. Use `Reset filters` to clear all filters.

Important notes:

- Hidden games are excluded unless `Show hidden` is enabled.
- When you search while rows are selected, selected rows remain pinned so they stay visible during the search session.

[[SCREENSHOT: Library Filters Panel|Capture the Browse Library filter panel with several filters populated and the status pills visible.|This screenshot helps users understand what each filter controls and where the counts appear.]]

### 7.2 Sorting

The library table supports sorting by clicking column headers:

- Title
- My Review
- Release
- TTB
- Time Played
- Last Played

Sorting behavior:

- First click sorts descending.
- Second click sorts ascending.
- Third click clears sorting for that column.

### 7.3 Selection and Bulk Actions

You can select rows using:

- The checkbox in the table header to select visible rows on the current page
- The checkbox on each individual row

Available bulk actions:

- Set Selected as Beaten
- Set Selected as Not Beaten
- Hide Selected
- Delete Selected
- Enrich Selected

How to run a bulk action:

1. Select one or more games.
2. Choose an action from `Bulk actions`.
3. Click `Run`.
4. Confirm the action if prompted.

Special note about bulk enrichment:

- It requires Twitch credentials.
- It applies an IGDB preview when found.
- It also attempts HLTB enrichment for time-to-beat data.
- A progress bar appears while the job runs.

[[SCREENSHOT: Library Table With Selection|Capture the library table with several selected rows, the bulk action dropdown, and the Run button visible.|This is the best screenshot for explaining bulk actions and page-level selection.]]

### 7.4 Row Actions

Each library row can include the following actions:

- `Add to Queue` or `Queued #`
- `Enrich`
- `Edit`
- `Hide` or `Unhide`
- `Delete`

How each action works:

- `Add to Queue`: places the game at the end of the queue.
- `Queued #`: jumps you to the Queue view if the game is already queued.
- `Enrich`: opens the IGDB multi-match chooser for that game.
- `Edit`: switches the row into inline edit mode.
- `Hide`: removes the game from normal library browsing until you enable `Show hidden` or unhide it.
- `Delete`: permanently removes the game.

### 7.5 Inline Editing

When you click `Edit`, the row becomes editable.

Fields available in inline edit mode:

- Title
- Platform
- License type
- License source when the license type is `Subscription`
- Beaten
- Genre
- Release year
- Time played in hours
- Last played date
- Cover URL

How to save an edited row:

1. Click `Edit`.
2. Update the desired fields.
3. Click `Save`.

To cancel changes:

1. Click `Cancel`.

Important note:

- If you change the license type away from `Subscription`, the existing subscription source label is cleared.

[[SCREENSHOT: Inline Edit Mode|Capture one library row in Edit mode with the Save and Cancel buttons visible.|This is the most useful screenshot for documenting editable fields and row-level editing.]]

### 7.6 Hidden Games

Hiding a game does not delete it.

What hiding does:

- Removes the game from normal library browsing.
- Excludes it from stats.
- Excludes it from random game selection.
- Excludes it from the standard enrichment passes.

How to reveal hidden games:

1. Enable `Show hidden`.
2. Find the hidden row.
3. Click `Unhide`.

## 8. Game Details Card

Open a game card by clicking a cover image or using `Open Card` from the Queue view.

The game details card has three tabs:

- `Details`
- `Notes`
- `Review`

The top of the card can show:

- Cover image
- Title, platform, release year, and genre
- Critic score
- User score
- Your review summary
- Preferred time-to-beat estimates
- Queue actions
- Manual IGDB ID apply
- Manual HLTB ID apply

[[SCREENSHOT: Game Details Card Overview|Capture a fully populated game card with the header, queue button, IGDB ID field, HLTB ID field, and estimated length panel visible.|This screenshot introduces the most powerful game-level view in the app.]]

### 8.1 Details Tab

The `Details` tab shows:

- Enriched summary text
- Optional storyline

If the game has not been enriched yet, the card shows a message explaining that no enriched summary is available yet.

### 8.2 Notes Tab

The `Notes` tab lets you store personal notes for the selected game.

How to save notes:

1. Open the game card.
2. Go to `Notes`.
3. Enter or edit text.
4. Click `Save Notes`.

### 8.3 Review Tab

The `Review` tab lets you record both a star rating and a written review.

Available rating choices:

- `0 Stars`
- `1` through `5` stars

How to save a review:

1. Open the game card.
2. Go to `Review`.
3. Click a star-rating button.
4. Enter optional review text.
5. Click `Save Review`.

Other review controls:

- `Clear Draft` resets the in-progress rating and review text in the form.

Important note:

- The current build accepts ratings from `0` to `5`.

[[SCREENSHOT: Notes Tab|Capture the Notes tab with example text and the Save Notes button visible.|This screenshot is ideal for documenting the note-taking workflow.]]

[[SCREENSHOT: Review Tab|Capture the Review tab with a selected star rating, review text, and the Save Review button visible.|This screenshot clearly explains the rating and written review workflow.]]

### 8.4 Applying IGDB and HLTB IDs Manually

You can manually set a specific IGDB or HLTB match from the game card.

How to apply an IGDB ID:

1. Open the game card.
2. Enter a numeric value in the `IGDB ID` field.
3. Click `Apply IGDB ID`.

How to apply an HLTB ID:

1. Open the game card.
2. Enter a numeric value in the `HLTB ID` field.
3. Click `Apply HLTB ID`.

Important notes:

- IGDB application requires Twitch credentials.
- Both fields only accept numeric IDs.
- After applying a valid ID, the card refreshes with the new metadata.

### 8.5 Estimated Length and Source Priority

The game card shows preferred estimated length values in the order:

- IGDB first, when available
- HLTB fallback when IGDB is missing

If IGDB and HLTB disagree significantly, the card warns that IGDB is treated as the preferred source.

## 9. Enrichment and Match Selection

GameLexicon supports two top-level enrichment actions:

- `Enrich Missing`
- `Enrich Unenriched`

What `Enrich Missing` does:

- Scans non-hidden games for missing fields such as genre, cover, release year, summary, ratings, and time-to-beat data.
- Fills missing IGDB fields and uses HLTB as fallback for time-to-beat data.

What `Enrich Unenriched` does:

- Focuses only on non-hidden rows that have never been marked as enriched.

How to run enrichment:

1. Save Twitch credentials in `Settings` first, or enter them when prompted.
2. Click either `Enrich Missing` or `Enrich Unenriched`.
3. Monitor the bottom progress bar until the job finishes.

[[SCREENSHOT: Enrichment Progress Bar|Capture the bottom progress bar while an enrichment run is active.|This screenshot is useful because it shows how users can monitor and collapse background work.]]

### 9.1 Enriching a Single Game with the Match Chooser

If you click `Enrich` on a single library row, the app opens an IGDB multi-match chooser.

The chooser lets you:

- Move backward and forward through candidates
- Compare current values against proposed values
- Review genre, release year, cover URL, summary, storyline, ratings, and TTB
- Apply the selected match

How to use the chooser:

1. Click `Enrich` on a library row.
2. Review each candidate.
3. Use the left and right arrows to browse choices.
4. Click `Apply` when the correct match is shown.

What happens after applying:

- IGDB details are saved to the game.
- The app also tries to apply the best HLTB match automatically for that same game.

[[SCREENSHOT: IGDB Match Chooser|Capture the IGDB candidate chooser with the comparison rows and the Apply button visible.|This is one of the most important screenshots in the manual because it explains single-row enrichment accurately.]]

## 10. Queue View

Switch to `Queue` using the main view tabs at the top of the app.

Queue view shows:

- Number of queued games
- Number currently marked as playing
- Estimated time to clear based on preferred TTB values
- Total queue playtime
- Queue table with cover, title, platform, TTB, playing state, dates, time played, and actions

[[SCREENSHOT: Queue View Overview|Capture the Queue tab showing several rows, the queue stats, and the reorder controls.|This screenshot gives users a complete picture of how the queue screen works.]]

### 10.1 Adding Games to the Queue

You can add a game to the queue from:

- The `Add to Queue` button in the library row
- The `Add to Queue` button in the game details card

When a game is added:

- It is placed at the end of the queue.
- Its queue number becomes visible.

### 10.2 Reordering the Queue

The current build supports three reorder methods:

- Drag the `::` handle
- Use the up and down arrow buttons
- Type a new queue number directly

How to reorder by drag-and-drop:

1. Go to `Queue`.
2. Hold the `::` handle on a row.
3. Drag it to the desired position.
4. Release to save the new order.

How to reorder by buttons:

1. Click the up or down arrows on a row.

How to reorder by position number:

1. Edit the queue number field.
2. Press `Enter` or click outside the field.

### 10.3 Playing Status and Dates

Each queued row includes:

- A `Playing` checkbox
- A `Started Playing` date
- A `Finished` date

How the `Playing` checkbox behaves:

- Turning it on marks the game as playing.
- If no started date exists yet, the app sets the started date automatically.

You can also edit the start and finish dates manually at any time.

### 10.4 Adding Time Played

Each queue row includes an `Add Time` button.

How to add playtime:

1. Click `Add Time`.
2. Enter hours and minutes.
3. Review the current total, added amount, and new total shown in the dialog.
4. Click `Add Time`.

What happens after saving:

- Queue playtime updates.
- The library table updates.
- Stats update as well.

[[SCREENSHOT: Add Queue Time Dialog|Capture the Add Time Played dialog with hours, minutes, and the live total preview visible.|This screenshot explains one of the more specialized queue workflows.]]

### 10.5 Opening or Removing a Queued Game

From the queue table you can:

- Click `Open Card`
- Click `Remove`

Removing a game:

- Clears its queue position
- Resets queue-specific playing and date fields
- Renumbers the remaining queue entries

## 11. Backups and Restore

GameLexicon has separate backup workflows for the library and the queue.

This separation matters:

- Library backups preserve your collection data.
- Queue backups preserve queue order and queue state.

### 11.1 Library Backup

Open `Library Backup` from the toolbar.

What the library backup dialog supports:

- Exporting the library to CSV
- Importing a CSV backup to merge or restore library data

How to export a library backup:

1. Click `Library Backup`.
2. Review the suggested export path.
3. Edit the path if needed.
4. Click `Save Library CSV`.

How to import a library backup:

1. Click `Library Backup`.
2. Use the `Import Library CSV` file chooser.
3. Select a CSV file.
4. Wait for the import summary.

How library backup import behaves:

- `Title` is the only required field.
- Platform is optional.
- Existing matching games are updated.
- Missing games can be created as new library entries.
- Missing details can be enriched later.

Typical exported columns include:

- Title
- Platform
- Beaten
- Genre
- Playtime Minutes
- Last Played
- Release Year
- Cover URL
- License Type
- License Source
- Hidden
- Notes
- Review Rating
- Review Text

Important notes:

- The export path is editable.
- If the target folder does not exist, the app creates it.
- Use this workflow for restore or merge behavior, not for create-only spreadsheet import.

[[SCREENSHOT: Library Backup Dialog|Capture the Library Backup dialog showing the export path, Save Library CSV button, and Import Library CSV input.|This screenshot clearly explains both directions of the backup workflow.]]

### 11.2 Queue Backup

Open `Queue Backup` from the toolbar.

What the queue backup dialog supports:

- Exporting the current queue to CSV
- Restoring the queue from a CSV backup

How to export a queue backup:

1. Click `Queue Backup`.
2. Review or edit the export path.
3. Click `Save Queue CSV`.

How to restore a queue backup:

1. Click `Queue Backup`.
2. Choose a CSV file under `Restore Queue From CSV`.
3. Confirm the warning when prompted.
4. Wait for the restore summary.

How queue restore behaves:

- The current queue order and queue state are replaced.
- Missing queued titles are recreated as manual library entries if needed.
- Title is the only required field.

Recommended queue backup columns:

- Queue Position
- Title
- Platform
- Playing
- Started Playing
- Finished
- Playtime Minutes

[[SCREENSHOT: Queue Backup Dialog|Capture the Queue Backup dialog showing the export path field and the restore file chooser.|This screenshot is important because queue restore behaves differently from library backup import.]]

## 12. My Stats

Open `My Stats` from the toolbar.

The stats window shows:

- Total Games Owned
- Total Play Time
- Completion percentage
- Top 5 Most Played

Important note:

- Stats exclude hidden entries.

How to use My Stats:

1. Click `My Stats`.
2. Review the summary cards.
3. Scroll down to see the `Top 5 Most Played` list.
4. Click `Close` when finished.

[[SCREENSHOT: My Stats Dialog|Capture the stats dialog with all three summary cards and the Top 5 Most Played section visible.|This gives users an accurate view of what the stats feature includes.]]

## 13. Random Game Picker

Open `Random Game` from the toolbar.

The Random Game picker lets you choose a random game from your non-hidden library, with optional filters.

Available random filters:

- Platform
- Genre
- Beaten status
- Time-to-beat range

Time-to-beat filter ranges:

- Under 2 hours
- 2 to 5 hours
- 5 to 10 hours
- 10 to 20 hours
- 20 to 40 hours
- 40+ hours
- Unknown

How to use Random Game:

1. Click `Random Game`.
2. Leave all filters blank for a fully random choice, or set the filters you want.
3. Click `Get Game`.
4. Review the selected game card.

Important notes:

- Hidden games are skipped.
- If no games match the chosen filters, the picker tells you no matches are available.
- While viewing a random result, you can use `Start Over`.

[[SCREENSHOT: Random Game Picker|Capture the Random Game dialog with several filters visible and the Get Game button enabled.|This screenshot helps users understand that the feature supports filtered randomness, not just one-click random picks.]]

## 14. Data Rules and Behaviors Worth Knowing

These behaviors are important during everyday use.

### 14.1 Duplicate Handling

Different features use different duplicate rules:

- `CSV Import` skips exact normalized `title + platform` matches.
- `Amazon Games` import skips existing Amazon rows when `Skip duplicates` is enabled.
- `Epic (Legendary)` import skips existing Epic rows when `Skip duplicates` is enabled.
- `Library Backup` import merges or creates rather than acting as create-only import.
- `Queue Backup` restore rebuilds queue state and can create missing titles as manual entries.

### 14.2 Hidden Games

Hidden games:

- Do not appear in the default library list
- Do not appear in stats totals
- Are skipped by the random picker
- Are excluded from the normal enrichment passes

### 14.3 Time-to-Beat Data

GameLexicon can store TTB data from both IGDB and HLTB.

Current display behavior:

- Preferred values use IGDB first when present.
- HLTB fills gaps when IGDB is missing.
- If the two sources disagree enough, the card warns that IGDB is still treated as preferred.

## 15. Troubleshooting

### 15.1 Steam Import Fails

Check the following:

- The profile field is not blank.
- The Steam profile format is valid.
- If no API key is used, your Steam profile and game details are public.

### 15.2 Twitch or IGDB Prompts Keep Appearing

This usually means Twitch credentials are not saved yet.

Fix:

1. Open `Settings`.
2. Save your Twitch Client ID and Twitch Client Secret.
3. Retry the enrichment or manual IGDB action.

### 15.3 Amazon Import Cannot Find a Database

Fix:

1. Retry auto-detect with the path blank.
2. If it still fails, browse to or paste the full Amazon Games SQLite path manually.

### 15.4 Epic Import Cannot Read Legendary

Fix:

1. Make sure Legendary is installed.
2. Run `legendary auth`.
3. If Legendary is not on your system `PATH`, paste the full path to `legendary.exe`.

### 15.5 GOG Import Cannot Find Galaxy Data

Fix:

1. Leave the field blank to try auto-detection.
2. If needed, provide the full path to `galaxy-2.0.db`.

### 15.6 Queue Restore Changed More Than Expected

This is normal for queue restore.

Queue restore:

- Replaces the current queue order
- Replaces queue playing state
- Recreates missing library rows when necessary to rebuild the queue

### 15.7 A Game Disappeared from the Library

Check whether it was hidden instead of deleted.

Fix:

1. Enable `Show hidden`.
2. Find the row marked as hidden.
3. Click `Unhide`.

## 16. Recommended Screenshot Checklist

If you want to add images after the fact, these are the recommended captures to collect in order:

1. Main window overview in Library view
2. Top toolbar groups
3. Settings dialog
4. Manual Entry panel
5. Steam import dialog
6. Amazon import load screen
7. Amazon import review table
8. CSV import file-selection screen
9. CSV import preview grid
10. Epic import fetch screen
11. Epic import selection table
12. GOG import dialog
13. Xbox device-code dialog
14. Library filters panel
15. Library table with selection and bulk actions
16. Inline edit mode
17. Game details card overview
18. Notes tab
19. Review tab
20. Enrichment progress bar
21. IGDB match chooser
22. Queue view overview
23. Add queue time dialog
24. Library backup dialog
25. Queue backup dialog
26. My Stats dialog
27. Random Game picker

## 17. Revision Notes For Future Updates

When the app changes, update this manual in the following order:

1. Update the `Document Versioning` table.
2. Revise the feature sections affected by the change.
3. Replace any screenshots that no longer match the current interface.
4. Regenerate the Word document so the manual and the `.docx` stay in sync.

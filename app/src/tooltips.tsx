export const HINTS = {
  // Top bar
  steamImport: "Import/update your Steam library or refresh play time for existing Steam-connected rows. A Steam API key is recommended; the no-key XML fallback is less reliable.",
  enrichAll: "Fetch IGDB details plus HLTB fallback times for games still missing enrichment fields.",
  enrichUnenriched: "Fetch IGDB details plus HLTB fallback times only for rows that have never been enriched.",
  myStats: "Open a summary of your owned games, total play time, completion, and most-played titles.",
  randomGame: "Open a random-picker flow and jump straight to one matching game card.",
  settings: "Configure Twitch (IGDB) credentials and other settings.",
  libraryBackup: "Export your library to CSV or restore a library backup from CSV.",
  queueBackup: "Export your queue to CSV or restore the queue from a CSV backup.",
  amazonImport: "Import owned Amazon Games titles from the local Amazon Games SQLite database.",
  csvImport: "Add new library entries from a CSV file. Requires Title and Platform and skips exact title + platform duplicates.",
  gogImport: 'Import GOG-owned titles from Galaxy local SQLite or refresh play time for existing GOG-connected rows.',
  epicImport: 'Import Epic via Legendary. Run “legendary auth” first; paste the Legendary path if it’s not in PATH.',


  // Controls / inputs
  addManual: "Add a game manually (optional: preselect a genre and platform).",

  // Sorting / filters
  bulkActions: "Run an action on the selected games.",
  bulkRun: "Execute the chosen action for all selected games.",
  selectAll: "Select/Deselect all games currently visible.",
  selectShown: "Add every row on the current page to the selection.",
  selectMatching: "Add every game matching the current filters to the selection.",
  rowCheckbox: "Select this game for bulk actions.",

  // Row actions
  enrichRow: "Enrich this game by choosing from multiple IGDB matches.",
  editRow: "Edit this game’s fields inline.",
  deleteRow: "Delete this game from your library.",
  saveRow: "Save changes to this game.",
  cancelEdit: "Cancel editing this row.",

  // Modals / footer
  hideProgress: "Hide the progress bar.",
  closeModal: "Close this window.",
  saveNotes: "Save notes for this game.",
  saveReview: "Save your personal star rating and written review for this game.",

  // Multi-match chooser
  carouselPrev: "Previous match",
  carouselNext: "Next match",
  applyCandidate: "Apply the selected IGDB match to this game",
} as const

// app/src/lib/api.ts
import { invoke } from '@tauri-apps/api/core'

/** ----- Shared Types (kept minimal and aligned with your UI) ----- */
export type IgdbGenre = { id: number; name: string }

export type UiGameRow = {
  id: string
  title: string
  platform: string
  beaten: boolean
  genre?: string | null
  playtime_minutes?: number | null
  last_played_at?: string | null
  cover_url?: string | null
  release_year?: number | null
  enriched_at?: string | null
  hidden_at?: string | null
  queue_position?: number | null
  queue_is_playing?: boolean | null
  queue_started_at?: string | null
  queue_finished_at?: string | null
  license_type?: 'owned' | 'subscription' | 'free' | 'trial' | null
  license_source?: string | null
  review_rating?: number | null
  has_review?: boolean | null
  ttb_main?: number | null
  ttb_extra?: number | null
  ttb_complete?: number | null
  hltb_main?: number | null
  hltb_extra?: number | null
  hltb_complete?: number | null
}


export type FullGame = UiGameRow & {
  igdb_id?: number | null
  igdb_url?: string | null
  notes?: string | null
  summary?: string | null
  storyline?: string | null
  agg_rating?: number | null
  user_rating?: number | null
  ttb_main?: number | null
  ttb_extra?: number | null
  ttb_complete?: number | null
  ttb_count?: number | null
  hltb_id?: number | null
  hltb_title?: string | null
  hltb_url?: string | null
  hltb_main?: number | null
  hltb_extra?: number | null
  hltb_complete?: number | null
  hltb_all?: number | null
  hltb_main_count?: number | null
  hltb_extra_count?: number | null
  hltb_complete_count?: number | null
  hltb_all_count?: number | null
  hltb_platforms?: string | null
  hltb_genres?: string | null
  hltb_summary?: string | null
  hltb_match_method?: string | null
  review_text?: string | null
}

export type TwitchCreds = { clientId: string; clientSecret: string }
export type SteamCreds = { apiKey: string; profile: string }
export type AmazonSqlitePreviewRow = {
  title: string
  release_year?: number | null
  genres?: string[] | null
  asin?: string | null
  sku?: string | null
  owned?: boolean | null
}

export type LibraryBackupRow = {
  title: string
  platform: string
  beaten: boolean
  genre?: string | null
  playtime_minutes?: number | null
  last_played_at?: string | null
  release_year?: number | null
  cover_url?: string | null
  license_type?: 'owned' | 'subscription' | 'free' | 'trial' | null
  license_source?: string | null
  hidden: boolean
  notes?: string | null
  review_rating?: number | null
  review_text?: string | null
}

export type LibraryBackupImportRow = {
  title: string
  platform?: string
  beaten?: boolean
  genre?: string
  playtime_minutes?: number
  last_played_at?: string
  release_year?: number
  cover_url?: string
  license_type?: 'owned' | 'subscription' | 'free' | 'trial'
  license_source?: string
  hidden?: boolean
  notes?: string
  review_rating?: number
  review_text?: string
}

export type LibraryBackupImportResult = {
  created: number
  updated: number
  skipped: number
}

export type QueueBackupRow = {
  queue_position: number
  title: string
  platform: string
  queue_is_playing: boolean
  queue_started_at?: string | null
  queue_finished_at?: string | null
  playtime_minutes?: number | null
}

export type QueueBackupImportRow = {
  queue_position?: number
  title: string
  platform?: string
  queue_is_playing?: boolean
  queue_started_at?: string
  queue_finished_at?: string
  playtime_minutes?: number
}

export type QueueBackupImportResult = {
  restored: number
  created: number
  skipped: number
}

export type CustomizationSettings = {
  buttonColor: string
  textColor: string
  cardBackgroundColor: string
  sectionBackgroundColor: string
  backgroundImageName?: string | null
  backgroundImageDataUrl?: string | null
  availableBackgrounds: string[]
  platformOptions: string[]
  genreOptions: string[]
}

export type UploadCustomizationBackgroundResult = {
  fileName: string
  backgroundImageDataUrl: string
  availableBackgrounds: string[]
}

/** ----- DB / Core commands ----- */

export async function addManualGame(payload: {
  canonicalTitle: string
  platform: string
  beaten: boolean
  genre?: string
  licenseType?: 'owned' | 'subscription' | 'free' | 'trial'
  licenseSource?: string
}): Promise<string> {
  // Rust expects { payload: {...} }
  return await invoke<string>('add_manual_game', { payload })
}

export async function listGames(): Promise<UiGameRow[]> {
  return await invoke<UiGameRow[]>('list_games')
}

export async function loadAmazonRowsFromSqlite(dbPath?: string): Promise<AmazonSqlitePreviewRow[]> {
  return await invoke<AmazonSqlitePreviewRow[]>('load_amazon_rows_from_sqlite', {
    dbPath: dbPath ?? null
  })
}

export async function importGogOwnedFromSqlite(dbPath?: string): Promise<number> {
  return await invoke<number>('import_gog_owned_from_sqlite', {
    dbPath: dbPath ?? null
  })
}

export async function syncGogPlaytimeFromSqlite(dbPath?: string): Promise<number> {
  return await invoke<number>('sync_gog_playtime_from_sqlite', {
    dbPath: dbPath ?? null
  })
}

export async function exportLibraryBackup(): Promise<LibraryBackupRow[]> {
  return await invoke<LibraryBackupRow[]>('export_library_backup')
}

export async function importLibraryBackup(rows: LibraryBackupImportRow[]): Promise<LibraryBackupImportResult> {
  return await invoke<LibraryBackupImportResult>('import_library_backup', { rows })
}

export async function exportQueueBackup(): Promise<QueueBackupRow[]> {
  return await invoke<QueueBackupRow[]>('export_queue_backup')
}

export async function importQueueBackup(rows: QueueBackupImportRow[]): Promise<QueueBackupImportResult> {
  return await invoke<QueueBackupImportResult>('import_queue_backup', { rows })
}

export async function defaultBackupExportPath(filename: string): Promise<string> {
  return await invoke<string>('default_backup_export_path', { filename })
}

export async function saveBackupFile(path: string, contents: string): Promise<string> {
  return await invoke<string>('save_backup_file', { path, contents })
}

export async function openExternalLink(url: string): Promise<void> {
  await invoke('open_external_link', { url })
}

export async function getIgdbGenres(): Promise<IgdbGenre[]> {
  return await invoke<IgdbGenre[]>('get_igdb_genres')
}

/** Import/refresh Steam library. Returns number of rows imported/updated. */
export async function importSteamGames(apiKey: string, profile: string): Promise<number> {
  // Exact shape that matches: import_steam_games(db, payload: SteamImportPayload)
  const primary = { payload: { api_key: apiKey ?? '', profile } }

  try {
    const n = await invoke<number>('import_steam_games', primary)
    return Number(n || 0)
  } catch (e) {
    // Fallbacks (in case the backend still accepts legacy shapes in some builds)
    const variants: any[] = [
      { payload: { api_key: apiKey ?? '', profile } },   // (again)
      { payload: { steamApiKey: apiKey ?? '', profile } },
      { api_key: apiKey ?? '', profile },                // old flat
      { apiKey, profile },                               // very old flat
    ]
    let lastErr = e
    for (const v of variants) {
      try { return Number(await invoke<number>('import_steam_games', v) || 0) } catch (err) { lastErr = err }
    }
    throw new Error('Steam import failed: ' + ((lastErr as any)?.message ?? String(lastErr)))
  }
}

/** Refresh playtime + last played for existing Steam-connected rows. */
export async function syncSteamPlaytime(apiKey: string, profile: string): Promise<number> {
  const primary = { payload: { api_key: apiKey ?? '', profile } }

  try {
    const n = await invoke<number>('sync_steam_playtime', primary)
    return Number(n || 0)
  } catch (e) {
    const variants: any[] = [
      { payload: { api_key: apiKey ?? '', profile } },
      { payload: { steamApiKey: apiKey ?? '', profile } },
      { api_key: apiKey ?? '', profile },
      { apiKey, profile },
    ]
    let lastErr = e
    for (const v of variants) {
      try { return Number(await invoke<number>('sync_steam_playtime', v) || 0) } catch (err) { lastErr = err }
    }
    throw new Error('Steam time sync failed: ' + ((lastErr as any)?.message ?? String(lastErr)))
  }
}


// --- XBOX DEVICE-CODE FLOW HELPERS ---

export type XboxDeviceCode = {
  device_code: string;
  user_code: string;
  verification_uri: string;
  verification_uri_complete?: string;
  interval: number;
  expires_in?: number;
  message: string; // HTML-ish string from Microsoft
};

/** Start device-code flow */
export async function xboxBeginDeviceCode(clientId: string): Promise<XboxDeviceCode> {
  return await invoke('xbox_begin_device_code', { clientId });
}

/** Finish device-code flow + import owned titles */
export async function xboxFinishAndImportOwned(
  deviceCode: string,
  clientId: string,
  market = 'US',
  language = 'en-US'
): Promise<number> {
  return await invoke('xbox_finish_device_code_and_import_owned', {
    deviceCode,
    clientId,
    market,
    language,
  });
}




export async function updateGame(changes: {
  id: string
  canonical_title?: string
  platform?: string
  beaten?: boolean
  genre?: string
  playtime_minutes?: number
  last_played_at?: string
  queue_is_playing?: boolean
  queue_started_at?: string
  queue_finished_at?: string
  cover_url?: string
  release_year?: number
  license_type?: 'owned' | 'subscription' | 'free' | 'trial'
  license_source?: string
}): Promise<void> {
  // Rust expects { changes: {...} }
  await invoke('update_game', { changes })
}

export async function deleteGame(id: string): Promise<void> {
  await invoke('delete_game', { id })
}

export async function addGameToQueue(id: string): Promise<void> {
  await invoke('add_game_to_queue', { id })
}

export async function removeGameFromQueue(id: string): Promise<void> {
  await invoke('remove_game_from_queue', { id })
}

export async function reorderQueue(ids: string[]): Promise<void> {
  await invoke('reorder_queue', { ids })
}

export async function addQueuePlaytime(id: string, minutesToAdd: number): Promise<number> {
  return await invoke<number>('add_queue_playtime', { id, minutesToAdd })
}

export async function hideGame(id: string): Promise<void> {
  await invoke('hide_game', { id })
}

export async function unhideGame(id: string): Promise<void> {
  await invoke('unhide_game', { id })
}

/** ----- Settings / Creds ----- */

export async function getTwitchCreds(): Promise<TwitchCreds> {
  return await invoke<TwitchCreds>('get_twitch_credentials')
}

export async function saveTwitchCreds(creds: TwitchCreds): Promise<void> {
  // Rust expects { creds: {...} }
  await invoke('save_twitch_credentials', { creds })
}

export async function getSteamCreds(): Promise<SteamCreds> {
  return await invoke<SteamCreds>('get_steam_credentials')
}

export async function saveSteamCreds(creds: SteamCreds): Promise<void> {
  await invoke('save_steam_credentials', { creds })
}

export async function getCustomizationSettings(): Promise<CustomizationSettings> {
  return await invoke<CustomizationSettings>('get_customization_settings')
}

export async function saveCustomizationSettings(payload: {
  buttonColor: string
  textColor: string
  cardBackgroundColor: string
  sectionBackgroundColor: string
  backgroundImageName?: string | null
  platformOptions: string[]
  genreOptions: string[]
}): Promise<CustomizationSettings> {
  return await invoke<CustomizationSettings>('save_customization_settings', { payload })
}

export async function uploadCustomizationBackground(payload: {
  fileName: string
  mimeType: string
  dataBase64: string
}): Promise<UploadCustomizationBackgroundResult> {
  return await invoke<UploadCustomizationBackgroundResult>('upload_customization_background', { payload })
}

export async function loadCustomizationBackgroundImage(fileName: string): Promise<string | null> {
  return await invoke<string | null>('load_customization_background_image', { fileName })
}

/** ----- Game details / notes ----- */

export async function getGameDetails(id: string): Promise<FullGame> {
  return await invoke<FullGame>('get_game_details', { id })
}

export async function saveGameNotes(id: string, notes: string): Promise<void> {
  await invoke('save_game_notes', { id, notes })
}

export async function saveGameReview(
  id: string,
  rating: number | null,
  review: string | null
): Promise<void> {
  await invoke('save_game_review', { id, rating, review })
}

/** ----- IGDB: enrich all (server side) ----- */

export async function enrichGenresWithIgdb(
  clientId: string,
  clientSecret: string,
  onlyUnenriched = false
): Promise<number> {
  // Rust expects { auth: { clientId, clientSecret } }
  return await invoke<number>('enrich_genres_with_igdb', {
    auth: { clientId, clientSecret },
    onlyUnenriched
  })
}

/** ----- IGDB: title → best preview (server side) ----- */

export async function lookupIgdbPreview(
  title: string,
  clientId: string,
  clientSecret: string
): Promise<any> {
  // Rust expects { title, auth: {...} }
  return await invoke('lookup_igdb_preview', {
    title,
    auth: { clientId, clientSecret }
  })
}

/** ----- IGDB: title → candidates (server side) ----- */

export async function lookupIgdbCandidates(
  title: string,
  clientId: string,
  clientSecret: string
): Promise<any[]> {
  return await invoke<any[]>('lookup_igdb_candidates', {
    title,
    auth: { clientId, clientSecret }
  })
}

/** ----- IGDB: by-id → rich preview (server side) ----- */

export async function lookupIgdbPreviewById(
  igdbId: number | string,
  clientId: string,
  clientSecret: string
): Promise<any> {
  const idNum = Number(igdbId)
  const authCamel = { clientId, clientSecret }
  const authSnake = { client_id: clientId, client_secret: clientSecret }
  const attempts: any[] = [
    // expected modern shape
    { igdbId: idNum, auth: authCamel },
    // snake_case id fallback
    { igdb_id: idNum, auth: authCamel },
    // send both id keys (some builds bind one or the other)
    { id: idNum, igdbId: idNum, igdb_id: idNum, auth: authCamel },
    // nested auth snake_case fallback
    { igdbId: idNum, auth: authSnake },
    { igdb_id: idNum, auth: authSnake },
    { id: idNum, igdbId: idNum, igdb_id: idNum, auth: authSnake },
  ]

  let lastErr: any = null
  for (const payload of attempts) {
    try {
      return await invoke('lookup_igdb_preview_by_id', payload)
    } catch (e) {
      lastErr = e
    }
  }
  throw lastErr ?? new Error('lookup_igdb_preview_by_id failed')
}

/** ----- Apply IGDB details to a game (server side) ----- */

export async function applyIgdbDetails(p: {
  id: string
  igdb_id?: number | string
  igdb_url?: string
  genres?: string[]
  cover_url?: string
  release_year?: number
  summary?: string
  storyline?: string
  agg_rating?: number
  user_rating?: number
  ttb_main?: number
  ttb_extra?: number
  ttb_complete?: number
  ttb_count?: number
}): Promise<void> {
  // Rust expects { p: {...} }
  await invoke('apply_igdb_details', { p })
}

/** ----- HLTB: title -> best preview (server side) ----- */

export async function lookupHltbPreview(
  title: string,
  platform?: string
): Promise<any> {
  const attempts: any[] = [
    { title, platform: platform ?? null },
    { title, platform },
  ]

  let lastErr: any = null
  for (const payload of attempts) {
    try {
      return await invoke('lookup_hltb_preview', payload)
    } catch (e) {
      lastErr = e
    }
  }
  throw lastErr ?? new Error('lookup_hltb_preview failed')
}

/** ----- HLTB: by-id -> rich preview (server side) ----- */

export async function lookupHltbPreviewById(
  hltbId: number | string
): Promise<any> {
  const idNum = Number(hltbId)
  const attempts: any[] = [
    { hltbId: idNum },
    { hltb_id: idNum },
    { id: idNum, hltbId: idNum, hltb_id: idNum },
  ]

  let lastErr: any = null
  for (const payload of attempts) {
    try {
      return await invoke('lookup_hltb_preview_by_id', payload)
    } catch (e) {
      lastErr = e
    }
  }
  throw lastErr ?? new Error('lookup_hltb_preview_by_id failed')
}

/** ----- Apply HLTB details to a game (server side) ----- */

export async function applyHltbDetails(p: {
  id: string
  hltb_id?: number | string
  title?: string
  profile_url?: string
  hltb_main?: number
  hltb_extra?: number
  hltb_complete?: number
  hltb_all?: number
  hltb_main_count?: number
  hltb_extra_count?: number
  hltb_complete_count?: number
  hltb_all_count?: number
  platforms?: string
  genres?: string
  summary?: string
  match_method?: string
}): Promise<void> {
  await invoke('apply_hltb_details', { p })
}

// ---- Epic (Legendary)
export type EpicOwned = { appName: string; title: string }

export async function fetchEpicOwnedViaLegendary(legendaryPath?: string): Promise<EpicOwned[]> {
  const { invoke } = await import('@tauri-apps/api/core');

  const res: any = await invoke('fetch_epic_owned_via_legendary', { legendaryPath: legendaryPath ?? null });
  if (!Array.isArray(res)) return [];

  const clean = (t: string) => String(t ?? '').replace(/^\s*\*\s*/, '').trim();

  return res
    .map((r: any) => ({
      title: clean(r?.title),
      appName: String(r?.app_name ?? r?.appName ?? '').trim(),
    }))
    .filter((r: EpicOwned) => r.title && r.appName);
}

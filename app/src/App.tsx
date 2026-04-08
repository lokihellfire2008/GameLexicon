import { useEffect, useMemo, useRef, useState } from 'react'
import {
  addManualGame,
  addGameToQueue,
  addQueuePlaytime,
  getCustomizationSettings,
  listGames,
  importSteamGames,
  syncSteamPlaytime,
  enrichGenresWithIgdb,
  loadCustomizationBackgroundImage,
  saveCustomizationSettings,
  updateGame,
  getTwitchCreds,
  getSteamCreds,
  saveTwitchCreds,
  saveSteamCreds,
  getGameDetails,
  saveGameNotes,
  saveGameReview,
  lookupIgdbPreview,          // title -> best match (server)
  lookupIgdbCandidates,       // title -> candidates (server)
  lookupIgdbPreviewById,      // igdb id -> rich details (server)
  applyIgdbDetails,
  lookupHltbPreview,
  lookupHltbPreviewById,
  applyHltbDetails,
  deleteGame,
  hideGame,
  removeGameFromQueue,
  reorderQueue,
  unhideGame,
  loadAmazonRowsFromSqlite,
  importGogOwnedFromSqlite,
  syncGogPlaytimeFromSqlite,
  exportLibraryBackup as exportLibraryBackupData,
  importLibraryBackup as importLibraryBackupData,
  exportQueueBackup as exportQueueBackupData,
  importQueueBackup as importQueueBackupData,
  defaultBackupExportPath,
  openExternalLink,
  saveBackupFile,
  fetchEpicOwnedViaLegendary,
  uploadCustomizationBackground,
  xboxBeginDeviceCode, 
  xboxFinishAndImportOwned,
  type CustomizationSettings as CustomizationSettingsDto,
  type IgdbGenre,
  type LibraryBackupImportRow,
  type QueueBackupImportRow,
  type UiGameRow,
  type FullGame,
} from './lib/api'
import { HINTS } from './tooltips'
import './steam-import.css'
import './settings.css'

const STATIC_GENRES: IgdbGenre[] = [
  "Action","Adventure","RPG","Shooter","Strategy","Puzzle","Racing","Sports","Simulation",
  "Platform","Hack and slash/Beat 'em up","Indie","Arcade","Visual Novel","Tactical",
  "RTS","Turn-based strategy","MOBA","Fighting","Music","Rhythm","Quiz/Trivia","Pinball",
  "Roguelike","Sandbox","Survival","Stealth","Point-and-click","Card & Board Game",
  "Battle Royale","Tactical RPG","JRPG","Western RPG","Metroidvania","Party","City Builder"
].map((name, i) => ({ id: i + 1, name } as IgdbGenre))
const DEFAULT_PLATFORM_OPTIONS = ['Steam', 'Epic', 'GOG', 'EA', 'Ubisoft', 'Amazon', 'Xbox', 'Nintendo', 'PlayStation', 'Manual']
const DEFAULT_GENRE_OPTIONS = STATIC_GENRES.map(genre => genre.name)

function hours(min?: number | null) {
  if (!min || min <= 0) return ''
  return (min/60).toFixed(1) + ' hrs'
}

function hoursOrZero(min?: number | null) {
  const safe = Math.max(0, min ?? 0)
  const value = safe / 60
  return `${value.toLocaleString(undefined, {
    minimumFractionDigits: value > 0 && value < 10 ? 1 : 0,
    maximumFractionDigits: value < 100 ? 1 : 0,
  })} hrs`
}

function formatPercent(value: number) {
  return `${value.toFixed(value > 0 && value < 10 ? 1 : 0)}%`
}

function hasReviewRecord(item: { has_review?: boolean | null, review_rating?: number | null, review_text?: string | null }) {
  return !!item.has_review || item.review_rating != null || !!item.review_text?.trim()
}

function formatReviewStars(rating?: number | null) {
  if (rating == null) return 'Not rated'
  const safe = Math.max(0, Math.min(5, Math.round(rating)))
  return `${'★'.repeat(safe)}${'☆'.repeat(5 - safe)}`
}

function reviewCellLabel(row: { has_review?: boolean | null, review_rating?: number | null }) {
  if (row.review_rating != null) return formatReviewStars(row.review_rating)
  if (row.has_review) return 'Text review'
  return ''
}

function firstNumber(...values: Array<number | null | undefined>) {
  for (const value of values) {
    if (value != null && Number.isFinite(value)) return value
  }
  return null
}

function ttbValuesDiffer(a?: number | null, b?: number | null) {
  if (a == null || b == null) return false
  const diff = Math.abs(a - b)
  const baseline = Math.max(Math.abs(a), Math.abs(b), 1)
  return diff >= 7200 && (diff / baseline) >= 0.25
}

function formatEstimateHours(s?: number | null) {
  if (!s || s <= 0) return '—'
  const h = s / 3600
  if (h >= 10) return `${Math.round(h)}h`
  return `${h.toFixed(1)}h`
}

function preferredMainTtbSeconds(item: {
  ttb_main?: number | null
  hltb_main?: number | null
}) {
  return firstNumber(item.ttb_main, item.hltb_main)
}

function matchesRandomTtbFilter(seconds: number | null, filter: RandomTtbFilter) {
  if (!filter) return true
  if (filter === 'unknown') return seconds == null
  if (seconds == null) return false

  const hours = seconds / 3600
  switch (filter) {
    case 'under-2': return hours < 2
    case '2-5': return hours >= 2 && hours < 5
    case '5-10': return hours >= 5 && hours < 10
    case '10-20': return hours >= 10 && hours < 20
    case '20-40': return hours >= 20 && hours < 40
    case '40-plus': return hours >= 40
    default: return true
  }
}

type EditRow = {
  id: string
  title: string
  platform: string
  beaten: boolean
  genre: string
  playtimeHours: string
  lastPlayed: string // yyyy-mm-dd
  coverUrl: string
  releaseYear: string

  // NEW: license fields for editing
  licenseType: 'owned' | 'subscription' | 'free' | 'trial'
  licenseSource: string
}

type Prog = { done: number, total: number, title: string }
type CardTab = 'details' | 'notes' | 'review'
type ScreenView = 'library' | 'queue'
type RandomBeatenFilter = '' | 'yes' | 'no'
type RandomTtbFilter = '' | 'under-2' | '2-5' | '5-10' | '10-20' | '20-40' | '40-plus' | 'unknown'

type SortKey =
  | ''
  | 'title-asc' | 'title-desc'
  | 'release-asc' | 'release-desc'
  | 'ttb-asc' | 'ttb-desc'
  | 'playtime-asc' | 'playtime-desc'
  | 'last-asc' | 'last-desc'
  | 'review-asc' | 'review-desc'

type BulkAction = '' | 'hide' | 'delete' | 'enrich' | 'beaten' | 'unbeaten'
type QueueTimeDialog = null | {
  id: string
  title: string
  currentMinutes: number
  hours: string
  minutes: string
}
type QueueFinishedTrackerKey = 'last7' | 'last30' | 'last365'
type QueueFinishedTrackerDialog = null | {
  key: QueueFinishedTrackerKey
  label: string
}
type QueueFinishedTrackerGame = {
  id: string
  title: string
  platform: string
  finishedAt: string
  queuePosition?: number | null
  hiddenAt?: string | null
}

/** Normalized IGDB candidate shape the UI expects */
type Npv = {
  igdb_id?: number | string
  igdb_url?: string | null
  title?: string
  cover_url?: string | null
  genres?: string[]
  release_year?: number | null
  summary?: string | null
  storyline?: string | null
  agg_rating?: number | null
  user_rating?: number | null
  ttb_main?: number | null   // seconds
  ttb_extra?: number | null  // seconds
  ttb_complete?: number | null // seconds
  ttb_count?: number | null
}

/** Multi-match chooser state */
type OfferState = null | {
  forId: string
  title: string
  current: UiGameRow | null
  candidates: Npv[]
  idx: number
  creds: { cid: string, secret: string }
}

type AmazonImportRow = {
  title: string
  releaseYear?: number | null
  genres?: string[]
  asin?: string | null
  sku?: string | null
  owned?: boolean | null
}

type LibraryCsvImportRow = LibraryBackupImportRow & {
  title: string
  platform: string
}

const AMAZON_PLATFORM = 'Amazon'
const DEFAULT_CUSTOMIZATION: CustomizationSettingsDto = {
  buttonColor: '#0f766e',
  textColor: '#132033',
  cardBackgroundColor: '#ffffff',
  sectionBackgroundColor: '#f4f7fa',
  backgroundImageName: null,
  backgroundImageDataUrl: null,
  availableBackgrounds: [],
  platformOptions: DEFAULT_PLATFORM_OPTIONS,
  genreOptions: DEFAULT_GENRE_OPTIONS,
}
const TITLE_DECORATION_PREFIX_RE = /^\s*[+*\u2022\u00B7\u25CF\u25BA\u25B6]+\s*/u
const TITLE_MARK_RE = /[\u2122\u00AE\u00A9\u2120]/g
const TITLE_LEADING_ARTICLE_RE = /^(?:a|an|the)\s+/i
const TEXT_COLLATOR = new Intl.Collator(undefined, { sensitivity: 'accent', numeric: true })
const TITLE_INDEX_OPTIONS = ['#', ...Array.from({ length: 26 }, (_, i) => String.fromCharCode(65 + i))]

/* ---------- pure helpers (safe outside component) ---------- */

function cleanLegendaryTitle(t: string) {
  // removes known leading markers from launcher output (+, *, bullets)
  return t.replace(/^\s*[+*•·●►▶]+\s*/u, '').trim();
}
// handy when comparing titles coming from Legendary
function normalizeTitleLocal(s: string) {
  return normalizeTitle(cleanLegendaryTitle(s));
}

function sanitizeTitleText(value?: string | null) {
  return String(value ?? '')
    .replace(TITLE_DECORATION_PREFIX_RE, '')
    .replace(TITLE_MARK_RE, '')
    .trim()
}

function significantTitleText(value?: string | null) {
  const sanitized = sanitizeTitleText(value)
  const withoutArticle = sanitized.replace(TITLE_LEADING_ARTICLE_RE, '').trim()
  return withoutArticle || sanitized
}

function titleIndexKey(value?: string | null) {
  const significant = significantTitleText(value)
  if (!significant) return '#'

  const normalized = significant.normalize('NFKD').replace(/[\u0300-\u036f]/g, '')
  const first = normalized.charAt(0).toUpperCase()
  return first >= 'A' && first <= 'Z' ? first : '#'
}

function normalizeManagedOptions(values: unknown, fallback: string[]) {
  const rawValues = Array.isArray(values) ? values : fallback
  const next: string[] = []

  for (const value of rawValues) {
    const normalized = String(value ?? '').trim()
    if (!normalized) continue
    if (next.some(existing => existing.localeCompare(normalized, undefined, { sensitivity: 'accent' }) === 0)) {
      continue
    }
    next.push(normalized)
  }

  if (!next.length) return [...fallback]
  return next.sort((a, b) => TEXT_COLLATOR.compare(a, b))
}

function mergeDistinctOptions(...groups: Array<Array<string | null | undefined> | undefined>) {
  const merged: string[] = []

  for (const group of groups) {
    for (const value of group ?? []) {
      const normalized = String(value ?? '').trim()
      if (!normalized) continue
      if (merged.some(existing => existing.localeCompare(normalized, undefined, { sensitivity: 'accent' }) === 0)) {
        continue
      }
      merged.push(normalized)
    }
  }

  return merged.sort((a, b) => TEXT_COLLATOR.compare(a, b))
}

function parseOptionEditorValue(value: string) {
  return value
    .split(/\r?\n/)
    .map(entry => entry.trim())
    .filter(Boolean)
}

function formatOptionEditorValue(values: string[]) {
  return values.join('\n')
}

function normalizeHexColorValue(raw: string | null | undefined, fallback: string) {
  const value = (raw ?? '').trim()
  return /^#[0-9a-fA-F]{6}$/.test(value) ? value.toLowerCase() : fallback.toLowerCase()
}

function parseHexColor(hex: string) {
  const normalized = normalizeHexColorValue(hex, '#000000')
  return {
    r: Number.parseInt(normalized.slice(1, 3), 16),
    g: Number.parseInt(normalized.slice(3, 5), 16),
    b: Number.parseInt(normalized.slice(5, 7), 16),
  }
}

function rgbaFromHex(hex: string, alpha: number) {
  const { r, g, b } = parseHexColor(hex)
  return `rgba(${r}, ${g}, ${b}, ${alpha})`
}

function mixHex(hex: string, targetHex: string, amount: number) {
  const from = parseHexColor(hex)
  const target = parseHexColor(targetHex)
  const ratio = Math.max(0, Math.min(1, amount))
  const mix = (start: number, end: number) => Math.round(start + (end - start) * ratio)
  return `#${[mix(from.r, target.r), mix(from.g, target.g), mix(from.b, target.b)]
    .map(value => value.toString(16).padStart(2, '0'))
    .join('')}`
}

function contrastTextForBackground(hex: string) {
  const { r, g, b } = parseHexColor(hex)
  const brightness = (r * 299 + g * 587 + b * 114) / 1000
  return brightness >= 150 ? '#132033' : '#ffffff'
}

function normalizeCustomizationSettings(
  settings?: Partial<CustomizationSettingsDto> | null,
): CustomizationSettingsDto {
  return {
    buttonColor: normalizeHexColorValue(settings?.buttonColor, DEFAULT_CUSTOMIZATION.buttonColor),
    textColor: normalizeHexColorValue(settings?.textColor, DEFAULT_CUSTOMIZATION.textColor),
    cardBackgroundColor: normalizeHexColorValue(
      settings?.cardBackgroundColor,
      DEFAULT_CUSTOMIZATION.cardBackgroundColor,
    ),
    sectionBackgroundColor: normalizeHexColorValue(
      settings?.sectionBackgroundColor,
      DEFAULT_CUSTOMIZATION.sectionBackgroundColor,
    ),
    backgroundImageName: settings?.backgroundImageName ?? null,
    backgroundImageDataUrl: settings?.backgroundImageDataUrl ?? null,
    availableBackgrounds: Array.isArray(settings?.availableBackgrounds)
      ? settings!.availableBackgrounds.filter((value): value is string => typeof value === 'string')
      : [],
    platformOptions: normalizeManagedOptions(settings?.platformOptions, DEFAULT_PLATFORM_OPTIONS),
    genreOptions: normalizeManagedOptions(settings?.genreOptions, DEFAULT_GENRE_OPTIONS),
  }
}

function applyCustomizationTheme(settings: CustomizationSettingsDto) {
  if (typeof document === 'undefined') return

  const root = document.documentElement
  const buttonStrong = mixHex(settings.buttonColor, '#000000', 0.14)
  const buttonText = contrastTextForBackground(settings.buttonColor)
  const textMuted = rgbaFromHex(settings.textColor, 0.72)
  const textFaint = rgbaFromHex(settings.textColor, 0.54)
  const sectionBg = rgbaFromHex(settings.sectionBackgroundColor, 0.78)
  const sectionBgStrong = rgbaFromHex(settings.sectionBackgroundColor, 0.92)
  const sectionBgSoft = rgbaFromHex(settings.sectionBackgroundColor, 0.68)
  const cardBg = rgbaFromHex(settings.cardBackgroundColor, 0.9)
  const cardBgStrong = rgbaFromHex(settings.cardBackgroundColor, 0.96)
  const cardBgSoft = rgbaFromHex(settings.cardBackgroundColor, 0.82)
  const hasBackgroundImage = !!settings.backgroundImageDataUrl

  root.style.setProperty('--bg-top', settings.sectionBackgroundColor)
  root.style.setProperty('--bg-bottom', settings.cardBackgroundColor)
  root.style.setProperty('--text', settings.textColor)
  root.style.setProperty('--text-muted', textMuted)
  root.style.setProperty('--text-faint', textFaint)
  root.style.setProperty('--border', rgbaFromHex(settings.textColor, 0.12))
  root.style.setProperty('--border-strong', rgbaFromHex(settings.textColor, 0.2))
  root.style.setProperty('--accent', settings.buttonColor)
  root.style.setProperty('--accent-strong', buttonStrong)
  root.style.setProperty('--accent-soft', rgbaFromHex(settings.buttonColor, 0.14))
  root.style.setProperty('--surface', sectionBg)
  root.style.setProperty('--surface-strong', cardBgStrong)
  root.style.setProperty('--surface-soft', sectionBgSoft)
  root.style.setProperty('--button-solid', settings.buttonColor)
  root.style.setProperty('--button-solid-strong', buttonStrong)
  root.style.setProperty('--button-solid-text', buttonText)
  root.style.setProperty('--button-tint', rgbaFromHex(settings.buttonColor, 0.14))
  root.style.setProperty('--button-tint-hover', rgbaFromHex(settings.buttonColor, 0.22))
  root.style.setProperty('--button-border', rgbaFromHex(settings.buttonColor, 0.34))
  root.style.setProperty('--button-shadow', rgbaFromHex(settings.buttonColor, 0.24))
  root.style.setProperty('--button-disabled-bg', rgbaFromHex('#6b7280', 0.82))
  root.style.setProperty('--section-bg', sectionBg)
  root.style.setProperty('--section-bg-strong', sectionBgStrong)
  root.style.setProperty('--section-bg-soft', sectionBgSoft)
  root.style.setProperty('--section-border', rgbaFromHex(settings.textColor, 0.1))
  root.style.setProperty('--section-border-strong', rgbaFromHex(settings.textColor, 0.16))
  root.style.setProperty('--card-bg', cardBg)
  root.style.setProperty('--card-bg-strong', cardBgStrong)
  root.style.setProperty('--card-bg-soft', cardBgSoft)
  root.style.setProperty('--card-border', rgbaFromHex(settings.textColor, 0.16))
  root.style.setProperty('--card-border-soft', rgbaFromHex(settings.textColor, 0.1))
  root.style.setProperty('--card-input-bg', rgbaFromHex(settings.cardBackgroundColor, 0.88))
  root.style.setProperty(
    '--app-background-overlay-top',
    rgbaFromHex(settings.sectionBackgroundColor, hasBackgroundImage ? 0.56 : 0.78),
  )
  root.style.setProperty(
    '--app-background-overlay-bottom',
    rgbaFromHex(settings.cardBackgroundColor, hasBackgroundImage ? 0.78 : 0.9),
  )
  root.style.setProperty(
    '--app-background-image',
    settings.backgroundImageDataUrl ? `url("${settings.backgroundImageDataUrl}")` : 'none',
  )
}

function fileToBase64(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader()
    reader.onerror = () => reject(reader.error ?? new Error('Could not read the selected file.'))
    reader.onload = () => {
      const result = typeof reader.result === 'string' ? reader.result : ''
      const base64 = result.includes(',') ? result.split(',', 2)[1] : ''
      if (!base64) {
        reject(new Error('Could not decode the selected file.'))
        return
      }
      resolve(base64)
    }
    reader.readAsDataURL(file)
  })
}

function parseCsv(text: string): { headers: string[], rows: string[][] } {
  const rows: string[][] = [];
  let i = 0, cur: string[] = [], field = '', inQuotes = false;

  const pushField = () => { cur.push(field); field = '' };
  const pushRow = () => { rows.push(cur); cur = [] };

  while (i < text.length) {
    const ch = text[i];

    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') { field += '"'; i += 2; continue } // escaped quote
        inQuotes = false; i++; continue;
      }
      field += ch; i++; continue;
    } else {
      if (ch === '"') { inQuotes = true; i++; continue }
      if (ch === ',') { pushField(); i++; continue }
      if (ch === '\r') { i++; continue }
      if (ch === '\n') { pushField(); pushRow(); i++; continue }
      field += ch; i++; continue;
    }
  }
  // last field/row
  pushField(); if (cur.length > 1 || (cur.length === 1 && cur[0] !== '')) pushRow();

  if (rows.length === 0) return { headers: [], rows: [] };
  const headers = rows[0].map(h => (h || '').trim());
  return { headers, rows: rows.slice(1) };
}

function stripBom(text: string) {
  return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text
}

function csvCell(value: unknown) {
  const raw = value == null ? '' : String(value)
  return /[",\r\n]/.test(raw) ? `"${raw.replace(/"/g, '""')}"` : raw
}

function buildCsv(columns: string[], rows: Array<Record<string, unknown>>) {
  const lines = [
    columns.map(csvCell).join(','),
    ...rows.map(row => columns.map(column => csvCell(row[column])).join(',')),
  ]
  return `\uFEFF${lines.join('\r\n')}`
}

function downloadCsv(filename: string, csvText: string) {
  const blob = new Blob([csvText], { type: 'text/csv;charset=utf-8;' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
}

function getField(obj: Record<string, string>, ...names: string[]): string | undefined {
  const key = Object.keys(obj).find(k => names.some(n => k.toLowerCase() === n.toLowerCase()));
  return key ? obj[key] : undefined;
}

function optionalCsvText(value?: string) {
  const trimmed = String(value ?? '').trim()
  return trimmed || undefined
}

type LibraryBackupMetadata = {
  platformOptions?: string[]
  genreOptions?: string[]
}

function buildLibraryBackupMetadataRows(settings: CustomizationSettingsDto) {
  return [
    {
      'Record Type': 'meta',
      'Meta Key': 'platformOptions',
      'Meta Value': JSON.stringify(settings.platformOptions),
    },
    {
      'Record Type': 'meta',
      'Meta Key': 'genreOptions',
      'Meta Value': JSON.stringify(settings.genreOptions),
    },
  ]
}

function extractLibraryBackupMetadata(rows: Record<string, string>[]): LibraryBackupMetadata {
  const metadata: LibraryBackupMetadata = {}

  for (const row of rows) {
    const recordType = optionalCsvText(getField(row, 'Record Type', 'record_type', 'Row Type', 'row_type'))
    if (recordType?.toLowerCase() !== 'meta') continue

    const key = optionalCsvText(getField(row, 'Meta Key', 'meta_key'))
    const value = optionalCsvText(getField(row, 'Meta Value', 'meta_value'))
    if (!key || !value) continue

    try {
      const parsed = JSON.parse(value)
      if (!Array.isArray(parsed)) continue
      if (key === 'platformOptions') metadata.platformOptions = normalizeManagedOptions(parsed, DEFAULT_PLATFORM_OPTIONS)
      if (key === 'genreOptions') metadata.genreOptions = normalizeManagedOptions(parsed, DEFAULT_GENRE_OPTIONS)
    } catch {
      continue
    }
  }

  return metadata
}

function parseBooleanCell(value?: string) {
  const trimmed = String(value ?? '').trim().toLowerCase()
  if (!trimmed) return undefined
  if (['1', 'true', 'yes', 'y'].includes(trimmed)) return true
  if (['0', 'false', 'no', 'n'].includes(trimmed)) return false
  return undefined
}

function parseIntegerCell(value?: string) {
  const trimmed = String(value ?? '').trim()
  if (!trimmed) return undefined
  const parsed = Number.parseInt(trimmed, 10)
  return Number.isFinite(parsed) ? parsed : undefined
}

function parsePositiveInteger(value?: string | number | null) {
  if (typeof value === 'number') {
    return Number.isSafeInteger(value) && value > 0 ? value : null
  }

  const trimmed = String(value ?? '').trim()
  if (!/^\d+$/.test(trimmed)) return null
  const parsed = Number.parseInt(trimmed, 10)
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : null
}

function buildHltbExternalUrl(id?: string | number | null) {
  const numericId = parsePositiveInteger(id)
  return numericId != null ? `https://howlongtobeat.com/game/${numericId}` : 'https://howlongtobeat.com/'
}

function buildGameFaqsSearchUrl(title?: string | null) {
  const query = String(title ?? '').trim()
  return query ? `https://gamefaqs.gamespot.com/search?game=${encodeURIComponent(query)}` : 'https://gamefaqs.gamespot.com/'
}

function todayFileDate() {
  const now = new Date()
  const year = now.getFullYear()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  return `${year}${month}${day}`
}

function toYearFromDateString(s?: string): number | null {
  if (!s) return null
  const m = String(s).match(/(\d{4})/)
  const y = m ? Number(m[1]) : NaN
  return Number.isFinite(y) ? y : null
}

function parseNumberCell(value?: string) {
  const trimmed = String(value ?? '').trim()
  if (!trimmed) return undefined
  const parsed = Number.parseFloat(trimmed)
  return Number.isFinite(parsed) ? parsed : undefined
}

function parsePlaytimeMinutesFromCsv(raw: Record<string, string>) {
  const minutes = parseIntegerCell(getField(
    raw,
    'Playtime Minutes',
    'playtime_minutes',
    'Time Played Minutes',
    'Minutes Played'
  ))
  if (minutes !== undefined) return minutes

  const hoursValue = parseNumberCell(getField(
    raw,
    'Playtime Hours',
    'playtime_hours',
    'Hours Played',
    'Time Played Hours'
  ))
  if (hoursValue === undefined) return undefined
  return Math.round(hoursValue * 60)
}

function parseReleaseYearFromCsv(raw: Record<string, string>) {
  const value = optionalCsvText(getField(
    raw,
    'Release Year',
    'release_year',
    'Year',
    'year',
    'Release Date',
    'release_date'
  ))
  if (!value) return undefined

  const extractedYear = toYearFromDateString(value)
  if (extractedYear != null) return extractedYear

  return parseIntegerCell(value)
}

function mapLibraryBackupCsvRow(raw: Record<string, string>): LibraryBackupImportRow | null {
  const title = optionalCsvText(getField(raw, 'Title', 'Game Title', 'title', 'canonical_title', 'name'))
  if (!title) return null

  const row: LibraryBackupImportRow = { title }
  const platform = optionalCsvText(getField(raw, 'Platform', 'platform', 'System', 'system', 'Console', 'console'))
  const beaten = parseBooleanCell(getField(raw, 'Beaten', 'Completed', 'beaten'))
  const genre = optionalCsvText(getField(raw, 'Genre', 'Genres', 'genre', 'genres'))
  const playtimeMinutes = parsePlaytimeMinutesFromCsv(raw)
  const lastPlayedAt = optionalCsvText(getField(raw, 'Last Played', 'Last Played At', 'last_played_at', 'last played'))
  const releaseYear = parseReleaseYearFromCsv(raw)
  const coverUrl = optionalCsvText(getField(raw, 'Cover URL', 'Cover Url', 'cover_url', 'cover'))
  const licenseTypeRaw = optionalCsvText(getField(raw, 'License Type', 'license_type'))
  const licenseSource = optionalCsvText(getField(raw, 'License Source', 'license_source'))
  const hidden = parseBooleanCell(getField(raw, 'Hidden', 'hidden'))
  const notes = optionalCsvText(getField(raw, 'Notes', 'notes'))
  const reviewRating = parseIntegerCell(getField(raw, 'Review Rating', 'review_rating'))
  const reviewText = optionalCsvText(getField(raw, 'Review Text', 'Review', 'review_text', 'review'))

  if (platform) row.platform = platform
  if (beaten !== undefined) row.beaten = beaten
  if (genre) row.genre = genre
  if (playtimeMinutes !== undefined) row.playtime_minutes = playtimeMinutes
  if (lastPlayedAt) row.last_played_at = lastPlayedAt
  if (releaseYear !== undefined) row.release_year = releaseYear
  if (coverUrl) row.cover_url = coverUrl
  if (licenseTypeRaw) {
    const lowered = licenseTypeRaw.toLowerCase()
    if (lowered === 'owned' || lowered === 'subscription' || lowered === 'free' || lowered === 'trial') {
      row.license_type = lowered
    }
  }
  if (licenseSource) row.license_source = licenseSource
  if (hidden !== undefined) row.hidden = hidden
  if (notes) row.notes = notes
  if (reviewRating !== undefined) row.review_rating = reviewRating
  if (reviewText) row.review_text = reviewText

  return row
}

function mapLibraryImportCsvRow(raw: Record<string, string>): LibraryCsvImportRow | null {
  const mapped = mapLibraryBackupCsvRow(raw)
  if (!mapped?.platform) return null
  return {
    ...mapped,
    title: mapped.title,
    platform: mapped.platform,
  }
}

function mapQueueBackupCsvRow(raw: Record<string, string>): QueueBackupImportRow | null {
  const title = optionalCsvText(getField(raw, 'Title', 'Game Title', 'title', 'canonical_title', 'name'))
  if (!title) return null

  const row: QueueBackupImportRow = { title }
  const queuePosition = parseIntegerCell(getField(raw, 'Queue Position', 'queue_position', 'Queue'))
  const platform = optionalCsvText(getField(raw, 'Platform', 'platform'))
  const queueIsPlaying = parseBooleanCell(getField(raw, 'Playing', 'Queue Playing', 'queue_is_playing'))
  const queueStartedAt = optionalCsvText(getField(raw, 'Started Playing', 'queue_started_at', 'Queue Started'))
  const queueFinishedAt = optionalCsvText(getField(raw, 'Finished', 'queue_finished_at', 'Queue Finished'))
  const playtimeMinutes = parseIntegerCell(getField(raw, 'Playtime Minutes', 'playtime_minutes', 'Time Played Minutes'))

  if (queuePosition !== undefined) row.queue_position = queuePosition
  if (platform) row.platform = platform
  if (queueIsPlaying !== undefined) row.queue_is_playing = queueIsPlaying
  if (queueStartedAt) row.queue_started_at = queueStartedAt
  if (queueFinishedAt) row.queue_finished_at = queueFinishedAt
  if (playtimeMinutes !== undefined) row.playtime_minutes = playtimeMinutes

  return row
}

function normalizeTitle(s: string) {
  return s
    .replace(/^\s*[+*•·●►▶]+\s*/u, '')
    .replace(/[\u2122\u00AE\u00A9\u2120]/g, '')
    .trim()
    .toLowerCase()
    .replace(/\s+/g, ' ')
}

function compareTextCaseInsensitive(a?: string | null, b?: string | null) {
  const left = String(a ?? '')
  const right = String(b ?? '')
  const cmp = TEXT_COLLATOR.compare(left, right)
  return cmp !== 0 ? cmp : left.localeCompare(right)
}

function compareTitleCaseInsensitive(a?: string | null, b?: string | null) {
  const cmp = compareTextCaseInsensitive(significantTitleText(a), significantTitleText(b))
  if (cmp !== 0) return cmp
  return compareTextCaseInsensitive(sanitizeTitleText(a), sanitizeTitleText(b))
}

function libraryIdentityKey(title: string, platform: string) {
  return `${normalizeTitle(title)}::${normalizeTitle(platform)}`
}

function summarizeLibraryCsvImportRow(row: LibraryCsvImportRow) {
  const parts: string[] = []
  if (row.genre) parts.push(`Genre: ${row.genre}`)
  if (row.release_year != null) parts.push(`Release: ${row.release_year}`)
  if (row.playtime_minutes != null) parts.push(`Playtime: ${hours(row.playtime_minutes) || `${row.playtime_minutes} min`}`)
  if (row.beaten) parts.push('Beaten')
  if (row.last_played_at) parts.push('Last played')
  if (row.license_type) parts.push(`License: ${row.license_type}`)
  if (row.notes) parts.push('Notes')
  if (row.review_rating != null || row.review_text) parts.push('Review')
  if (row.hidden) parts.push('Hidden')
  return parts.join(', ') || 'Title and platform only'
}

// --- License helpers ---
function normLicenseType(v?: string | null): 'owned' | 'subscription' | 'free' | 'trial' {
  const s = String(v ?? 'owned').toLowerCase()
  return (['owned','subscription','free','trial'] as const).includes(s as any) ? (s as any) : 'owned'
}
function licenseLabelOf(row: UiGameRow): string {
  const t = normLicenseType(row.license_type)
  if (t === 'subscription') return (row.license_source ?? '').trim() || 'Subscription'
  if (t === 'free') return 'Free'
  if (t === 'trial') return 'Trial'
  return 'Owned'
}

function localDateInputValue(value?: string | null) {
  if (!value) return ''
  const raw = String(value).trim()
  if (!raw) return ''
  const match = raw.match(/^(\d{4})-(\d{2})-(\d{2})/)
  if (match) return `${match[1]}-${match[2]}-${match[3]}`
  const d = new Date(raw)
  if (!Number.isFinite(d.getTime())) return ''
  const year = d.getFullYear()
  const month = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function toIsoDateOrEmpty(value: string) {
  return value ? new Date(`${value}T00:00:00Z`).toISOString() : ''
}

function formatDateLabel(value?: string | null) {
  const raw = localDateInputValue(value)
  if (!raw) return ''
  const [year, month, day] = raw.split('-').map(Number)
  return new Date(year, month - 1, day, 12, 0, 0).toLocaleDateString()
}

function todayDateInputValue() {
  const now = new Date()
  const year = now.getFullYear()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

function localDateAtNoon(value: string) {
  const [year, month, day] = value.split('-').map(Number)
  return new Date(year, month - 1, day, 12, 0, 0, 0)
}

function isWithinLastCalendarDays(value: string | null | undefined, days: number) {
  if (days <= 0) return false
  const raw = localDateInputValue(value)
  if (!raw) return false

  const target = localDateAtNoon(raw)
  if (!Number.isFinite(target.getTime())) return false

  const today = localDateAtNoon(todayDateInputValue())
  const diffDays = Math.floor((today.getTime() - target.getTime()) / 86400000)
  return diffDays >= 0 && diffDays < days
}

function parseAddedQueueMinutes(hoursText: string, minutesText: string) {
  const hours = Number.parseInt(hoursText || '0', 10)
  const minutes = Number.parseInt(minutesText || '0', 10)
  const safeHours = Number.isFinite(hours) ? Math.max(0, hours) : 0
  const safeMinutes = Number.isFinite(minutes) ? Math.max(0, minutes) : 0
  return (safeHours * 60) + safeMinutes
}

function formatQueueClearEstimate(seconds?: number | null) {
  if (!seconds || seconds <= 0) return '—'

  const totalMinutes = Math.round(seconds / 60)
  const hours = Math.floor(totalMinutes / 60)
  const minutes = totalMinutes % 60

  if (hours > 0) {
    if (minutes > 0) return `${hours}h ${minutes}m`
    return `${hours}h`
  }
  return `${minutes}m`
}

function normalizeIgdb(raw: any): Npv {
  const r: any = raw || {}
  const title = r.title ?? r.name ?? undefined
  const igdb_url = r.igdb_url ?? r.url ?? null

  const cover_url =
    r.cover_url ??
    (r.cover && (r.cover.url || r.cover.image_id ? (r.cover.url ?? null) : null)) ??
    null

  let genres: string[] | undefined
  if (Array.isArray(r.genres)) {
    genres = r.genres.map((g: any) => typeof g === 'string' ? g : (g?.name ?? '')).filter(Boolean)
  }

  let release_year: number | null = null
  if (typeof r.release_year === 'number') release_year = r.release_year
  else if (r.first_release_date) {
    const d = new Date((Number(r.first_release_date) || 0) * 1000)
    release_year = Number.isFinite(d.getTime()) ? d.getUTCFullYear() : null
  }

  const num = (v: any) => {
    const n = Number(v); return Number.isFinite(n) ? n : null
  }

  const igdb_id = r.igdb_id ?? r.id ?? undefined
  const agg_rating = num(r.agg_rating ?? r.aggregated_rating ?? r.total_rating)
  const user_rating = num(r.user_rating ?? r.rating)

  const ttb = r.time_to_beat || r.ttb || {}
  const ttb_main = num(r.ttb_main ?? ttb.normally ?? ttb.main)
  const ttb_complete = num(r.ttb_complete ?? ttb.completely ?? ttb.complete)
  const ttb_extra = num(r.ttb_extra ?? ttb.hastily ?? ttb.extra)
  const ttb_count = num(r.ttb_count ?? r.time_to_beat_count)

  return {
    igdb_id, igdb_url, title, cover_url, genres, release_year,
    summary: r.summary ?? null,
    storyline: r.storyline ?? null,
    agg_rating, user_rating,
    ttb_main, ttb_extra, ttb_complete, ttb_count,
  }
}

/** Merge missing non-identity fields, never overwrite igdb_id/title. */
function safeFill(base: Npv, extra: Partial<Npv>): Npv {
  const out: Npv = { ...base }
  const keys: (keyof Npv)[] = [
    'igdb_url','cover_url','genres','release_year','summary','storyline',
    'agg_rating','user_rating','ttb_main','ttb_extra','ttb_complete','ttb_count'
  ]
  for (const k of keys) {
    const bv = (out as any)[k]
    const ev = (extra as any)[k]
    if ((bv === null || bv === undefined) && ev !== undefined && ev !== null) {
      ;(out as any)[k] = ev
    }
  }
  if (!out.title && extra.title) out.title = extra.title
  if (!out.igdb_id && extra.igdb_id != null) out.igdb_id = extra.igdb_id
  return out
}

function normPunctInsensitive(s: string) {
  return s
    .replace(/^\s*[+*•·●►▶]+\s*/u, '')
    .replace(/[\u2122\u00AE\u00A9\u2120]/g, '')
    .toLowerCase()
    .replace(/[\s'":;.,!?-]+/g, '')
}

function dedupeAndRankCandidates(cands: Npv[], queryTitle: string): Npv[] {
  const byId = new Map<string | number, Npv>()
  const byKey = new Map<string, Npv>()

  for (const c of cands) {
    if (c.igdb_id != null) {
      if (!byId.has(c.igdb_id)) byId.set(c.igdb_id, c)
    } else {
      const k = (c.title ?? '') + '|' + (c.release_year ?? '')
      if (!byKey.has(k)) byKey.set(k, c)
    }
  }
  const deduped = [...byId.values(), ...byKey.values()]

  const qtNorm = normPunctInsensitive(queryTitle)
  return deduped.sort((a, b) => {
    const ta = a.title ?? ''
    const tb = b.title ?? ''
    const exactA = ta === queryTitle ? 1 : 0
    const exactB = tb === queryTitle ? 1 : 0
    if (exactA !== exactB) return exactB - exactA
    const psA = normPunctInsensitive(ta) === qtNorm ? 1 : 0
    const psB = normPunctInsensitive(tb) === qtNorm ? 1 : 0
    if (psA !== psB) return psB - psA
    return 0
  })
}

/* ---------- IGDB via backend commands ONLY (with arg probing) ---------- */
async function getIgdbCandidates(
  qTitle: string,
  clientId: string,
  clientSecret: string
): Promise<Npv[]> {
  try {
    const arr = await lookupIgdbCandidates(qTitle, clientId, clientSecret)
    if (Array.isArray(arr) && arr.length) {
      return dedupeAndRankCandidates(arr.map(normalizeIgdb), qTitle)
    }
  } catch (e) {
    console.warn('[IGDB] candidates error via lib/api:', e)
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const shapes: any[] = [
      { title: qTitle, clientId, clientSecret },
      { query: qTitle, clientId, clientSecret },
      { q: qTitle, clientId, clientSecret },
      { title: qTitle, auth: { clientId, clientSecret } },
      { query: qTitle, auth: { clientId, clientSecret } },
      { q: qTitle, auth: { clientId, clientSecret } },
    ]

    for (const payload of shapes) {
      try {
        const raw = await invoke('lookup_igdb_candidates', payload)
        if (Array.isArray(raw) && raw.length) {
          return dedupeAndRankCandidates(raw.map(normalizeIgdb), qTitle)
        }
      } catch {}
    }
  } catch (e) {
    console.warn('[IGDB] direct invoke unavailable:', e)
  }

  try {
    const pv = await lookupIgdbPreview(qTitle, clientId, clientSecret)
    return pv ? dedupeAndRankCandidates([normalizeIgdb(pv)], qTitle) : []
  } catch {
    return []
  }
}

async function previewIgdbFlexible(
  title: string,
  clientId: string,
  clientSecret: string
): Promise<Npv | null> {
  // Try your lib/api wrapper first
  try {
    const pv: any = await lookupIgdbPreview(title, clientId, clientSecret)
    if (pv) return normalizeIgdb(pv)
  } catch (e) {
    console.warn('[IGDB] preview via lib/api failed:', e)
  }

  // Then probe common Tauri arg shapes
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    const shapes: any[] = [
      { title, clientId, clientSecret },
      { query: title, clientId, clientSecret },
      { title, auth: { clientId, clientSecret } },
      { query: title, auth: { clientId, clientSecret } },
    ]
    for (const payload of shapes) {
      try {
        const raw: any = await invoke('lookup_igdb_preview', payload)
        if (raw) return normalizeIgdb(raw)
      } catch {
        /* try next */
      }
    }
  } catch (e) {
    console.warn('[IGDB] direct invoke unavailable:', e)
  }

  return null
}

/* ---------- component ---------- */

export default function App() {
  const [games, setGames] = useState<UiGameRow[]>([])
  const [title, setTitle] = useState('')
  const [platform, setPlatform] = useState('Manual')
  const [beaten, setBeaten] = useState(false)
  const [genreValue, setGenreValue] = useState('')

  // NEW: quick-add license state
  const [addLicenseType, setAddLicenseType] = useState<'owned' | 'subscription' | 'free' | 'trial'>('owned')
  const [addLicenseSource, setAddLicenseSource] = useState('')

  // Epic (Legendary) import modal state
  const [showEpicImport, setShowEpicImport] = useState(false)
  const [legendaryPath, setLegendaryPath] = useState('')
  const [epicOwned, setEpicOwned] = useState<{ title: string, appName: string }[]>([])
  const [epicSelected, setEpicSelected] = useState<Set<number>>(new Set())
  const [epicSkipDupes, setEpicSkipDupes] = useState(true)
  const [epicImporting, setEpicImporting] = useState(false)

  // Steam import
  const [showImport, setShowImport] = useState(false)
  const [steamApiKey, setSteamApiKey] = useState('')
  const [steamProfile, setSteamProfile] = useState('')
  const [importing, setImporting] = useState(false)
  const [steamSyncing, setSteamSyncing] = useState(false)

  // Settings (Twitch creds)
  const [screenView, setScreenView] = useState<ScreenView>('library')
  const [showSettings, setShowSettings] = useState(false)
  const [showStats, setShowStats] = useState(false)
  const [showRandomGame, setShowRandomGame] = useState(false)
  const [showLibraryBackup, setShowLibraryBackup] = useState(false)
  const [showQueueBackup, setShowQueueBackup] = useState(false)
  const [showCsvImport, setShowCsvImport] = useState(false)
  const [libraryBackupBusy, setLibraryBackupBusy] = useState(false)
  const [queueBackupBusy, setQueueBackupBusy] = useState(false)
  const [libraryBackupPath, setLibraryBackupPath] = useState('')
  const [queueBackupPath, setQueueBackupPath] = useState('')
  const [twId, setTwId] = useState('')
  const [twSecret, setTwSecret] = useState('')
  const [customization, setCustomization] = useState<CustomizationSettingsDto>(DEFAULT_CUSTOMIZATION)
  const [customizationDraft, setCustomizationDraft] = useState<CustomizationSettingsDto>(DEFAULT_CUSTOMIZATION)
  const [platformOptionsDraftText, setPlatformOptionsDraftText] = useState(formatOptionEditorValue(DEFAULT_CUSTOMIZATION.platformOptions))
  const [genreOptionsDraftText, setGenreOptionsDraftText] = useState(formatOptionEditorValue(DEFAULT_CUSTOMIZATION.genreOptions))
  const [customizationSaving, setCustomizationSaving] = useState(false)
  const [backgroundUploading, setBackgroundUploading] = useState(false)

  // Editing
  const [editingId, setEditingId] = useState<string | null>(null)
  const [edit, setEdit] = useState<EditRow | null>(null)

  // Filters + sort
  const [q, setQ] = useState('')
  const [fTitleIndex, setFTitleIndex] = useState('')
  const [fPlatform, setFPlatform] = useState<string>('')
  const [fBeaten, setFBeaten] = useState<string>('')
  const [fGenre, setFGenre] = useState<string>('')
  const [fMinHours, setFMinHours] = useState<string>('')
  const [fLicense, setFLicense] = useState<string>('') // '', 'owned','subscription','free','trial'
  const [fEnriched, setFEnriched] = useState<string>('') // '', 'yes', 'no'
  const [fReviewed, setFReviewed] = useState<string>('') // '', 'yes', 'no'
  const [showHidden, setShowHidden] = useState(false)
  const [sortBy, setSortBy] = useState<SortKey>('')
  const [randomPlatform, setRandomPlatform] = useState('')
  const [randomBeaten, setRandomBeaten] = useState<RandomBeatenFilter>('')
  const [randomGenre, setRandomGenre] = useState('')
  const [randomTtb, setRandomTtb] = useState<RandomTtbFilter>('')
  const [randomPicking, setRandomPicking] = useState(false)

  // Pagination
  const [pageSize, setPageSize] = useState<number>(25)
  const [page, setPage] = useState<number>(1)

  // Selection + bulk
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [bulkAction, setBulkAction] = useState<BulkAction>('')

  // Progress overlay
  const [prog, setProg] = useState<Prog | null>(null)
  const [progCollapsed, setProgCollapsed] = useState(false)

  // Details card
  const [card, setCard] = useState<FullGame | null>(null)
  const [cardTab, setCardTab] = useState<CardTab>('details')
  const [notesDraft, setNotesDraft] = useState<string>('')
  const [notesSaving, setNotesSaving] = useState(false)
  const [reviewDraft, setReviewDraft] = useState<string>('')
  const [reviewRatingDraft, setReviewRatingDraft] = useState<number | null>(null)
  const [reviewSaving, setReviewSaving] = useState(false)
  const [cardIgdbIdInput, setCardIgdbIdInput] = useState('')
  const [cardIgdbApplying, setCardIgdbApplying] = useState(false)
  const [cardHltbIdInput, setCardHltbIdInput] = useState('')
  const [cardHltbApplying, setCardHltbApplying] = useState(false)
  const [queueOrderDrafts, setQueueOrderDrafts] = useState<Record<string, string>>({})
  const [dragQueueId, setDragQueueId] = useState<string | null>(null)
  const [queueDropId, setQueueDropId] = useState<string | null>(null)
  const [queueTimeDialog, setQueueTimeDialog] = useState<QueueTimeDialog>(null)
  const [queueFinishedTrackerDialog, setQueueFinishedTrackerDialog] = useState<QueueFinishedTrackerDialog>(null)
  const [queueTimeSaving, setQueueTimeSaving] = useState(false)
  const queueDropIdRef = useRef<string | null>(null)

  // IGDB chooser
  const [igdbOffer, setIgdbOffer] = useState<OfferState>(null)
  const [hydratingIdx, setHydratingIdx] = useState<number | null>(null)

  // Library CSV import state
  const [csvImportRows, setCsvImportRows] = useState<LibraryCsvImportRow[]>([])
  const [csvImportSelected, setCsvImportSelected] = useState<Set<number>>(new Set())
  const [csvImportRejectedCount, setCsvImportRejectedCount] = useState(0)
  const [csvImportFileName, setCsvImportFileName] = useState('')
  const [csvImporting, setCsvImporting] = useState(false)

  // Amazon library import state
  const [showAmazonImport, setShowAmazonImport] = useState(false)
  const [amazonRows, setAmazonRows] = useState<AmazonImportRow[]>([])
  const [amazonSelected, setAmazonSelected] = useState<Set<number>>(new Set())
  const [amazonSkipDupes, setAmazonSkipDupes] = useState(true)
  const [amazonImporting, setAmazonImporting] = useState(false)
  const [amazonDbPath, setAmazonDbPath] = useState('')
  const [showGogImport, setShowGogImport] = useState(false)
  const [gogDbPath, setGogDbPath] = useState('')
  const [gogImporting, setGogImporting] = useState(false)
  const [gogSyncing, setGogSyncing] = useState(false)
  
  // Xbox (Microsoft) device-code login
const [xboxClientId, setXboxClientId] = useState<string>(localStorage.getItem('xboxClientId') || '');
const [showXboxLogin, setShowXboxLogin] = useState(false);
const [xboxDC, setXboxDC] = useState<null | {
  device_code: string;
  user_code: string;
  verification_uri: string;
  verification_uri_complete?: string;
  interval: number;
  message: string;
}>(null);

// persist client ID
useEffect(() => {
  localStorage.setItem('xboxClientId', xboxClientId || '');
}, [xboxClientId]);

  async function loadStoredSteamCreds() {
    try {
      const saved = await getSteamCreds()
      setSteamApiKey(saved.apiKey || '')
      setSteamProfile(saved.profile || '')
    } catch {}
  }

  async function persistSteamCreds(apiKey: string, profile: string) {
    try {
      await saveSteamCreds({ apiKey: apiKey.trim(), profile: profile.trim() })
    } catch {}
  }

  useEffect(() => {
    refresh()
    loadCustomization()
    loadStoredSteamCreds()
  }, [])

  useEffect(() => {
    applyCustomizationTheme(showSettings ? customizationDraft : customization)
  }, [customization, customizationDraft, showSettings])

  useEffect(() => {
    const unsubs: any[] = []
    const w: any = (window as any).__TAURI__
    if (w?.event?.listen) {
      w.event
        .listen('igdb_enrich_progress', (e: any) => {
          const p = (e?.payload ?? e) as any
          try { console.log('[igdb_enrich_progress]', p) } catch {}
          setProg({
            done: Number(p?.done) || 0,
            total: Number(p?.total) || 0,
            title: typeof p?.title === 'string' ? p.title : '',
          })
        })
        .then((unsub: any) => unsubs.push(unsub))
        .catch(() => {})
    }
    return () => { unsubs.forEach(u => { try { u() } catch {} }) }
  }, [])

  async function loadCustomization() {
    try {
      const settings = normalizeCustomizationSettings(await getCustomizationSettings())
      setCustomization(settings)
      setCustomizationDraft(settings)
      setPlatformOptionsDraftText(formatOptionEditorValue(settings.platformOptions))
      setGenreOptionsDraftText(formatOptionEditorValue(settings.genreOptions))
    } catch (e) {
      console.warn('Could not load customization settings:', e)
      setCustomization(DEFAULT_CUSTOMIZATION)
      setCustomizationDraft(DEFAULT_CUSTOMIZATION)
      setPlatformOptionsDraftText(formatOptionEditorValue(DEFAULT_CUSTOMIZATION.platformOptions))
      setGenreOptionsDraftText(formatOptionEditorValue(DEFAULT_CUSTOMIZATION.genreOptions))
    }
  }

  async function applyLibraryBackupMetadata(metadata: LibraryBackupMetadata) {
    if (!metadata.platformOptions && !metadata.genreOptions) return false

    const currentSettings = normalizeCustomizationSettings(await getCustomizationSettings())
    const savedSettings = normalizeCustomizationSettings(
      await saveCustomizationSettings({
        buttonColor: currentSettings.buttonColor,
        textColor: currentSettings.textColor,
        cardBackgroundColor: currentSettings.cardBackgroundColor,
        sectionBackgroundColor: currentSettings.sectionBackgroundColor,
        backgroundImageName: currentSettings.backgroundImageName ?? null,
        platformOptions: metadata.platformOptions ?? currentSettings.platformOptions,
        genreOptions: metadata.genreOptions ?? currentSettings.genreOptions,
      }),
    )

    setCustomization(savedSettings)
    setCustomizationDraft(savedSettings)
    setPlatformOptionsDraftText(formatOptionEditorValue(savedSettings.platformOptions))
    setGenreOptionsDraftText(formatOptionEditorValue(savedSettings.genreOptions))
    return true
  }

  async function refresh() { setGames(await listGames()) }
  function patchGameReviewInList(id: string, review_rating: number | null, review_text: string | null) {
    const has_review = review_rating != null || !!review_text?.trim()
    setGames(prev => prev.map(game => game.id === id ? { ...game, review_rating, has_review } : game))
  }

  async function prepareBackupPath(kind: 'library' | 'queue') {
    const filename = `gamelexicon-${kind}-backup-${todayFileDate()}.csv`
    return await defaultBackupExportPath(filename)
  }

  async function openLibraryBackupModal() {
    try {
      setLibraryBackupBusy(true)
      setLibraryBackupPath(await prepareBackupPath('library'))
      setShowLibraryBackup(true)
    } catch (e: any) {
      alert('Could not prepare the library backup path: ' + (e?.message ?? String(e)))
    } finally {
      setLibraryBackupBusy(false)
    }
  }

  async function openQueueBackupModal() {
    try {
      setQueueBackupBusy(true)
      setQueueBackupPath(await prepareBackupPath('queue'))
      setShowQueueBackup(true)
    } catch (e: any) {
      alert('Could not prepare the queue backup path: ' + (e?.message ?? String(e)))
    } finally {
      setQueueBackupBusy(false)
    }
  }

  async function downloadLibraryBackup() {
    try {
      setLibraryBackupBusy(true)
      const settings = normalizeCustomizationSettings(await getCustomizationSettings())
      const rows = await exportLibraryBackupData()
      const csv = buildCsv(
        [
          'Record Type',
          'Meta Key',
          'Meta Value',
          'Title',
          'Platform',
          'Beaten',
          'Genre',
          'Playtime Minutes',
          'Last Played',
          'Release Year',
          'Cover URL',
          'License Type',
          'License Source',
          'Hidden',
          'Notes',
          'Review Rating',
          'Review Text',
        ],
        [
          ...buildLibraryBackupMetadataRows(settings),
          ...rows.map(row => ({
            'Record Type': 'game',
            'Meta Key': '',
            'Meta Value': '',
            'Title': row.title,
            'Platform': row.platform,
            'Beaten': row.beaten,
            'Genre': row.genre ?? '',
            'Playtime Minutes': row.playtime_minutes ?? '',
            'Last Played': row.last_played_at ?? '',
            'Release Year': row.release_year ?? '',
            'Cover URL': row.cover_url ?? '',
            'License Type': row.license_type ?? '',
            'License Source': row.license_source ?? '',
            'Hidden': row.hidden,
            'Notes': row.notes ?? '',
            'Review Rating': row.review_rating ?? '',
            'Review Text': row.review_text ?? '',
          })),
        ]
      )
      const path = libraryBackupPath.trim()
      if (path) {
        const savedTo = await saveBackupFile(path, csv)
        setLibraryBackupPath(savedTo)
        alert(`Library backup saved to:\n${savedTo}`)
      } else {
        downloadCsv(`gamelexicon-library-backup-${todayFileDate()}.csv`, csv)
      }
    } catch (e: any) {
      alert('Could not export library backup: ' + (e?.message ?? String(e)))
    } finally {
      setLibraryBackupBusy(false)
    }
  }

  async function importLibraryBackupFile(file: File) {
    try {
      setLibraryBackupBusy(true)
      const text = stripBom(await file.text())
      const parsed = parseCsv(text)
      if (!parsed.headers.length) {
        throw new Error('The CSV is missing a header row.')
      }

      const rawRows = parsed.rows
        .map((values) => Object.fromEntries(parsed.headers.map((header, idx) => [header, values[idx] ?? ''])))
      const metadata = extractLibraryBackupMetadata(rawRows)
      const restoredLists = !!metadata.platformOptions || !!metadata.genreOptions

      const mapped = rawRows
        .map((raw) => mapLibraryBackupCsvRow(raw))
        .filter((row): row is LibraryBackupImportRow => !!row)

      if (!mapped.length) {
        if (!restoredLists) {
          throw new Error('No valid library rows were found. Title is required.')
        }
        await applyLibraryBackupMetadata(metadata)
        alert('Library backup import complete.\nCreated: 0\nUpdated: 0\nSkipped: 0\nRestored option lists: yes')
        return
      }

      const result = await importLibraryBackupData(mapped)
      if (restoredLists) {
        await applyLibraryBackupMetadata(metadata)
      }
      await refresh()
      alert(
        `Library backup import complete.\nCreated: ${result.created}\nUpdated: ${result.updated}\nSkipped: ${result.skipped}\nRestored option lists: ${restoredLists ? 'yes' : 'no'}`
      )
    } catch (e: any) {
      alert('Could not import library backup: ' + (e?.message ?? String(e)))
    } finally {
      setLibraryBackupBusy(false)
    }
  }

  async function downloadQueueBackup() {
    try {
      setQueueBackupBusy(true)
      const rows = await exportQueueBackupData()
      const csv = buildCsv(
        [
          'Queue Position',
          'Title',
          'Platform',
          'Playing',
          'Started Playing',
          'Finished',
          'Playtime Minutes',
        ],
        rows.map(row => ({
          'Queue Position': row.queue_position,
          'Title': row.title,
          'Platform': row.platform,
          'Playing': row.queue_is_playing,
          'Started Playing': row.queue_started_at ?? '',
          'Finished': row.queue_finished_at ?? '',
          'Playtime Minutes': row.playtime_minutes ?? '',
        }))
      )
      const path = queueBackupPath.trim()
      if (path) {
        const savedTo = await saveBackupFile(path, csv)
        setQueueBackupPath(savedTo)
        alert(`Queue backup saved to:\n${savedTo}`)
      } else {
        downloadCsv(`gamelexicon-queue-backup-${todayFileDate()}.csv`, csv)
      }
    } catch (e: any) {
      alert('Could not export queue backup: ' + (e?.message ?? String(e)))
    } finally {
      setQueueBackupBusy(false)
    }
  }

  async function importQueueBackupFile(file: File) {
    const confirmed = window.confirm(
      'Restore the queue from this CSV? This will replace the current queue order and queue state.'
    )
    if (!confirmed) return

    try {
      setQueueBackupBusy(true)
      const text = stripBom(await file.text())
      const parsed = parseCsv(text)
      if (!parsed.headers.length) {
        throw new Error('The CSV is missing a header row.')
      }

      const mapped = parsed.rows
        .map((values) => {
          const raw = Object.fromEntries(parsed.headers.map((header, idx) => [header, values[idx] ?? '']))
          return mapQueueBackupCsvRow(raw)
        })
        .filter((row): row is QueueBackupImportRow => !!row)

      if (!mapped.length) {
        throw new Error('No valid queue rows were found. Title is required.')
      }

      const result = await importQueueBackupData(mapped)
      setQueueOrderDrafts({})
      await refresh()
      await refreshCardIfOpen()
      alert(`Queue restore complete.\nRestored: ${result.restored}\nCreated missing games: ${result.created}\nSkipped: ${result.skipped}`)
    } catch (e: any) {
      alert('Could not restore queue backup: ' + (e?.message ?? String(e)))
    } finally {
      setQueueBackupBusy(false)
    }
  }

  function resetLibraryCsvImport() {
    setCsvImportRows([])
    setCsvImportSelected(new Set())
    setCsvImportRejectedCount(0)
    setCsvImportFileName('')
  }

  function closeLibraryCsvImport() {
    if (csvImporting) return
    resetLibraryCsvImport()
    setShowCsvImport(false)
  }

  function toggleLibraryCsvImportRow(i: number) {
    setCsvImportSelected(prev => {
      const next = new Set(prev)
      if (next.has(i)) next.delete(i)
      else next.add(i)
      return next
    })
  }

  function toggleLibraryCsvImportSelectAll() {
    setCsvImportSelected(prev => {
      if (prev.size === csvImportRows.length) return new Set()
      return new Set(csvImportRows.map((_, i) => i))
    })
  }

  async function handleLibraryCsvFile(file: File) {
    try {
      const text = stripBom(await file.text())
      const parsed = parseCsv(text)
      if (!parsed.headers.length || !parsed.rows.length) {
        alert('Could not read CSV (no rows).')
        return
      }

      const rows: LibraryCsvImportRow[] = []
      let rejected = 0
      for (const values of parsed.rows) {
        const raw = Object.fromEntries(parsed.headers.map((header, idx) => [header, values[idx] ?? '']))
        const mapped = mapLibraryImportCsvRow(raw)
        if (mapped) rows.push(mapped)
        else rejected += 1
      }

      if (!rows.length) {
        alert('No recognizable rows were found. This importer requires both Title and Platform columns.')
        return
      }

      rows.sort((a, b) => {
        const titleCompare = compareTitleCaseInsensitive(a.title, b.title)
        if (titleCompare !== 0) return titleCompare
        return compareTextCaseInsensitive(a.platform, b.platform)
      })

      setCsvImportRows(rows)
      setCsvImportSelected(new Set(rows.map((_, i) => i)))
      setCsvImportRejectedCount(rejected)
      setCsvImportFileName(file.name)
    } catch (e: any) {
      alert('Could not read CSV: ' + (e?.message ?? String(e)))
    }
  }

  async function applyLibraryCsvImportFields(id: string, row: LibraryCsvImportRow) {
    const changes: {
      id: string
      playtime_minutes?: number
      last_played_at?: string
      cover_url?: string
      release_year?: number
    } = { id }

    if (row.playtime_minutes !== undefined) changes.playtime_minutes = row.playtime_minutes
    if (row.last_played_at) changes.last_played_at = row.last_played_at
    if (row.cover_url) changes.cover_url = row.cover_url
    if (row.release_year !== undefined) changes.release_year = row.release_year

    if (Object.keys(changes).length > 1) {
      await updateGame(changes)
    }
    if (row.hidden === true) {
      await hideGame(id)
    }
    if (row.notes) {
      await saveGameNotes(id, row.notes)
    }
    if (row.review_rating !== undefined || row.review_text) {
      await saveGameReview(id, row.review_rating ?? null, row.review_text ?? null)
    }
  }

  async function runLibraryCsvImport() {
    if (csvImportSelected.size === 0) {
      alert('No rows selected.')
      return
    }

    setCsvImporting(true)
    try {
      const existing = new Set(games.map(game => libraryIdentityKey(game.title, game.platform)))
      const selected = csvImportRows
        .map((row, i) => ({ row, i }))
        .filter(entry => csvImportSelected.has(entry.i))

      let added = 0
      let skipped = 0
      let errored = 0

      for (const { row } of selected) {
        const key = libraryIdentityKey(row.title, row.platform)
        if (existing.has(key)) {
          skipped += 1
          continue
        }

        try {
          const id = await addManualGame({
            canonicalTitle: row.title,
            platform: row.platform,
            beaten: row.beaten ?? false,
            genre: row.genre,
            licenseType: row.license_type,
            licenseSource: row.license_source,
          })
          await applyLibraryCsvImportFields(id, row)
          existing.add(key)
          added += 1
        } catch (e: any) {
          console.warn('[CSV Import] failed to add:', row.title, row.platform, e)
          errored += 1
        }
      }

      await refresh()
      await refreshCardIfOpen()
      alert(`CSV import complete.\nAdded: ${added}\nSkipped: ${skipped}\nErrors: ${errored}`)
      resetLibraryCsvImport()
      setShowCsvImport(false)
    } finally {
      setCsvImporting(false)
    }
  }

  async function addToQueue(row: UiGameRow) {
    try {
      await addGameToQueue(row.id)
      await refresh()
      await refreshCardIfOpen(row.id)
    } catch (e: any) {
      alert('Could not add to queue: ' + (e?.message ?? String(e)))
    }
  }

  async function removeFromQueue(row: UiGameRow) {
    try {
      await removeGameFromQueue(row.id)
      await refresh()
      await refreshCardIfOpen(row.id)
      setQueueOrderDrafts(prev => {
        const next = { ...prev }
        delete next[row.id]
        return next
      })
    } catch (e: any) {
      alert('Could not remove from queue: ' + (e?.message ?? String(e)))
    }
  }

  async function persistQueueOrder(ids: string[]) {
    try {
      await reorderQueue(ids)
      setQueueOrderDrafts({})
      await refresh()
      await refreshCardIfOpen()
    } catch (e: any) {
      alert('Could not reorder queue: ' + (e?.message ?? String(e)))
    } finally {
      setDragQueueId(null)
      setQueueDropId(null)
      queueDropIdRef.current = null
    }
  }

  function updateQueueDragTarget(clientX: number, clientY: number) {
    const hovered = document.elementFromPoint(clientX, clientY) as HTMLElement | null
    const row = hovered?.closest?.('tr[data-queue-id]') as HTMLElement | null
    const nextId = row?.dataset.queueId ?? null
    queueDropIdRef.current = nextId
    setQueueDropId(nextId)
  }

  async function finishQueueDrag(sourceId: string) {
    const targetId = queueDropIdRef.current
    setDragQueueId(null)
    setQueueDropId(null)
    queueDropIdRef.current = null

    if (!targetId || targetId === sourceId) return

    const ids = queueGames.map(game => game.id)
    const fromIndex = ids.indexOf(sourceId)
    const toIndex = ids.indexOf(targetId)
    if (fromIndex < 0 || toIndex < 0 || fromIndex === toIndex) return

    const nextIds = [...ids]
    const [moved] = nextIds.splice(fromIndex, 1)
    nextIds.splice(toIndex, 0, moved)
    await persistQueueOrder(nextIds)
  }

  async function moveQueueItemToPosition(id: string, nextPosition: number) {
    if (!Number.isFinite(nextPosition)) {
      setQueueOrderDrafts(prev => {
        const next = { ...prev }
        delete next[id]
        return next
      })
      return
    }
    const ids = queueGames.map(game => game.id)
    const currentIndex = ids.indexOf(id)
    if (currentIndex < 0) return

    const clampedPosition = Math.min(ids.length, Math.max(1, nextPosition))
    const targetIndex = clampedPosition - 1
    if (targetIndex === currentIndex) {
      setQueueOrderDrafts(prev => {
        const next = { ...prev }
        delete next[id]
        return next
      })
      return
    }

    const nextIds = [...ids]
    const [moved] = nextIds.splice(currentIndex, 1)
    nextIds.splice(targetIndex, 0, moved)
    await persistQueueOrder(nextIds)
  }

  async function moveQueueItemByDelta(id: string, delta: number) {
    const ids = queueGames.map(game => game.id)
    const currentIndex = ids.indexOf(id)
    if (currentIndex < 0) return

    const targetIndex = Math.min(ids.length - 1, Math.max(0, currentIndex + delta))
    if (targetIndex === currentIndex) return

    const nextIds = [...ids]
    const [moved] = nextIds.splice(currentIndex, 1)
    nextIds.splice(targetIndex, 0, moved)
    await persistQueueOrder(nextIds)
  }

  async function toggleQueuePlaying(row: UiGameRow, isPlaying: boolean) {
    try {
      await updateGame({
        id: row.id,
        queue_is_playing: isPlaying,
        queue_started_at: isPlaying && !localDateInputValue(row.queue_started_at)
          ? toIsoDateOrEmpty(todayDateInputValue())
          : undefined,
      })
      await refresh()
      await refreshCardIfOpen(row.id)
    } catch (e: any) {
      alert('Could not update queue status: ' + (e?.message ?? String(e)))
    }
  }

  async function updateQueueDate(
    id: string,
    field: 'queue_started_at' | 'queue_finished_at',
    value: string
  ) {
    try {
      await updateGame({
        id,
        [field]: value ? toIsoDateOrEmpty(value) : '',
      } as any)
      await refresh()
      await refreshCardIfOpen(id)
    } catch (e: any) {
      alert('Could not update queue date: ' + (e?.message ?? String(e)))
    }
  }

  function openQueueTimeEditor(row: UiGameRow) {
    setQueueTimeDialog({
      id: row.id,
      title: row.title,
      currentMinutes: Math.max(0, row.playtime_minutes ?? 0),
      hours: '',
      minutes: '',
    })
  }

  async function saveQueueTime() {
    if (!queueTimeDialog || queueTimeSaving) return
    const minutesToAdd = parseAddedQueueMinutes(queueTimeDialog.hours, queueTimeDialog.minutes)
    if (minutesToAdd <= 0) {
      alert('Enter a positive time amount to add.')
      return
    }

    try {
      setQueueTimeSaving(true)
      await addQueuePlaytime(queueTimeDialog.id, minutesToAdd)
      const targetId = queueTimeDialog.id
      setQueueTimeDialog(null)
      await refresh()
      await refreshCardIfOpen(targetId)
    } catch (e: any) {
      alert('Could not add queue time: ' + (e?.message ?? String(e)))
    } finally {
      setQueueTimeSaving(false)
    }
  }

  // ---------- Epic via Legendary ----------
  async function fetchEpicOwned() {
    try {
      setEpicOwned([])
      setEpicSelected(new Set())
      const rows = await fetchEpicOwnedViaLegendary(legendaryPath || undefined)
      // sanitize known star prefix and sort
      const cleaned = rows.map(r => ({ title: cleanLegendaryTitle(r.title), appName: r.appName }))
      cleaned.sort((a, b) => compareTitleCaseInsensitive(a.title, b.title))
      setEpicOwned(cleaned)
      setEpicSelected(new Set(cleaned.map((_, i) => i))) // preselect all
    } catch (e: any) {
      alert('Legendary fetch failed: ' + (e?.message ?? String(e)))
    }
  }

  function toggleEpicRow(i: number) {
    setEpicSelected(prev => {
      const next = new Set(prev)
      if (next.has(i)) next.delete(i); else next.add(i)
      return next
    })
  }

  function toggleEpicSelectAll() {
    setEpicSelected(prev => {
      if (prev.size === epicOwned.length) return new Set()
      return new Set(epicOwned.map((_, i) => i))
    })
  }

  async function runEpicImport() {
    if (epicSelected.size === 0) { alert('No rows selected.'); return }
    const existing = new Set(
      games
        .filter(g => g.platform === 'Epic')
        .map(g => normalizeTitleLocal(g.title))
    )
    const hiddenEpic = new Set(
      games
        .filter(g => g.platform === 'Epic' && !!g.hidden_at)
        .map(g => normalizeTitleLocal(g.title))
    )

    const selected = epicOwned.map((r, i) => ({ r, i })).filter(x => epicSelected.has(x.i))
    setEpicImporting(true)
    let added = 0, skipped = 0, errored = 0

    for (const { r } of selected) {
      const nTitle = normalizeTitleLocal(r.title)
      if (hiddenEpic.has(nTitle)) { skipped++; continue }
      if (epicSkipDupes && existing.has(nTitle)) { skipped++; continue }
      try {
        await addManualGame({
          canonicalTitle: r.title,
          platform: 'Epic',
          beaten: false,
        })
        added++
        existing.add(nTitle)
      } catch (e: any) {
        console.warn('[Epic Import] failed to add', r.title, e)
        errored++
      }
    }

    setEpicImporting(false)
    await refresh()
    alert(`Epic import complete.\nAdded: ${added}\nSkipped: ${skipped}\nErrors: ${errored}`)
    setShowEpicImport(false)
    setEpicOwned([])
    setEpicSelected(new Set())
    setLegendaryPath('')
  }

  const platformOptions = customization.platformOptions
  const genreOptions = useMemo(
    () => customization.genreOptions.map((name, index) => ({ id: index + 1, name } as IgdbGenre)),
    [customization.genreOptions],
  )
  const genres = genreOptions
  const genreOptionNames = useMemo(
    () => genreOptions.map(genre => genre.name),
    [genreOptions],
  )

  useEffect(() => {
    if (platformOptions.length && !platformOptions.includes(platform)) {
      setPlatform(platformOptions[0])
    }
  }, [platform, platformOptions])

  const allGenreTags = useMemo(() => {
    const tags = games.flatMap(g =>
      (g.genre ?? '')
        .split(',')
        .map(s => s.trim())
        .filter(Boolean)
    )
    return mergeDistinctOptions(genreOptionNames, tags)
  }, [games, genreOptionNames])
  const libraryPlatformOptions = useMemo(() => {
    const platforms = games.map(game => game.platform).filter(Boolean)
    return mergeDistinctOptions(platformOptions, platforms)
  }, [games, platformOptions])
  const randomGamePlatformOptions = useMemo(() => {
    const platforms = games
      .filter(g => !g.hidden_at)
      .map(g => g.platform)
      .filter(Boolean)
    return mergeDistinctOptions(platformOptions, platforms)
  }, [games, platformOptions])
  const randomGameGenreOptions = useMemo(() => {
    const tags = games
      .filter(g => !g.hidden_at)
      .flatMap(g =>
        (g.genre ?? '')
          .split(',')
          .map(s => s.trim())
          .filter(Boolean)
      )
    return mergeDistinctOptions(genreOptionNames, tags)
  }, [games, genreOptionNames])
  const randomGameCandidates = useMemo(() => {
    return games.filter(g => {
      if (g.hidden_at) return false
      if (randomPlatform && g.platform !== randomPlatform) return false
      if (randomBeaten === 'yes' && !g.beaten) return false
      if (randomBeaten === 'no' && g.beaten) return false
      if (randomGenre) {
        const tags = (g.genre ?? '').split(',').map(s => s.trim().toLowerCase()).filter(Boolean)
        if (!tags.includes(randomGenre.toLowerCase())) return false
      }
      if (!matchesRandomTtbFilter(preferredMainTtbSeconds(g), randomTtb)) return false
      return true
    })
  }, [games, randomPlatform, randomBeaten, randomGenre, randomTtb])
  const visibleGamesForCounters = useMemo(
    () => games.filter(g => showHidden || !g.hidden_at),
    [games, showHidden]
  )
  const enrichedCount = useMemo(
    () => visibleGamesForCounters.filter(g => !!g.enriched_at).length,
    [visibleGamesForCounters]
  )
  const statsGames = useMemo(
    () => games.filter(g => !g.hidden_at),
    [games]
  )
  const libraryStats = useMemo(() => {
    const totalGames = statsGames.length
    const totalPlaytimeMinutes = statsGames.reduce((sum, game) => sum + Math.max(0, game.playtime_minutes ?? 0), 0)
    const beatenGames = statsGames.filter(game => game.beaten).length
    const completionPct = totalGames > 0 ? (beatenGames / totalGames) * 100 : 0
    const topPlayed = [...statsGames]
      .filter(game => (game.playtime_minutes ?? 0) > 0)
      .sort((a, b) => {
        const byPlaytime = (b.playtime_minutes ?? 0) - (a.playtime_minutes ?? 0)
        return byPlaytime !== 0 ? byPlaytime : compareTitleCaseInsensitive(a.title, b.title)
      })
      .slice(0, 5)

    return {
      totalGames,
      totalPlaytimeMinutes,
      beatenGames,
      completionPct,
      topPlayed,
    }
  }, [statsGames])
  const queueGames = useMemo(() => {
    return games
      .filter(game => game.queue_position != null)
      .sort((a, b) => {
        const byPosition = (a.queue_position ?? Number.MAX_SAFE_INTEGER) - (b.queue_position ?? Number.MAX_SAFE_INTEGER)
        return byPosition !== 0 ? byPosition : compareTitleCaseInsensitive(a.title, b.title)
      })
  }, [games])
  const queueStats = useMemo(() => {
    const playingCount = queueGames.filter(game => !!game.queue_is_playing).length
    const finishedTrackedGames = games
      .flatMap<QueueFinishedTrackerGame>(game => {
        const finishedAt = localDateInputValue(game.queue_finished_at)
        if (!finishedAt) return []
        return [{
          id: game.id,
          title: game.title,
          platform: game.platform,
          finishedAt,
          queuePosition: game.queue_position,
          hiddenAt: game.hidden_at,
        }]
      })
      .sort((a, b) => {
        const byDate = b.finishedAt.localeCompare(a.finishedAt)
        return byDate !== 0 ? byDate : compareTitleCaseInsensitive(a.title, b.title)
      })
    const finishedLast7Games = finishedTrackedGames.filter(game => isWithinLastCalendarDays(game.finishedAt, 7))
    const finishedLast30Games = finishedTrackedGames.filter(game => isWithinLastCalendarDays(game.finishedAt, 30))
    const finishedLast365Games = finishedTrackedGames.filter(game => isWithinLastCalendarDays(game.finishedAt, 365))
    const totalPlaytimeMinutes = queueGames.reduce((sum, game) => sum + Math.max(0, game.playtime_minutes ?? 0), 0)
    const totalTtbSeconds = queueGames.reduce((sum, game) => {
      const seconds = preferredMainTtbSeconds(game)
      return sum + Math.max(0, seconds ?? 0)
    }, 0)
    const ttbKnownCount = queueGames.filter(game => (preferredMainTtbSeconds(game) ?? 0) > 0).length
    const ttbUnknownCount = queueGames.length - ttbKnownCount
    return {
      totalGames: queueGames.length,
      playingCount,
      finishedTrackedCount: finishedTrackedGames.length,
      finishedTrackedGames,
      finishedLast7Days: finishedLast7Games.length,
      finishedLast7Games,
      finishedLast30Days: finishedLast30Games.length,
      finishedLast30Games,
      finishedLast365Days: finishedLast365Games.length,
      finishedLast365Games,
      totalPlaytimeMinutes,
      totalTtbSeconds,
      ttbKnownCount,
      ttbUnknownCount,
    }
  }, [games, queueGames])
  const queueFinishedTrackerGames = queueFinishedTrackerDialog == null
    ? []
    : queueFinishedTrackerDialog.key === 'last7'
      ? queueStats.finishedLast7Games
      : queueFinishedTrackerDialog.key === 'last30'
        ? queueStats.finishedLast30Games
        : queueStats.finishedLast365Games
  useEffect(() => {
    setQueueOrderDrafts(prev => {
      const validIds = new Set(queueGames.map(game => game.id))
      const next = Object.fromEntries(
        Object.entries(prev).filter(([id]) => validIds.has(id))
      )
      return Object.keys(next).length === Object.keys(prev).length ? prev : next
    })
  }, [queueGames])

  function toggleSort(field: 'title' | 'release' | 'ttb' | 'playtime' | 'last' | 'review') {
    setSortBy(prev => {
      const asc = `${field}-asc` as SortKey
      const desc = `${field}-desc` as SortKey
      if (prev === desc) return asc
      if (prev === asc) return ''
      return desc
    })
  }
  function sortIcon(field: 'title' | 'release' | 'ttb' | 'playtime' | 'last' | 'review') {
    const p = sortBy.startsWith(field) ? (sortBy.endsWith('asc') ? '▲' : '▼') : ''
    return p
  }

  function sortIndicator(field: 'title' | 'release' | 'ttb' | 'playtime' | 'last' | 'review') {
    if (!sortBy.startsWith(field)) return ''
    return sortBy.endsWith('asc') ? '\u2191' : '\u2193'
  }

  const baseFilteredGames = useMemo(() => {
    return games.filter(g => {
      if (!showHidden && !!g.hidden_at) return false
      if (fPlatform && g.platform !== fPlatform) return false
      if (fBeaten === 'yes' && !g.beaten) return false
      if (fBeaten === 'no' && g.beaten) return false
      if (fGenre) {
        const tags = (g.genre ?? '').split(',').map(s => s.trim().toLowerCase()).filter(Boolean)
        if (!tags.includes(fGenre.toLowerCase())) return false
      }
      if (fMinHours) {
        const min = parseFloat(fMinHours)
        const hoursPlayed = (g.playtime_minutes ?? 0) / 60
        if (isFinite(min) && hoursPlayed < min) return false
      }
      if (fLicense) {
        const lt = normLicenseType(g.license_type)
        if (lt !== fLicense) return false
      }
      if (fEnriched === 'yes' && !g.enriched_at) return false
      if (fEnriched === 'no' && !!g.enriched_at) return false
      if (fReviewed === 'yes' && !hasReviewRecord(g)) return false
      if (fReviewed === 'no' && hasReviewRecord(g)) return false
      return true
    })
  }, [games, showHidden, fPlatform, fBeaten, fGenre, fMinHours, fLicense, fEnriched, fReviewed])

  const titleIndexCounts = useMemo(() => {
    const counts = new Map<string, number>()
    for (const game of baseFilteredGames) {
      const key = titleIndexKey(game.title)
      counts.set(key, (counts.get(key) ?? 0) + 1)
    }
    return counts
  }, [baseFilteredGames])

  const titleFilteredGames = useMemo(() => {
    return fTitleIndex
      ? baseFilteredGames.filter(game => titleIndexKey(game.title) === fTitleIndex)
      : baseFilteredGames
  }, [baseFilteredGames, fTitleIndex])

  const matchingGames = useMemo(() => {
    const query = q.trim().toLowerCase()
    if (!query) return titleFilteredGames
    return titleFilteredGames.filter(game => game.title.toLowerCase().includes(query))
  }, [titleFilteredGames, q])

  // Filtering + sorting
  const filtered = useMemo(() => {
    const query = q.trim().toLowerCase()

    const dateKey = (s?: string | null) => {
      if (!s) return -1
      const d = new Date(s); const t = d.getTime()
      return Number.isFinite(t) && d.getFullYear() > 1970 ? t : -1
    }
    const compareGames = (a: UiGameRow, b: UiGameRow) => {
      switch (sortBy) {
        case 'title-asc':
        case 'title-desc': {
          const cmp = compareTitleCaseInsensitive(a.title, b.title)
          return sortBy === 'title-asc' ? cmp : -cmp
        }
        case 'release-asc':
        case 'release-desc': {
          const A = a.release_year ?? -1
          const B = b.release_year ?? -1
          const cmp = A === B ? compareTitleCaseInsensitive(a.title, b.title) : (A - B)
          return sortBy === 'release-asc' ? cmp : -cmp
        }
        case 'ttb-asc':
        case 'ttb-desc': {
          const A = preferredMainTtbSeconds(a) ?? -1
          const B = preferredMainTtbSeconds(b) ?? -1
          const cmp = A === B ? compareTitleCaseInsensitive(a.title, b.title) : (A - B)
          return sortBy === 'ttb-asc' ? cmp : -cmp
        }
        case 'playtime-asc':
        case 'playtime-desc': {
          const A = a.playtime_minutes ?? -1
          const B = b.playtime_minutes ?? -1
          const cmp = A === B ? compareTitleCaseInsensitive(a.title, b.title) : (A - B)
          return sortBy === 'playtime-asc' ? cmp : -cmp
        }
        case 'last-asc':
        case 'last-desc': {
          const A = dateKey(a.last_played_at)
          const B = dateKey(b.last_played_at)
          const cmp = A === B ? compareTitleCaseInsensitive(a.title, b.title) : (A - B)
          return sortBy === 'last-asc' ? cmp : -cmp
        }
        case 'review-asc':
        case 'review-desc': {
          const describe = (game: UiGameRow) => {
            if (game.review_rating != null) return { bucket: 0, rating: game.review_rating }
            if (hasReviewRecord(game)) return { bucket: 1, rating: -1 }
            return { bucket: 2, rating: -1 }
          }
          const A = describe(a)
          const B = describe(b)
          if (A.bucket !== B.bucket) return A.bucket - B.bucket
          if (A.bucket === 0) {
            const cmp = A.rating === B.rating ? compareTitleCaseInsensitive(a.title, b.title) : (A.rating - B.rating)
            return sortBy === 'review-asc' ? cmp : -cmp
          }
          return compareTitleCaseInsensitive(a.title, b.title)
        }
        default: return 0
      }
    }

    const selectedPinned = query
      ? titleFilteredGames.filter(game => selectedIds.has(game.id))
      : []
    const pinnedIds = new Set(selectedPinned.map(game => game.id))
    const remainingMatches = matchingGames.filter(game => !pinnedIds.has(game.id))

    selectedPinned.sort(compareGames)
    remainingMatches.sort(compareGames)

    return query ? [...selectedPinned, ...remainingMatches] : remainingMatches
  }, [matchingGames, selectedIds, q, sortBy, titleFilteredGames])

  // Clamp/reset page when filters or page size change
  const pageCount = Math.max(1, Math.ceil(filtered.length / pageSize))
  useEffect(() => { if (page > pageCount) setPage(pageCount) }, [pageCount]) // clamp down
  useEffect(() => { setPage(1) }, [q, showHidden, fTitleIndex, fPlatform, fBeaten, fGenre, fMinHours, fLicense, fEnriched, fReviewed, sortBy])
  useEffect(() => { setPage(1) }, [pageSize])

  const startIdx = (page - 1) * pageSize
  const endIdx = Math.min(startIdx + pageSize, filtered.length)
  const paged = useMemo(() => filtered.slice(startIdx, endIdx), [filtered, startIdx, endIdx])

  // Selection helpers tied to visible (paged) rows
  const pageIds = useMemo(() => paged.map(g => g.id), [paged])
  const matchingIds = useMemo(() => matchingGames.map(g => g.id), [matchingGames])
  const allVisibleSelected = pageIds.length > 0 && pageIds.every(id => selectedIds.has(id))
  const allMatchingSelected = matchingIds.length > 0 && matchingIds.every(id => selectedIds.has(id))
  function toggleSelectAll() {
    setSelectedIds(prev => {
      const next = new Set(prev)
      if (allVisibleSelected) pageIds.forEach(id => next.delete(id))
      else pageIds.forEach(id => next.add(id))
      return next
    })
  }
  function selectAllShown() {
    setSelectedIds(prev => {
      const next = new Set(prev)
      pageIds.forEach(id => next.add(id))
      return next
    })
  }
  function selectAllMatching() {
    setSelectedIds(prev => {
      const next = new Set(prev)
      matchingIds.forEach(id => next.add(id))
      return next
    })
  }
  function toggleRow(id: string) {
    setSelectedIds(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id); else next.add(id)
      return next
    })
  }
  function clearSelection() { setSelectedIds(new Set()) }

  const pct = prog && prog.total > 0 ? Math.round((prog.done/prog.total)*100) : 0
  const topPlayedMaxMinutes = libraryStats.topPlayed[0]?.playtime_minutes ?? 0
  const hasActiveFilters =
    !!q.trim() ||
    !!fTitleIndex ||
    !!fPlatform ||
    !!fBeaten ||
    !!fGenre ||
    !!fMinHours ||
    !!fLicense ||
    !!fEnriched ||
    !!fReviewed ||
    showHidden
  const normalizedCardReviewText = (card?.review_text ?? '').trim()
  const normalizedReviewDraft = reviewDraft.trim()
  const reviewUnchanged =
    !!card &&
    reviewRatingDraft === (card.review_rating ?? null) &&
    normalizedReviewDraft === normalizedCardReviewText
  const hasIgdbTtb =
    !!card &&
    (card.ttb_main != null || card.ttb_extra != null || card.ttb_complete != null)
  const hasHltbTtb =
    !!card &&
    (card.hltb_main != null || card.hltb_extra != null || card.hltb_complete != null)
  const preferredTtb = card ? {
    main: firstNumber(card.ttb_main, card.hltb_main),
    extra: firstNumber(card.ttb_extra, card.hltb_extra),
    complete: firstNumber(card.ttb_complete, card.hltb_complete),
    count: firstNumber(
      card.ttb_count,
      card.hltb_all_count,
      card.hltb_complete_count,
      card.hltb_extra_count,
      card.hltb_main_count
    ),
  } : null
  const cardTtbDisagreement =
    !!card &&
    (
      ttbValuesDiffer(card.ttb_main, card.hltb_main) ||
      ttbValuesDiffer(card.ttb_extra, card.hltb_extra) ||
      ttbValuesDiffer(card.ttb_complete, card.hltb_complete)
    )

  function fmtHoursFromSec(s?: number | null) {
    if (!s || s <= 0) return '—'
    const h = s / 3600
    if (h >= 10) return `${Math.round(h)}h`
    return `${h.toFixed(1)}h`
  }
  function resetFilters() {
    setQ('')
    setFTitleIndex('')
    setFPlatform('')
    setFBeaten('')
    setFGenre('')
    setFMinHours('')
    setFLicense('')
    setFEnriched('')
    setFReviewed('')
    setShowHidden(false)
  }
  async function openCard(id: string) {
    try {
      const d = await getGameDetails(id)
      setCard(d)
      setCardTab('details')
      setNotesDraft(d.notes ?? '')
      setNotesSaving(false)
      setReviewDraft(d.review_text ?? '')
      setReviewRatingDraft(d.review_rating ?? null)
      setReviewSaving(false)
      setCardIgdbIdInput(d.igdb_id != null ? String(d.igdb_id) : '')
      setCardIgdbApplying(false)
      setCardHltbIdInput(d.hltb_id != null ? String(d.hltb_id) : '')
      setCardHltbApplying(false)
    } catch (e:any) {
      console.error('getGameDetails failed:', e)
      alert('Could not load details: ' + (e?.message ?? String(e)))
    }
  }
  async function refreshCardIfOpen(targetId?: string) {
    const cardId = targetId ?? card?.id
    if (!cardId) return
    if (targetId && card?.id !== targetId) return
    try {
      const fresh = await getGameDetails(cardId)
      setCard(fresh)
      setCardIgdbIdInput(fresh.igdb_id != null ? String(fresh.igdb_id) : '')
      setCardHltbIdInput(fresh.hltb_id != null ? String(fresh.hltb_id) : '')
    } catch (e) {
      console.warn('refreshCardIfOpen failed:', e)
    }
  }
  async function applyBestHltbForGame(id: string, title: string, platform?: string | null) {
    try {
      const pv: any = await lookupHltbPreview(title, platform ?? undefined)
      if (!pv) return false
      await applyHltbDetails({
        id,
        hltb_id: pv.hltb_id ?? undefined,
        title: pv.title ?? undefined,
        profile_url: pv.profile_url ?? undefined,
        hltb_main: pv.hltb_main ?? undefined,
        hltb_extra: pv.hltb_extra ?? undefined,
        hltb_complete: pv.hltb_complete ?? undefined,
        hltb_all: pv.hltb_all ?? undefined,
        hltb_main_count: pv.hltb_main_count ?? undefined,
        hltb_extra_count: pv.hltb_extra_count ?? undefined,
        hltb_complete_count: pv.hltb_complete_count ?? undefined,
        hltb_all_count: pv.hltb_all_count ?? undefined,
        platforms: pv.platforms ?? undefined,
        genres: pv.genres ?? undefined,
        summary: pv.summary ?? undefined,
        match_method: pv.match_method ?? 'auto-title',
      })
      return true
    } catch (e) {
      console.warn('[HLTB] auto apply failed for', title, e)
      return false
    }
  }
  function resetRandomGameFilters() {
    setRandomPlatform('')
    setRandomBeaten('')
    setRandomGenre('')
    setRandomTtb('')
  }
  function openRandomGamePicker() {
    resetRandomGameFilters()
    setRandomPicking(false)
    setShowRandomGame(true)
  }
  function restartRandomGameFlow() {
    setRandomPicking(false)
    setCard(null)
    resetRandomGameFilters()
  }
  function closeRandomGameFlow() {
    setRandomPicking(false)
    setCard(null)
    resetRandomGameFilters()
    setShowRandomGame(false)
  }
  function closeCardView() {
    if (showRandomGame) {
      closeRandomGameFlow()
      return
    }
    setCard(null)
  }
  async function pickRandomGame() {
    if (!randomGameCandidates.length || randomPicking) return
    const pick = randomGameCandidates[Math.floor(Math.random() * randomGameCandidates.length)]
    try {
      setRandomPicking(true)
      await openCard(pick.id)
    } finally {
      setRandomPicking(false)
    }
  }
  async function saveNotes() {
    if (!card) return
    if (notesDraft === (card.notes ?? '')) return
    try {
      setNotesSaving(true)
      await saveGameNotes(card.id, notesDraft)
      setCard(prev => (prev ? { ...prev, notes: notesDraft } : prev))
    } catch (e:any) {
      alert('Failed to save notes: ' + (e?.message ?? String(e)))
    } finally {
      setNotesSaving(false)
    }
  }

  async function saveReview() {
    if (!card || reviewUnchanged) return
    const reviewText = normalizedReviewDraft ? normalizedReviewDraft : null
    const reviewRating = reviewRatingDraft

    try {
      setReviewSaving(true)
      await saveGameReview(card.id, reviewRating, reviewText)
      setReviewDraft(reviewText ?? '')
      setCard(prev => prev ? { ...prev, review_rating: reviewRating, review_text: reviewText } : prev)
      patchGameReviewInList(card.id, reviewRating, reviewText)
    } catch (e: any) {
      alert('Failed to save review: ' + (e?.message ?? String(e)))
    } finally {
      setReviewSaving(false)
    }
  }

  async function applyCardIgdbId() {
    if (!card || cardIgdbApplying) return
    const raw = cardIgdbIdInput.trim()
    if (!/^\d+$/.test(raw)) {
      alert('Enter a numeric IGDB ID.')
      return
    }
    const igdbId = Number(raw)
    if (!Number.isFinite(igdbId) || igdbId <= 0) {
      alert('Enter a valid IGDB ID.')
      return
    }

    let cid = twId
    let secret = twSecret
    if (!cid || !secret) {
      try {
        const got = await getTwitchCreds()
        cid = got.clientId
        secret = got.clientSecret
      } catch {}
    }
    if (!cid || !secret) {
      cid = prompt('Enter Twitch Client ID for IGDB:') || ''
      secret = prompt('Enter Twitch Client Secret:') || ''
      if (cid && secret) {
        try { await saveTwitchCreds({ clientId: cid, clientSecret: secret }) } catch {}
        setTwId(cid)
        setTwSecret(secret)
      }
    }
    if (!cid || !secret) {
      alert('Client ID and Secret are required.')
      return
    }

    const gameId = card.id
    setCardIgdbApplying(true)
    try {
      const rawPreview = await lookupIgdbPreviewById(igdbId, cid, secret)
      const pv = normalizeIgdb(rawPreview)
      if (!pv) throw new Error('No IGDB match found for that ID.')

      await applyIgdbDetails({
        id: gameId,
        igdb_id: pv.igdb_id ?? igdbId,
        igdb_url: pv.igdb_url ?? undefined,
        genres: pv.genres,
        cover_url: pv.cover_url ?? undefined,
        release_year: pv.release_year ?? undefined,
        summary: pv.summary ?? undefined,
        storyline: pv.storyline ?? undefined,
        agg_rating: pv.agg_rating ?? undefined,
        user_rating: pv.user_rating ?? undefined,
        ttb_main: pv.ttb_main ?? undefined,
        ttb_extra: pv.ttb_extra ?? undefined,
        ttb_complete: pv.ttb_complete ?? undefined,
        ttb_count: pv.ttb_count ?? undefined,
      })

      await refresh()
      const fresh = await getGameDetails(gameId)
      setCard(fresh)
      setNotesDraft(fresh.notes ?? '')
      setCardIgdbIdInput(fresh.igdb_id != null ? String(fresh.igdb_id) : String(igdbId))
    } catch (e: any) {
      alert('IGDB ID apply failed: ' + (e?.message ?? String(e)))
    } finally {
      setCardIgdbApplying(false)
    }
  }

  async function applyCardHltbId() {
    if (!card || cardHltbApplying) return
    const raw = cardHltbIdInput.trim()
    if (!/^\d+$/.test(raw)) {
      alert('Enter a numeric HLTB ID.')
      return
    }
    const hltbId = Number(raw)
    if (!Number.isFinite(hltbId) || hltbId <= 0) {
      alert('Enter a valid HLTB ID.')
      return
    }

    const gameId = card.id
    setCardHltbApplying(true)
    try {
      const pv: any = await lookupHltbPreviewById(hltbId)
      if (!pv) throw new Error('No HLTB match found for that ID.')

      await applyHltbDetails({
        id: gameId,
        hltb_id: pv.hltb_id ?? hltbId,
        title: pv.title ?? undefined,
        profile_url: pv.profile_url ?? undefined,
        hltb_main: pv.hltb_main ?? undefined,
        hltb_extra: pv.hltb_extra ?? undefined,
        hltb_complete: pv.hltb_complete ?? undefined,
        hltb_all: pv.hltb_all ?? undefined,
        hltb_main_count: pv.hltb_main_count ?? undefined,
        hltb_extra_count: pv.hltb_extra_count ?? undefined,
        hltb_complete_count: pv.hltb_complete_count ?? undefined,
        hltb_all_count: pv.hltb_all_count ?? undefined,
        platforms: pv.platforms ?? undefined,
        genres: pv.genres ?? undefined,
        summary: pv.summary ?? undefined,
        match_method: 'manual-id',
      })

      await refresh()
      const fresh = await getGameDetails(gameId)
      setCard(fresh)
      setNotesDraft(fresh.notes ?? '')
      setCardHltbIdInput(fresh.hltb_id != null ? String(fresh.hltb_id) : String(hltbId))
    } catch (e: any) {
      alert('HLTB ID apply failed: ' + (e?.message ?? String(e)))
    } finally {
      setCardHltbApplying(false)
    }
  }

  async function openCardExternalPage(kind: 'igdb' | 'hltb') {
    if (!card) return

    const url = kind === 'igdb'
      ? (String(card.igdb_url ?? '').trim() || 'https://www.igdb.com/')
      : (String(card.hltb_url ?? '').trim()
        || buildHltbExternalUrl(parsePositiveInteger(cardHltbIdInput) ?? parsePositiveInteger(card.hltb_id)))

    try {
      await openExternalLink(url)
    } catch (e: any) {
      alert(`Could not open ${kind.toUpperCase()} in your browser: ` + (e?.message ?? String(e)))
    }
  }

  async function openCardGameFaqsSearch() {
    if (!card) return

    try {
      await openExternalLink(buildGameFaqsSearchUrl(card.title))
    } catch (e: any) {
      alert('Could not open GameFAQs in your browser: ' + (e?.message ?? String(e)))
    }
  }

  async function add() {
    const t = title.trim(); if (!t) return
    const genreName = genreValue.trim() || undefined
    try {
      const id = await addManualGame({
        canonicalTitle: t,
        platform,
        beaten,
        genre: genreName,
        licenseType: addLicenseType,
        licenseSource: addLicenseType === 'subscription' ? addLicenseSource : undefined,
      })
      setTitle(''); setBeaten(false); setGenreValue(''); setAddLicenseType('owned'); setAddLicenseSource('')

      await applyBestHltbForGame(id, t, platform)

      // creds
      let cid = twId, secret = twSecret
      if (!cid || !secret) {
        try { const got = await getTwitchCreds(); cid = got.clientId; secret = got.clientSecret } catch {}
      }
      if (!cid || !secret) {
        cid = prompt('Enter Twitch Client ID for IGDB lookup:') || ''
        secret = prompt('Enter Twitch Client Secret for IGDB lookup:') || ''
        if (cid && secret) { try { await saveTwitchCreds({ clientId: cid, clientSecret: secret }) } catch {} ; setTwId(cid); setTwSecret(secret) }
      }

      if (cid && secret) {
        try {
          const candidates = await getIgdbCandidates(t, cid, secret)
          if (candidates.length) {
            const cur: UiGameRow = {
              id, title: t, platform, beaten,
              genre: genreName ?? '', playtime_minutes: 0, last_played_at: null,
              cover_url: null, release_year: null,
              license_type: addLicenseType,
              license_source: addLicenseType === 'subscription' ? (addLicenseSource.trim() || null) : null,
            } as any
            setIgdbOffer({ forId: id, title: t, current: cur, candidates, idx: 0, creds: { cid, secret } })
          }
        } catch (e:any) {
          console.warn('[add] candidate lookup failed:', e?.message ?? String(e))
        }
      }

      await refresh()
    } catch (e: any) {
      console.error('add manual failed:', e)
      alert('Add failed: ' + (e?.message ?? String(e)))
    }
  }

  async function runSteamImport() {
    if (importing || steamSyncing) return
    const key = steamApiKey.trim(); const prof = steamProfile.trim()
    if (!prof) { alert('Enter a Steam profile (SteamID64, vanity, or full profile URL).'); return }
    try {
      setImporting(true)
      const count = await importSteamGames(key, prof)
      await persistSteamCreds(key, prof)
      alert(`Imported/updated ${count} games from Steam.`)
      setShowImport(false)
      await refresh()
    } catch (err: any) {
      console.error('Steam import failed:', err); alert('Steam import failed: ' + (err?.message ?? String(err)))
    } finally { setImporting(false) }
  }

  async function runSteamTimeSync() {
    if (importing || steamSyncing) return
    const key = steamApiKey.trim(); const prof = steamProfile.trim()
    if (!prof) { alert('Enter a Steam profile (SteamID64, vanity, or full profile URL).'); return }
    try {
      setSteamSyncing(true)
      const count = await syncSteamPlaytime(key, prof)
      await persistSteamCreds(key, prof)
      alert(`Synced play time for ${count} existing Steam game${count === 1 ? '' : 's'}.`)
      setShowImport(false)
      await refresh()
    } catch (err: any) {
      console.error('Steam time sync failed:', err); alert('Steam time sync failed: ' + (err?.message ?? String(err)))
    } finally { setSteamSyncing(false) }
  }

  async function runIgdbEnrich(onlyUnenriched = false) {
    let cid = twId, secret = twSecret
    if (!cid || !secret) {
      try {
        const got = await getTwitchCreds()
        cid = got.clientId; secret = got.clientSecret
      } catch {}
      if (!cid || !secret) {
        cid = prompt('Enter Twitch Client ID for IGDB:') || ''
        secret = prompt('Enter Twitch Client Secret:') || ''
      }
    }
    if (!cid || !secret) { alert('Client ID and Secret are required.'); return }
    try {
      setProgCollapsed(false)
      setProg({ done: 0, total: 0, title: onlyUnenriched ? 'starting unenriched pass' : 'starting' })
      const updated = await enrichGenresWithIgdb(cid, secret, onlyUnenriched)
      alert(
        onlyUnenriched
          ? `Updated ${updated} unenriched games via IGDB plus HLTB fallback.`
          : `Updated ${updated} games via IGDB plus HLTB fallback.`
      )
    } catch (e: any) {
      alert('IGDB enrichment failed: ' + (e?.message ?? String(e)))
    } finally {
      setProg(null)
      setProgCollapsed(false)
      await refresh()
      await refreshCardIfOpen()
    }
  }

  function startEdit(row: UiGameRow) {
    setEditingId(row.id)
    setEdit({
      id: row.id,
      title: row.title,
      platform: row.platform,
      beaten: row.beaten,
      genre: row.genre ?? '',
      playtimeHours: ((row.playtime_minutes ?? 0)/60).toFixed(1),
      lastPlayed: localDateInputValue(row.last_played_at),
      coverUrl: row.cover_url ?? '',
      releaseYear: row.release_year ? String(row.release_year) : '',

      // NEW:
      licenseType: normLicenseType(row.license_type),
      licenseSource: row.license_source ?? '',
    })
  }
  function cancelEdit() { setEditingId(null); setEdit(null) }

  async function saveEdit() {
    if (!edit) return
    try {
      // parse fields safely
      const hoursFloat = parseFloat(edit.playtimeHours);
      const minutes = Number.isFinite(hoursFloat) ? Math.round(hoursFloat * 60) : undefined;

      const ryNum = parseInt(edit.releaseYear, 10);
      const release_year = Number.isFinite(ryNum) ? ryNum : undefined;

      const last_played_at = edit.lastPlayed
        ? toIsoDateOrEmpty(edit.lastPlayed)
        : '';

      // build snake_case payload expected by the backend
      const payload: any = {
        id: edit.id,
        canonical_title: edit.title,     // title update
        platform: edit.platform,
        beaten: edit.beaten,
        genre: edit.genre || undefined,
        playtime_minutes: minutes,       // time played
        last_played_at,                  // last played
        cover_url: edit.coverUrl || undefined,
        release_year,                    // release year

        // NEW: license fields
        license_type: edit.licenseType,
        license_source: edit.licenseType === 'subscription' ? edit.licenseSource : undefined,
      };

      await updateGame(payload);

      await refresh();
      setEditingId(null);
      setEdit(null);
    } catch (e: any) {
      console.error('updateGame failed:', e);
      alert('Save failed: ' + (e?.message ?? String(e)));
    }
  }

  useEffect(() => {
    if (showSettings) {
      setCustomizationDraft(customization)
      setPlatformOptionsDraftText(formatOptionEditorValue(customization.platformOptions))
      setGenreOptionsDraftText(formatOptionEditorValue(customization.genreOptions))
      getTwitchCreds().then(v => {
        setTwId(v.clientId || ''); setTwSecret(v.clientSecret || '')
      }).catch(()=>{})
      getCustomizationSettings()
        .then(settings => {
          const normalized = normalizeCustomizationSettings(settings)
          setCustomization(normalized)
          setCustomizationDraft(normalized)
          setPlatformOptionsDraftText(formatOptionEditorValue(normalized.platformOptions))
          setGenreOptionsDraftText(formatOptionEditorValue(normalized.genreOptions))
        })
        .catch(() => {})
    }
  }, [showSettings])

  async function selectCustomizationBackground(backgroundImageName: string) {
    if (!backgroundImageName) {
      setCustomizationDraft(prev => ({
        ...prev,
        backgroundImageName: null,
        backgroundImageDataUrl: null,
      }))
      return
    }

    const dataUrl = await loadCustomizationBackgroundImage(backgroundImageName)
    setCustomizationDraft(prev =>
      normalizeCustomizationSettings({
        ...prev,
        backgroundImageName,
        backgroundImageDataUrl: dataUrl,
      }),
    )
  }

  async function uploadCustomizationBackgroundFile(file: File) {
    try {
      setBackgroundUploading(true)
      const dataBase64 = await fileToBase64(file)
      const uploaded = await uploadCustomizationBackground({
        fileName: file.name,
        mimeType: file.type || 'image/png',
        dataBase64,
      })

      setCustomizationDraft(prev =>
        normalizeCustomizationSettings({
          ...prev,
          backgroundImageName: uploaded.fileName,
          backgroundImageDataUrl: uploaded.backgroundImageDataUrl,
          availableBackgrounds: uploaded.availableBackgrounds,
        }),
      )
    } catch (e: any) {
      alert('Could not upload background image: ' + (e?.message ?? String(e)))
    } finally {
      setBackgroundUploading(false)
    }
  }

  function resetCustomizationDraft() {
    setCustomizationDraft(prev =>
      normalizeCustomizationSettings({
        ...DEFAULT_CUSTOMIZATION,
        availableBackgrounds: prev.availableBackgrounds,
        platformOptions: prev.platformOptions,
        genreOptions: prev.genreOptions,
      }),
    )
  }

  function closeSettingsModal() {
    setCustomizationDraft(customization)
    setPlatformOptionsDraftText(formatOptionEditorValue(customization.platformOptions))
    setGenreOptionsDraftText(formatOptionEditorValue(customization.genreOptions))
    setShowSettings(false)
  }

  async function saveSettings() {
    try {
      setCustomizationSaving(true)
      await saveTwitchCreds({ clientId: twId.trim(), clientSecret: twSecret.trim() })
      const savedCustomization = normalizeCustomizationSettings(
        await saveCustomizationSettings({
          buttonColor: customizationDraft.buttonColor,
          textColor: customizationDraft.textColor,
          cardBackgroundColor: customizationDraft.cardBackgroundColor,
          sectionBackgroundColor: customizationDraft.sectionBackgroundColor,
          backgroundImageName: customizationDraft.backgroundImageName ?? null,
          platformOptions: parseOptionEditorValue(platformOptionsDraftText),
          genreOptions: parseOptionEditorValue(genreOptionsDraftText),
        }),
      )
      setCustomization(savedCustomization)
      setCustomizationDraft(savedCustomization)
      setPlatformOptionsDraftText(formatOptionEditorValue(savedCustomization.platformOptions))
      setGenreOptionsDraftText(formatOptionEditorValue(savedCustomization.genreOptions))
      alert('Saved settings and customization.')
      setShowSettings(false)
    } catch (e:any) {
      alert('Failed to save settings: ' + (e?.message ?? String(e)))
    } finally {
      setCustomizationSaving(false)
    }
  }

  async function openEnrichChooser(row: UiGameRow) {
    let cid = twId, secret = twSecret
    if (!cid || !secret) {
      try { const got = await getTwitchCreds(); cid = got.clientId; secret = got.clientSecret } catch {}
      if (!cid || !secret) {
        cid = prompt('Enter Twitch Client ID for IGDB:') || ''
        secret = prompt('Enter Twitch Client Secret:') || ''
      }
    }
    if (!cid || !secret) { alert('Client ID and Secret are required.'); return }
    try {
      const candidates = await getIgdbCandidates(row.title, cid, secret)
      if (!candidates.length) { alert('No matches found on IGDB for: ' + row.title); return }
      console.log('[chooser] got candidates', candidates.map(c => ({ id: c.igdb_id, title: c.title })))
      setIgdbOffer({ forId: row.id, title: row.title, current: row, candidates, idx: 0, creds: { cid, secret } })
    } catch (e: any) {
      alert('Lookup failed: ' + (e?.message ?? String(e)))
    }
  }

  /** Hydrate details for a candidate by ID via backend only. */
  async function hydrateCandidateByIndex(idx: number): Promise<Npv | null> {
    if (!igdbOffer) return null
    const c = igdbOffer.candidates[idx]
    if (!c) return null

    const isRich = (x: Npv | null | undefined) =>
      !!x && (x.summary != null || x.storyline != null || x.agg_rating != null || x.user_rating != null || x.ttb_main != null || x.ttb_complete != null)

    if (isRich(c)) return c
    if (c.igdb_id == null) return c

    setHydratingIdx(idx)
    try {
      const { cid, secret } = igdbOffer.creds
      let norm: Npv | null = null

      try {
        const pv = await lookupIgdbPreviewById(c.igdb_id, cid, secret)
        if (pv) norm = normalizeIgdb(pv)
      } catch (e) {
        console.warn('[hydrate] byId via backend failed', e)
      }

      if (!norm) return c
      const merged = safeFill(c, norm)
      setIgdbOffer(o =>
        o ? { ...o, candidates: o.candidates.map((x, i) => (i === idx ? merged : x)) } : o
      )
      return merged
    } finally {
      setHydratingIdx(null)
    }
  }

  // --- Bulk actions runner ---
  async function runBulk() {
    const ids = Array.from(selectedIds)
    if (ids.length === 0 || !bulkAction) return

    if (bulkAction === 'hide') {
      if (!confirm(`Hide ${ids.length} selected game(s) from your library view?`)) return
      try {
        for (const id of ids) {
          try { await hideGame(id) } catch (e: any) { console.warn('hide failed for', id, e) }
        }
        await refresh()
      } finally { clearSelection() }
      return
    }

    if (bulkAction === 'beaten' || bulkAction === 'unbeaten') {
      const nextBeaten = bulkAction === 'beaten'
      if (!confirm(`${nextBeaten ? 'Mark' : 'Mark as not beaten'} ${ids.length} selected game(s)?`)) return
      try {
        for (const id of ids) {
          try { await updateGame({ id, beaten: nextBeaten }) } catch (e: any) { console.warn('beaten update failed for', id, e) }
        }
        await refresh()
      } finally { clearSelection() }
      return
    }

    if (bulkAction === 'delete') {
      if (!confirm(`Delete ${ids.length} selected game(s)? This cannot be undone.`)) return
      try {
        for (const id of ids) {
          try { await deleteGame(id) } catch (e: any) { console.warn('delete failed for', id, e) }
        }
        await refresh()
      } finally { clearSelection() }
      return
    }

    if (bulkAction === 'enrich') {
      let cid = twId, secret = twSecret
      if (!cid || !secret) {
        try { const got = await getTwitchCreds(); cid = got.clientId; secret = got.clientSecret } catch {}
        if (!cid || !secret) { cid = prompt('Enter Twitch Client ID for IGDB:') || ''; secret = prompt('Enter Twitch Client Secret:') || '' }
      }
      if (!cid || !secret) { alert('Client ID and Secret are required.'); return }

      const idToRow = new Map(games.map(g => [g.id, g]))
      setProgCollapsed(false)
      setProg({ done: 0, total: ids.length, title: 'Starting…' })
      let done = 0

      for (const id of ids) {
        const row = idToRow.get(id)
        const t = row?.title || ''
        try {
          setProg({ done, total: ids.length, title: t })

          // robust preview that tries multiple payload shapes
          const n = await previewIgdbFlexible(t, cid, secret)
          if (n && n.igdb_id) {
            await applyIgdbDetails({
              id,
              igdb_id: n.igdb_id,
              igdb_url: n.igdb_url ?? undefined,
              genres: n.genres,
              cover_url: n.cover_url ?? undefined,
              release_year: n.release_year ?? undefined,
              summary: n.summary ?? undefined,
              storyline: n.storyline ?? undefined,
              agg_rating: n.agg_rating ?? undefined,
              user_rating: n.user_rating ?? undefined,
              ttb_main: n.ttb_main ?? undefined,
              ttb_extra: n.ttb_extra ?? undefined,
              ttb_complete: n.ttb_complete ?? undefined,
              ttb_count: n.ttb_count ?? undefined,
            })
          } else {
            console.warn('No preview match for', id, t)
          }
          if (row) {
            await applyBestHltbForGame(id, row.title, row.platform)
          }
        } catch (e: any) {
          console.warn('Enrich failed for', id, t, e)
        } finally {
          done += 1
          setProg({ done, total: ids.length, title: t })
        }
      }

      setProg(null)
      setProgCollapsed(false)
      clearSelection()
      await refresh()
      await refreshCardIfOpen()
    }
  }

  function closeAmazonImport() {
    setShowAmazonImport(false)
    setAmazonRows([])
    setAmazonSelected(new Set())
    setAmazonDbPath('')
  }

  // ---------- Amazon library handlers ----------
  async function loadAmazonFromSqlite() {
    try {
      const rows = await loadAmazonRowsFromSqlite(amazonDbPath.trim() || undefined)
      const objs: AmazonImportRow[] = rows
        .map(r => ({
          title: String(r.title ?? '').trim(),
          releaseYear: r.release_year ?? null,
          genres: r.genres ?? undefined,
          asin: r.asin ?? null,
          sku: r.sku ?? null,
          owned: r.owned ?? null,
        }))
        .filter(r => r.title.length > 0)

      if (!objs.length) {
        alert('No recognizable rows found in the selected Amazon SQLite DB.')
        return
      }

      objs.sort((a, b) => compareTitleCaseInsensitive(a.title, b.title))
      setAmazonRows(objs)
      setAmazonSelected(new Set(objs.map((_, i) => i)))
    } catch (e:any) {
      alert('Could not read Amazon SQLite: ' + (e?.message ?? String(e)))
    }
  }

  function toggleAmazonRow(i: number) {
    setAmazonSelected(prev => {
      const next = new Set(prev);
      if (next.has(i)) next.delete(i); else next.add(i);
      return next;
    });
  }

  function toggleAmazonSelectAll() {
    setAmazonSelected(prev => {
      if (prev.size === amazonRows.length) return new Set();
      return new Set(amazonRows.map((_, i) => i));
    });
  }

  async function runAmazonImport() {
    if (amazonSelected.size === 0) { alert('No rows selected.'); return; }

    const existingTitles = new Set(
      games
        .filter(g => normalizeTitle(g.platform) === normalizeTitle(AMAZON_PLATFORM))
        .map(g => libraryIdentityKey(g.title, g.platform))
    )
    const hiddenTitles = new Set(
      games
        .filter(g => normalizeTitle(g.platform) === normalizeTitle(AMAZON_PLATFORM) && !!g.hidden_at)
        .map(g => libraryIdentityKey(g.title, g.platform))
    )

    const selected = amazonRows
      .map((r, i) => ({ r, i }))
      .filter(x => amazonSelected.has(x.i));

    setAmazonImporting(true);
    let added = 0, skipped = 0, errored = 0;

    for (const { r } of selected) {
      const key = libraryIdentityKey(r.title, AMAZON_PLATFORM)
      if (hiddenTitles.has(key)) {
        skipped++; continue;
      }
      if (amazonSkipDupes && existingTitles.has(key)) {
        skipped++; continue;
      }
      try {
        const id = await addManualGame({
          canonicalTitle: r.title,
          platform: AMAZON_PLATFORM,
          beaten: false,
          genre: r.genres && r.genres.length ? r.genres.join(', ') : undefined,
        });
        if (r.releaseYear && Number.isFinite(r.releaseYear)) {
          try { await updateGame({ id, release_year: r.releaseYear }); } catch {}
        }
        added++;
        existingTitles.add(key);
      } catch (e: any) {
        console.warn('[Amazon Import] failed to add:', r.title, e);
        errored++;
      }
    }

    setAmazonImporting(false);
    await refresh();
    alert(`Amazon import complete.\nAdded: ${added}\nSkipped: ${skipped}\nErrors: ${errored}`);
    closeAmazonImport()
  }

  async function runGogImport() {
    if (gogImporting || gogSyncing) return
    try {
      setGogImporting(true)
      const count = await importGogOwnedFromSqlite(gogDbPath.trim() || undefined)
      alert(`Imported/updated ${count} GOG owned titles.`)
      setShowGogImport(false)
      setGogDbPath('')
      await refresh()
    } catch (e: any) {
      alert('GOG import failed: ' + (e?.message ?? String(e)))
    } finally {
      setGogImporting(false)
    }
  }

  async function runGogTimeSync() {
    if (gogImporting || gogSyncing) return
    try {
      setGogSyncing(true)
      const count = await syncGogPlaytimeFromSqlite(gogDbPath.trim() || undefined)
      alert(`Synced play time for ${count} existing GOG game${count === 1 ? '' : 's'}.`)
      setShowGogImport(false)
      setGogDbPath('')
      await refresh()
    } catch (e: any) {
      alert('GOG time sync failed: ' + (e?.message ?? String(e)))
    } finally {
      setGogSyncing(false)
    }
  }

  return (
    <div className="wrap">
      <header className="topbar">
        <div className="brand-panel">
          <span className="eyebrow">{screenView === 'queue' ? 'Queue' : 'Library'}</span>
          <div>
            <h1>GameLexicon</h1>
          </div>
          <div className="hero-stats">
            {screenView === 'queue' ? (
              <>
                <div className="hero-stat">
                  <span className="hero-stat-label">Queued</span>
                  <strong>{queueStats.totalGames.toLocaleString()}</strong>
                </div>
                <div className="hero-stat">
                  <span className="hero-stat-label">Playing</span>
                  <strong>{queueStats.playingCount.toLocaleString()}</strong>
                </div>
                <div className="hero-stat">
                  <span className="hero-stat-label">Time to Clear</span>
                  <strong>{formatQueueClearEstimate(queueStats.totalTtbSeconds)}</strong>
                </div>
                <div className="hero-stat">
                  <span className="hero-stat-label">Time Played</span>
                  <strong>{hoursOrZero(queueStats.totalPlaytimeMinutes)}</strong>
                </div>
              </>
            ) : (
              <>
                <div className="hero-stat">
                  <span className="hero-stat-label">Owned</span>
                  <strong>{libraryStats.totalGames.toLocaleString()}</strong>
                </div>
                <div className="hero-stat">
                  <span className="hero-stat-label">Tracked Time</span>
                  <strong>{hoursOrZero(libraryStats.totalPlaytimeMinutes)}</strong>
                </div>
                <div className="hero-stat">
                  <span className="hero-stat-label">Completed</span>
                  <strong>{formatPercent(libraryStats.completionPct)}</strong>
                </div>
              </>
            )}
          </div>
          {screenView === 'queue' && (
            <p className="brand-copy">
              Reorder by dragging rows, using the arrow buttons, or changing the queue number. The clear estimate is based on the preferred TTB value for {queueStats.ttbKnownCount.toLocaleString()} queued game{queueStats.ttbKnownCount === 1 ? '' : 's'}.
            </p>
          )}
          <div className="screen-tabs" role="tablist" aria-label="Primary view">
            <button
              type="button"
              className={`screen-tab ${screenView === 'library' ? 'is-active' : ''}`}
              onClick={() => setScreenView('library')}
            >
              Library
            </button>
            <button
              type="button"
              className={`screen-tab ${screenView === 'queue' ? 'is-active' : ''}`}
              onClick={() => setScreenView('queue')}
            >
              Queue
            </button>
          </div>
        </div>

        <div className="topbar-toolbar">
          <div className="action-cluster">
            <div className="action-cluster-label">Imports</div>
            <div className="actions">
              <button className="icon" title={HINTS.steamImport} onClick={() => setShowImport(s => !s)}>
                <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
                  <path d="M12 2C6.486 2 2 6.486 2 12l4.707 1.964A3.5 3.5 0 0 0 10.5 17h.15l2.547 1.364A10 10 0 1 0 12 2Zm6.5 6a2.5 2.5 0 1 1-4.999.001A2.5 2.5 0 0 1 18.5 8ZM9 18.5a1.5 1.5 0 1 1-.001-3.001A1.5 1.5 0 0 1 9 18.5Zm9.5-7.5a3.5 3.5 0 1 0-3.501-3.5A3.5 3.5 0 0 0 18.5 11Z"/></svg>
                <span>Steam Import</span>
              </button>
              <button className="icon" title={(HINTS && HINTS.amazonImport) || 'Import Amazon Games'} onClick={() => setShowAmazonImport(true)}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M19 13H5v-2h14v2zm-7 7l-5-5h10l-5 5zM7 4h10l-5 5-5-5z"/>
                </svg>
                <span>Amazon Games</span>
              </button>
              <button className="icon" title={HINTS.csvImport || 'Import games from a CSV file'} onClick={() => setShowCsvImport(true)}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M5 4h10a1 1 0 0 1 .7.29l3.01 3.01A1 1 0 0 1 19 8v12a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1Zm9 1.5V8h2.5L14 5.5ZM7 11h10v1.5H7V11Zm0 3.5h10V16H7v-1.5Zm0 3.5h7v1.5H7V18Z"/>
                </svg>
                <span>CSV Import</span>
              </button>
              <button className="icon" title={(HINTS && (HINTS as any).epicImport) || 'Import Epic (Legendary)'} onClick={() => setShowEpicImport(true)}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M5 4h14v2H5V4zm0 7h14v2H5v-2zm0 7h14v2H5v-2z"/>
                </svg>
                <span>Epic (Legendary)</span>
              </button>
              <button className="icon" title={HINTS.gogImport || 'Import GOG (owned only)'} onClick={() => setShowGogImport(true)}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M12 2a10 10 0 1 0 10 10A10.011 10.011 0 0 0 12 2Zm-1.8 14.6a4 4 0 1 1 5.6-5.7l-1.4 1.4a2 2 0 1 0 .6 1.4h-2v-2h4v1a4 4 0 0 1-7.8 1.9Z"/>
                </svg>
                <span>GOG (Owned)</span>
              </button>
              <button
                className="icon"
                title="Import Xbox (Owned)"
                onClick={async () => {
                  try {
                    const clientId = xboxClientId.trim()
                    if (!clientId) {
                      alert('Enter your Microsoft Application (client) ID in Settings first.')
                      return
                    }
                    const dc = await xboxBeginDeviceCode(clientId)
                    setXboxDC(dc)
                    setShowXboxLogin(true)
                  } catch (e: any) {
                    console.error(e)
                    alert('Could not start Xbox sign-in: ' + (e?.message ?? String(e)))
                  }
                }}
              >
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M12 2a10 10 0 1 0 10 10A10.011 10.011 0 0 0 12 2Zm3.5 12.5-1 1L12 13l-2.5 2.5-1-1L11 12 8.5 9.5l1-1L12 11l2.5-2.5 1 1L13 12l2.5 2.5Z"/>
                </svg>
                <span>Xbox (Owned)</span>
              </button>
            </div>
          </div>

          <div className="action-cluster">
            <div className="action-cluster-label">Backups</div>
            <div className="actions">
              <button className="icon" title={HINTS.libraryBackup} onClick={openLibraryBackupModal}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M5 4h14a1 1 0 0 1 1 1v11h-2V6H6v12h5v2H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1Zm8 8h6v2h-6v3l-4-4 4-4v3Z"/>
                </svg>
                <span>Library Backup</span>
              </button>
              <button className="icon" title={HINTS.queueBackup} onClick={openQueueBackupModal}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M4 5h12v2H4V5Zm0 6h12v2H4v-2Zm0 6h8v2H4v-2Zm13-8 4 4-4 4v-3h-4v-2h4V9Z"/>
                </svg>
                <span>Queue Backup</span>
              </button>
            </div>
          </div>

          <div className="action-cluster">
            <div className="action-cluster-label">Library Tools</div>
            <div className="actions">
              <button className="icon" title={HINTS.enrichAll} onClick={() => runIgdbEnrich(false)}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d="M12 2a10 10 0 1 0 10 10A10.011 10.011 0 0 0 12 2Zm1 15h-2v-2h2Zm0-4h-2V7h2Z"/></svg>
                <span>Enrich Missing</span>
              </button>
              <button className="icon" title={HINTS.enrichUnenriched || 'Enrich only games not yet enriched'} onClick={() => runIgdbEnrich(true)}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M12 2a10 10 0 1 0 10 10A10.011 10.011 0 0 0 12 2Zm4.3 7.7-5.1 5.1-3.5-3.5 1.4-1.4 2.1 2.1 3.7-3.7Z"/>
                </svg>
                <span>Enrich Unenriched</span>
              </button>
              <button className="icon" title={HINTS.myStats} onClick={() => setShowStats(true)}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M4 19h16v2H4v-2Zm1-2V9h3v8H5Zm5 0V5h4v12h-4Zm6 0v-6h3v6h-3Z"/>
                </svg>
                <span>My Stats</span>
              </button>
              <button className="icon" title={HINTS.randomGame || 'Pick a random game from your library'} onClick={openRandomGamePicker}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                  <path d="M4 7a3 3 0 1 1 3 3H4V7Zm13 0a3 3 0 1 1 3 3h-3V7ZM4 17a3 3 0 1 1 3 3H4v-3Zm13 0a3 3 0 1 1 3 3h-3v-3ZM9 8h6v2H9V8Zm0 6h6v2H9v-2Z"/>
                </svg>
                <span>Random Game</span>
              </button>
              <button className="icon" title={HINTS.settings} onClick={() => setShowSettings(true)}>
                <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d="M19.14,12.94a7.43,7.43,0,0,0,0-1.88l2.11-1.65a.5.5,0,0,0,.12-.64l-2-3.46a.5.5,0,0,0-.6-.22l-2.49,1a7.28,7.28,0,0,0-1.62-.94l-.38-2.65A.5.5,0,0,0,13.9,2H10.1a.5.5,0,0,0-.5.42L9.22,5.07a7.28,7.28,0,0,0-1.62.94l-2.49-1a.5.5,0,0,0-.6.22l-2,3.46a.5.5,0,0,0,.12.64L4.86,11.06a7.43,7.43,0,0,0,0,1.88L2.75,14.59a.5.5,0,0,0-.12.64l2,3.46a.5.5,0,0,0,.6.22l2-3.46a.5.5,0,0,0,.12-.64ZM12,15.5A3.5,3.5,0,1,1,15.5,12,3.5,3.5,0,0,1,12,15.5Z"/></svg>
                <span>Settings</span>
              </button>
            </div>
          </div>
        </div>
      </header>

      {prog && !progCollapsed && (
        <div className="footer-progress" style={{position:'fixed', left:0, right:0, bottom:0}}>
          <div className="footer-row" style={{display:'flex', alignItems:'center', gap:8, padding:'8px 12px', background:'#0b0e14', borderTop:'1px solid rgba(255,255,255,0.08)'}}>
            <div className="bar" style={{flex:1, height:8, background:'rgba(255,255,255,0.08)', borderRadius:8, overflow:'hidden'}}>
              <div className="fill" style={{width: pct + '%', height:'100%', background:'#3b82f6'}}/>
            </div>
            <div className="msg" style={{width:340, whiteSpace:'nowrap', overflow:'hidden', textOverflow:'ellipsis'}}>
              {prog.total > 0 ? `${prog.done}/${prog.total}` : (prog.title ? '' : 'Starting...')}
              {prog.title ? ` ${prog.title}` : ''}
            </div>
            <button
              onClick={()=>setProgCollapsed(true)}
              title={HINTS.hideProgress}
              style={{marginLeft:8, width:24, height:24, borderRadius:999, border:'1px solid rgba(255,255,255,0.2)', background:'#0f172a', color:'#9ae6b4', cursor:'pointer'}}
            >-</button>
          </div>
        </div>
      )}
      {prog && progCollapsed && (
        <button
          onClick={() => setProgCollapsed(false)}
          title="Show progress"
          style={{
            position:'fixed',
            right:14,
            bottom:14,
            zIndex:2100,
            display:'flex',
            alignItems:'center',
            gap:8,
            padding:'8px 10px',
            borderRadius:999,
            border:'1px solid rgba(255,255,255,0.18)',
            background:'#0b0e14',
            color:'#fff',
            cursor:'pointer',
            boxShadow:'0 8px 20px rgba(0,0,0,0.35)',
          }}
        >
          <svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true">
            <circle cx="12" cy="12" r="9" fill="none" stroke="rgba(255,255,255,0.25)" strokeWidth="3" />
            <path d="M12 3a9 9 0 0 1 0 18" fill="none" stroke="#60a5fa" strokeWidth="3" strokeLinecap="round">
              <animateTransform
                attributeName="transform"
                type="rotate"
                from="0 12 12"
                to="360 12 12"
                dur="0.9s"
                repeatCount="indefinite"
              />
            </path>
          </svg>
          <span style={{fontSize:12, whiteSpace:'nowrap'}}>
            Enriching... {prog.total > 0 ? `${prog.done}/${prog.total}` : ''}
          </span>
        </button>
      )}

      {showEpicImport && (
        <div className="modal">
          <div className="modal-card" style={{ width: 820, maxWidth: '95vw' }}>
            <h3>Import Epic library (Legendary)</h3>

            {!epicOwned.length ? (
              <>
                <p style={{ opacity: 0.85 }}>
                  Fetch your owned Epic games via the <b>Legendary</b> CLI.
                  Make sure you’ve run <code>legendary auth</code> once. If Legendary isn’t in PATH,
                  paste the full path below (e.g., <code>C:\Tools\legendary.exe</code>).
                </p>
                <div className="import-row">
                  <label>Legendary path (optional)</label>
                  <input
                    placeholder="legendary"
                    value={legendaryPath}
                    onChange={e => setLegendaryPath(e.target.value)}
                  />
                </div>
                <div className="import-actions">
                  <button onClick={() => setShowEpicImport(false)}>Close</button>
                  <button className="run" onClick={fetchEpicOwned}>Fetch Owned</button>
                </div>
                <div className="tip">
                  We try <code>legendary list-games --json</code> first, then fall back to parsing plaintext output.
                </div>
              </>
            ) : (
              <>
                <div style={{ display: 'flex', gap: 10, alignItems: 'center', marginBottom: 8 }}>
                  <label className="check">
                    <input
                      type="checkbox"
                      checked={epicSelected.size === epicOwned.length}
                      onChange={toggleEpicSelectAll}
                    />
                    <span>Select all</span>
                  </label>
                  <label className="check" title="Skip titles that already exist in your library (any platform).">
                    <input
                      type="checkbox"
                      checked={epicSkipDupes}
                      onChange={e => setEpicSkipDupes(e.target.checked)}
                    />
                    <span>Skip duplicates</span>
                  </label>
                  <div style={{ marginLeft: 'auto', opacity: 0.8 }}>
                    {epicSelected.size} of {epicOwned.length} selected
                  </div>
                </div>

                <div style={{ maxHeight: '45vh', overflow: 'auto', border: '1px solid rgba(255,255,255,0.08)', borderRadius: 8 }}>
                  <table className="grid" style={{ margin: 0 }}>
                    <thead>
                      <tr>
                        <th></th>
                        <th>Title</th>
                        <th>App Name</th>
                      </tr>
                    </thead>
                    <tbody>
                      {epicOwned.map((r, i) => {
                        const exists = games.some(
                          g => g.platform === 'Epic' &&
                               normalizeTitleLocal(g.title) === normalizeTitleLocal(r.title)
                        )
                        return (
                          <tr key={i}>
                            <td>
                              <input
                                type="checkbox"
                                checked={epicSelected.has(i)}
                                onChange={() => toggleEpicRow(i)}
                              />
                            </td>
                            <td>
                              {r.title}
                              {exists && <span style={{ marginLeft: 8, fontSize: 12, opacity: 0.65 }}>(already in library)</span>}
                            </td>
                            <td>{r.appName}</td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                </div>

                <div className="import-actions" style={{ marginTop: 10 }}>
                  <button onClick={() => setShowEpicImport(false)} disabled={epicImporting}>Close</button>
                  <button className="run" onClick={runEpicImport} disabled={epicImporting || epicSelected.size === 0}>
                    {epicImporting ? 'Importing…' : 'Import Selected'}
                  </button>
                </div>
                <div className="tip">
                  Imported games are added with platform “Epic”. You can enrich details via IGDB later.
                </div>
              </>
            )}
          </div>
        </div>
      )}
{showXboxLogin && xboxDC && (
  <div className="modal">
    <div className="modal-card" style={{ width: 620, maxWidth: '95vw' }}>
      <h3>Sign in to Microsoft</h3>
      {/* Microsoft returns a friendly HTML string in `message` */}
      <div
        style={{ margin: '8px 0', opacity: 0.9, lineHeight: 1.5 }}
        dangerouslySetInnerHTML={{ __html: xboxDC.message }}
      />
      <div className="import-row" style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <label style={{ minWidth: 80 }}>Code</label>
        <code style={{ fontSize: 18 }}>{xboxDC.user_code}</code>
      </div>
      <div className="import-row" style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <label style={{ minWidth: 80 }}>URL</label>
        <a
          href={xboxDC.verification_uri_complete ?? xboxDC.verification_uri}
          target="_blank"
          rel="noreferrer"
        >
          {xboxDC.verification_uri}
        </a>
      </div>

      <div className="import-actions" style={{ marginTop: 14 }}>
        <button
          onClick={() => { setShowXboxLogin(false); setXboxDC(null); }}
        >
          Cancel
        </button>
        <button
  className="run"
  onClick={async () => {
    try {
      if (!xboxDC) return;
      const clientId = xboxClientId.trim()
      if (!clientId) {
        alert('Enter your Microsoft Application (client) ID in Settings first.')
        return
      }
      const n = await xboxFinishAndImportOwned(xboxDC.device_code, clientId, 'US', 'en-US');
      alert(`Imported/updated ${n} Xbox owned titles.`);
      setShowXboxLogin(false);
      setXboxDC(null);
      await refresh();
    } catch (e: any) {
      alert('Xbox import failed: ' + (e?.message ?? String(e)));
    }
  }}
>
  I’ve signed in — Continue
</button>

      </div>

      <div className="tip" style={{ marginTop: 8 }}>
        You don’t need a client secret. The device-code flow uses only your Application (client) ID.
      </div>
    </div>
  </div>
)}

      {showLibraryBackup && (
        <div className="modal">
          <div className="modal-card" style={{ width: 760, maxWidth: '95vw' }}>
            <h3>Library Backup</h3>
            <p style={{ opacity: 0.82 }}>
              Export your full library to CSV, or import a CSV backup to merge it into the current library.
              Only the game title is required when importing. Platform and all other fields are optional.
            </p>
            <div className="import-row">
              <label>Export Path</label>
              <input
                value={libraryBackupPath}
                onChange={e => setLibraryBackupPath(e.target.value)}
                placeholder="C:\\Users\\you\\Downloads\\gamelexicon-library-backup.csv"
                disabled={libraryBackupBusy}
              />
            </div>
            <div className="import-actions" style={{ justifyContent: 'space-between', marginTop: 12 }}>
              <button onClick={() => setShowLibraryBackup(false)} disabled={libraryBackupBusy}>Close</button>
              <button className="run" onClick={downloadLibraryBackup} disabled={libraryBackupBusy}>
                {libraryBackupBusy ? 'Working...' : 'Save Library CSV'}
              </button>
            </div>
            <hr style={{ borderColor: 'rgba(255,255,255,0.08)', margin: '16px 0' }} />
            <div className="import-row">
              <label>Import Library CSV</label>
              <input
                type="file"
                accept=".csv,text/csv"
                disabled={libraryBackupBusy}
                onChange={e => {
                  const file = e.currentTarget.files?.[0]
                  e.currentTarget.value = ''
                  if (file) {
                    importLibraryBackupFile(file).catch(err => {
                      alert('Could not import library backup: ' + (err?.message ?? String(err)))
                    })
                  }
                }}
              />
            </div>
            <div className="tip">
              The export path is editable, so you can save the backup wherever you want. If the folder does not exist yet,
              the app will create it for you.
            </div>
            <div className="tip" style={{ marginTop: 10 }}>
              Supported headers include <code>Title</code>, <code>Platform</code>, <code>Beaten</code>, <code>Genre</code>,
              <code> Playtime Minutes</code>, <code>Last Played</code>, <code>Release Year</code>, <code>Notes</code>,
              and review/license fields. Missing details can be enriched later.
            </div>
            <div className="tip" style={{ marginTop: 10 }}>
              Use this backup import to restore or merge saved library data. For create-only spreadsheet imports that should skip existing title + platform matches, use the separate <code>CSV Import</code> tool.
            </div>
          </div>
        </div>
      )}

      {showQueueBackup && (
        <div className="modal">
          <div className="modal-card" style={{ width: 760, maxWidth: '95vw' }}>
            <h3>Queue Backup</h3>
            <p style={{ opacity: 0.82 }}>
              Export the current queue to CSV, or restore the queue from a CSV backup. Restoring replaces the current
              queue order and queue state. If a queued title is missing from the library, it will be recreated as a
              manual entry during restore.
            </p>
            <div className="import-row">
              <label>Export Path</label>
              <input
                value={queueBackupPath}
                onChange={e => setQueueBackupPath(e.target.value)}
                placeholder="C:\\Users\\you\\Downloads\\gamelexicon-queue-backup.csv"
                disabled={queueBackupBusy}
              />
            </div>
            <div className="import-actions" style={{ justifyContent: 'space-between', marginTop: 12 }}>
              <button onClick={() => setShowQueueBackup(false)} disabled={queueBackupBusy}>Close</button>
              <button className="run" onClick={downloadQueueBackup} disabled={queueBackupBusy}>
                {queueBackupBusy ? 'Working...' : 'Save Queue CSV'}
              </button>
            </div>
            <hr style={{ borderColor: 'rgba(255,255,255,0.08)', margin: '16px 0' }} />
            <div className="import-row">
              <label>Restore Queue From CSV</label>
              <input
                type="file"
                accept=".csv,text/csv"
                disabled={queueBackupBusy}
                onChange={e => {
                  const file = e.currentTarget.files?.[0]
                  e.currentTarget.value = ''
                  if (file) {
                    importQueueBackupFile(file).catch(err => {
                      alert('Could not restore queue backup: ' + (err?.message ?? String(err)))
                    })
                  }
                }}
              />
            </div>
            <div className="tip">
              The export path is editable, so you can save the queue backup to any folder and filename you want.
            </div>
            <div className="tip" style={{ marginTop: 10 }}>
              Recommended headers are <code>Queue Position</code>, <code>Title</code>, <code>Platform</code>,
              <code> Playing</code>, <code>Started Playing</code>, <code>Finished</code>, and
              <code> Playtime Minutes</code>. Title is the only required field.
            </div>
          </div>
        </div>
      )}

      {/* Settings modal */}
      {showSettings && (
        <div className="modal">
          <div className="modal-card settings-modal">
            <h3>Settings</h3>
            <div className="settings-layout">
              <div className="settings-column">
                <section className="settings-section">
                  <div className="settings-section-heading">
                    <div>
                      <span className="panel-kicker">Accounts</span>
                      <h4>API Credentials</h4>
                    </div>
                    <div className="panel-note">Stored securely in your system keychain.</div>
                  </div>
                  <div className="import-row">
                    <label>Twitch Client ID</label>
                    <input value={twId} onChange={e => setTwId(e.target.value)} />
                  </div>
                  <div className="import-row">
                    <label>Microsoft (Entra) Client ID</label>
                    <input
                      placeholder="Application (client) ID"
                      value={xboxClientId}
                      onChange={e => setXboxClientId(e.target.value)}
                    />
                  </div>
                  <div className="import-row">
                    <label>Twitch Client Secret</label>
                    <input value={twSecret} onChange={e => setTwSecret(e.target.value)} />
                  </div>
                </section>

                <section className="settings-section">
                  <div className="settings-section-heading">
                    <div>
                      <span className="panel-kicker">Library Lists</span>
                      <h4>Platforms + Genres</h4>
                    </div>
                    <div className="panel-note">One entry per line. Removing an item stops suggesting it for new rows, but existing game values stay untouched.</div>
                  </div>

                  <div className="settings-list-grid">
                    <div className="import-row">
                      <label>Platform Options</label>
                      <textarea
                        className="settings-list-editor"
                        rows={10}
                        value={platformOptionsDraftText}
                        disabled={customizationSaving || backgroundUploading}
                        onChange={e => setPlatformOptionsDraftText(e.target.value)}
                      />
                      <div className="settings-list-actions">
                        <button
                          type="button"
                          className="ghost-action"
                          disabled={customizationSaving || backgroundUploading}
                          onClick={() => setPlatformOptionsDraftText(formatOptionEditorValue(DEFAULT_PLATFORM_OPTIONS))}
                        >
                          Reset Platform Defaults
                        </button>
                      </div>
                    </div>

                    <div className="import-row">
                      <label>Genre Options</label>
                      <textarea
                        className="settings-list-editor"
                        rows={10}
                        value={genreOptionsDraftText}
                        disabled={customizationSaving || backgroundUploading}
                        onChange={e => setGenreOptionsDraftText(e.target.value)}
                      />
                      <div className="settings-list-actions">
                        <button
                          type="button"
                          className="ghost-action"
                          disabled={customizationSaving || backgroundUploading}
                          onClick={() => setGenreOptionsDraftText(formatOptionEditorValue(DEFAULT_GENRE_OPTIONS))}
                        >
                          Reset Genre Defaults
                        </button>
                      </div>
                    </div>
                  </div>
                </section>
              </div>

              <div className="settings-column">
                <section className="settings-section">
                  <div className="settings-section-heading">
                    <div>
                      <span className="panel-kicker">Customization</span>
                      <h4>Theme + Background</h4>
                    </div>
                    <button
                      type="button"
                      className="ghost-action"
                      onClick={resetCustomizationDraft}
                      disabled={customizationSaving || backgroundUploading}
                    >
                      Reset Defaults
                    </button>
                  </div>

                  <div className="color-grid">
                    <label className="color-control">
                      <span className="field-label">Buttons</span>
                      <input
                        type="color"
                        value={customizationDraft.buttonColor}
                        onChange={e =>
                          setCustomizationDraft(prev => ({ ...prev, buttonColor: e.target.value }))
                        }
                        disabled={customizationSaving || backgroundUploading}
                      />
                      <code>{customizationDraft.buttonColor}</code>
                    </label>
                    <label className="color-control">
                      <span className="field-label">Text</span>
                      <input
                        type="color"
                        value={customizationDraft.textColor}
                        onChange={e =>
                          setCustomizationDraft(prev => ({ ...prev, textColor: e.target.value }))
                        }
                        disabled={customizationSaving || backgroundUploading}
                      />
                      <code>{customizationDraft.textColor}</code>
                    </label>
                    <label className="color-control">
                      <span className="field-label">Game Card</span>
                      <input
                        type="color"
                        value={customizationDraft.cardBackgroundColor}
                        onChange={e =>
                          setCustomizationDraft(prev => ({
                            ...prev,
                            cardBackgroundColor: e.target.value,
                          }))
                        }
                        disabled={customizationSaving || backgroundUploading}
                      />
                      <code>{customizationDraft.cardBackgroundColor}</code>
                    </label>
                    <label className="color-control">
                      <span className="field-label">Sections</span>
                      <input
                        type="color"
                        value={customizationDraft.sectionBackgroundColor}
                        onChange={e =>
                          setCustomizationDraft(prev => ({
                            ...prev,
                            sectionBackgroundColor: e.target.value,
                          }))
                        }
                        disabled={customizationSaving || backgroundUploading}
                      />
                      <code>{customizationDraft.sectionBackgroundColor}</code>
                    </label>
                  </div>

                  <div className="import-row">
                    <label>Upload Background Image</label>
                    <input
                      type="file"
                      accept="image/png,image/jpeg,image/webp,image/gif,image/bmp,image/svg+xml"
                      disabled={customizationSaving || backgroundUploading}
                      onChange={e => {
                        const file = e.currentTarget.files?.[0]
                        e.currentTarget.value = ''
                        if (file) {
                          uploadCustomizationBackgroundFile(file)
                        }
                      }}
                    />
                  </div>

                  <div className="import-row">
                    <label>Active Background</label>
                    <select
                      value={customizationDraft.backgroundImageName ?? ''}
                      disabled={customizationSaving || backgroundUploading}
                      onChange={async e => {
                        try {
                          await selectCustomizationBackground(e.target.value)
                        } catch (err: any) {
                          alert('Could not load that background image: ' + (err?.message ?? String(err)))
                        }
                      }}
                    >
                      <option value="">No image</option>
                      {customizationDraft.availableBackgrounds.map(name => (
                        <option key={name} value={name}>{name}</option>
                      ))}
                    </select>
                  </div>

                  <div className="tip">
                    Uploaded images are stored in <code>%LOCALAPPDATA%\GameLexicon\backgrounds\</code>.
                    Button label contrast is adjusted automatically from your chosen button color.
                  </div>

                  <div className="settings-background-preview">
                    {customizationDraft.backgroundImageDataUrl ? (
                      <img
                        src={customizationDraft.backgroundImageDataUrl}
                        alt="Background preview"
                      />
                    ) : (
                      <div className="settings-background-empty">
                        No uploaded background selected. The built-in backdrop stays active.
                      </div>
                    )}
                  </div>
                </section>
              </div>
            </div>
            <div className="import-actions">
              <button onClick={closeSettingsModal} disabled={customizationSaving || backgroundUploading}>Close</button>
              <button
                className="run"
                onClick={async ()=>{await saveSettings();}}
                disabled={customizationSaving || backgroundUploading}
              >
                {customizationSaving ? 'Saving...' : (backgroundUploading ? 'Uploading...' : 'Save')}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Steam Import modal */}
      {showImport && (
        <div className="modal">
          <div className="modal-card" style={{ width: 760, maxWidth: '95vw' }}>
            <h3>Import or Sync Steam Library</h3>
          <div className="import-row">
            <label>Steam API Key <span className="muted">(recommended)</span></label>
            <input placeholder="Paste Steam Web API key" value={steamApiKey} onChange={e => setSteamApiKey(e.target.value)} />
          </div>
          <div className="import-row">
            <label>Profile (SteamID64, vanity, or profile URL)</label>
            <input placeholder="Paste SteamID64, vanity name, or profile URL" value={steamProfile} onChange={e => setSteamProfile(e.target.value)} />
          </div>
          <div className="import-actions">
            <button onClick={() => setShowImport(false)} disabled={importing || steamSyncing}>Cancel</button>
            <button className="run" onClick={runSteamTimeSync} disabled={importing || steamSyncing}>
              {steamSyncing ? 'Syncing...' : 'Time Sync Only'}
            </button>
            <button className="run" onClick={runSteamImport} disabled={importing || steamSyncing}>
              {importing ? 'Importing...' : 'Run Import'}
            </button>
          </div>
          <div className="tip">
            API key is recommended. Leaving it empty uses Steam Community XML, which requires a public profile plus public game details and can still be less reliable.
          </div>
          <div className="tip">
            Time sync updates play time for existing Steam-connected rows only. It does not add missing games.
          </div>
          </div>
        </div>
      )}

      {/* General library CSV import modal */}
      {showCsvImport && (
        <div className="modal">
          <div className="modal-card" style={{ width: 860, maxWidth: '95vw' }}>
            <h3>Import Library CSV</h3>

            {!csvImportRows.length ? (
              <>
                <p style={{ opacity: 0.82 }}>
                  Add new library entries from a CSV file. This importer requires <code>Title</code> and <code>Platform</code>.
                  Optional columns like <code>Genre</code>, <code>Beaten</code>, <code>Playtime Minutes</code>,
                  <code> Last Played</code>, <code>Release Year</code>, notes, review fields, and license fields are applied when present.
                </p>
                <div className="import-row">
                  <label>CSV File</label>
                  <input
                    type="file"
                    accept=".csv,text/csv"
                    disabled={csvImporting}
                    onChange={e => {
                      const file = e.currentTarget.files?.[0]
                      e.currentTarget.value = ''
                      if (file) {
                        handleLibraryCsvFile(file).catch(err => {
                          alert('Could not read CSV: ' + (err?.message ?? String(err)))
                        })
                      }
                    }}
                  />
                </div>
                <div className="tip">
                  Existing matches are checked by exact title + platform, so the same title can still be imported for another platform.
                </div>
                <div className="tip" style={{ marginTop: 10 }}>
                  Header aliases are supported for common variations like <code>System</code>, <code>Console</code>,
                  <code> Genres</code>, <code>Playtime Hours</code>, and <code>Release Date</code>.
                </div>
                <div className="import-actions" style={{ marginTop: 10 }}>
                  <button onClick={closeLibraryCsvImport} disabled={csvImporting}>Close</button>
                </div>
              </>
            ) : (
              <>
                <div style={{ display: 'flex', gap: 10, alignItems: 'center', marginBottom: 8 }}>
                  <label className="check">
                    <input
                      type="checkbox"
                      checked={csvImportSelected.size === csvImportRows.length}
                      onChange={toggleLibraryCsvImportSelectAll}
                    />
                    <span>Select all</span>
                  </label>
                  <div style={{ opacity: 0.8 }}>
                    {csvImportFileName || 'CSV loaded'}
                    {csvImportRejectedCount > 0 ? ` - ${csvImportRejectedCount} row${csvImportRejectedCount === 1 ? '' : 's'} skipped during parsing` : ''}
                  </div>
                  <div style={{ marginLeft: 'auto', opacity: 0.8 }}>
                    {csvImportSelected.size} of {csvImportRows.length} selected
                  </div>
                </div>

                <div style={{ maxHeight: '45vh', overflow: 'auto', border: '1px solid rgba(255,255,255,0.08)', borderRadius: 8 }}>
                  <table className="grid" style={{ margin: 0 }}>
                    <thead>
                      <tr>
                        <th></th>
                        <th>Title</th>
                        <th>Platform</th>
                        <th>Import Data</th>
                        <th>Status</th>
                      </tr>
                    </thead>
                    <tbody>
                      {csvImportRows.map((row, i) => {
                        const exists = games.some(game => libraryIdentityKey(game.title, game.platform) === libraryIdentityKey(row.title, row.platform))
                        return (
                          <tr key={`${row.title}-${row.platform}-${i}`}>
                            <td>
                              <input
                                type="checkbox"
                                checked={csvImportSelected.has(i)}
                                onChange={() => toggleLibraryCsvImportRow(i)}
                              />
                            </td>
                            <td>{row.title}</td>
                            <td>{row.platform}</td>
                            <td>{summarizeLibraryCsvImportRow(row)}</td>
                            <td>{exists ? 'Already in library' : 'New entry'}</td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                </div>

                <div className="import-actions" style={{ marginTop: 10 }}>
                  <button onClick={closeLibraryCsvImport} disabled={csvImporting}>Close</button>
                  <button className="run" onClick={runLibraryCsvImport} disabled={csvImporting || csvImportSelected.size === 0}>
                    {csvImporting ? 'Importing...' : 'Import Selected'}
                  </button>
                </div>
                <div className="tip">
                  Exact title + platform duplicates are skipped. This importer only adds new library entries; it does not merge into existing ones.
                </div>
              </>
            )}
          </div>
        </div>
      )}

      {/* Amazon library import modal */}
      {showAmazonImport && (
        <div className="modal">
          <div className="modal-card" style={{ width: 820, maxWidth: '95vw' }}>
            <h3>Import Amazon Games Library</h3>

            {!amazonRows.length ? (
              <>
                <p style={{ opacity: 0.8 }}>
                  Load directly from your local Amazon Games SQLite database.
                </p>
                <div className="import-row">
                  <label>Amazon SQLite path <span className="muted">(optional)</span></label>
                  <input
                    placeholder="Leave blank to auto-detect under LOCALAPPDATA\\Amazon Games\\Data"
                    value={amazonDbPath}
                    onChange={e => setAmazonDbPath(e.target.value)}
                  />
                </div>
                <div className="import-actions" style={{ marginTop: 6 }}>
                  <button className="run" onClick={loadAmazonFromSqlite}>Load from SQLite</button>
                </div>
                <div className="tip">
                  Auto-detect tries common Amazon Games DB locations. If it misses, paste a full SQLite path.
                </div>
                <div className="tip" style={{ marginTop: 10 }}>
                  If you have a general spreadsheet instead of the local Amazon database, use the separate <code>CSV Import</code> tool in the Imports section.
                </div>
                <div className="import-actions" style={{ marginTop: 10 }}>
                  <button onClick={closeAmazonImport}>Close</button>
                </div>
              </>
            ) : (
              <>
                <div style={{ display: 'flex', gap: 10, alignItems: 'center', marginBottom: 8 }}>
                  <label className="check">
                    <input
                      type="checkbox"
                      checked={amazonSelected.size === amazonRows.length}
                      onChange={toggleAmazonSelectAll}
                    />
                    <span>Select all</span>
                  </label>
                  <label className="check" title="Skip Amazon entries that already exist in your library with the same title and Amazon platform.">
                    <input
                      type="checkbox"
                      checked={amazonSkipDupes}
                      onChange={e => setAmazonSkipDupes(e.target.checked)}
                    />
                    <span>Skip duplicates</span>
                  </label>
                  <div style={{ marginLeft: 'auto', opacity: 0.8 }}>
                    {amazonSelected.size} of {amazonRows.length} selected
                  </div>
                </div>

                <div style={{ maxHeight: '45vh', overflow: 'auto', border: '1px solid rgba(255,255,255,0.08)', borderRadius: 8 }}>
                  <table className="grid" style={{ margin: 0 }}>
                    <thead>
                      <tr>
                        <th></th>
                        <th>Title</th>
                        <th>Release</th>
                        <th>Genres</th>
                        <th>ASIN</th>
                        <th>SKU</th>
                      </tr>
                    </thead>
                    <tbody>
                      {amazonRows.map((r, i) => {
                        const exists = games.some(g => libraryIdentityKey(g.title, g.platform) === libraryIdentityKey(r.title, AMAZON_PLATFORM))
                        return (
                          <tr key={i}>
                            <td>
                              <input
                                type="checkbox"
                                checked={amazonSelected.has(i)}
                                onChange={() => toggleAmazonRow(i)}
                              />
                            </td>
                            <td>{r.title}{exists && <span style={{ marginLeft: 8, fontSize: 12, opacity: 0.65 }}>(already in Amazon library)</span>}</td>
                            <td>{r.releaseYear ?? ''}</td>
                            <td>{(r.genres ?? []).join(', ')}</td>
                            <td>{r.asin ?? ''}</td>
                            <td>{r.sku ?? ''}</td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                </div>

                <div className="import-actions" style={{ marginTop: 10 }}>
                  <button onClick={closeAmazonImport} disabled={amazonImporting}>Close</button>
                  <button className="run" onClick={runAmazonImport} disabled={amazonImporting || amazonSelected.size === 0}>
                    {amazonImporting ? 'Importing…' : 'Import Selected'}
                  </button>
                </div>
                <div className="tip">
                  Imported games are added with platform "Amazon". If a release year is present, it will be set after creation.
                </div>
              </>
            )}
          </div>
        </div>
      )}

      {showGogImport && (
        <div className="modal">
          <div className="modal-card" style={{ width: 760, maxWidth: '95vw' }}>
            <h3>Import or Sync GOG Owned Library</h3>
            <p style={{ opacity: 0.82 }}>
              Imports only games owned on GOG from Galaxy's local SQLite DB.
              Connected-platform entries (Steam/Epic/Xbox/etc.) are excluded.
            </p>
            <div className="import-row">
              <label>GOG Galaxy SQLite path <span className="muted">(optional)</span></label>
              <input
                placeholder="Leave blank to auto-detect C:\\ProgramData\\GOG.com\\Galaxy\\storage\\galaxy-2.0.db"
                value={gogDbPath}
                onChange={e => setGogDbPath(e.target.value)}
              />
            </div>
            <div className="import-actions" style={{ marginTop: 8 }}>
              <button onClick={() => { if (!gogImporting && !gogSyncing) { setShowGogImport(false); setGogDbPath(''); } }} disabled={gogImporting || gogSyncing}>
                Close
              </button>
              <button className="run" onClick={runGogTimeSync} disabled={gogImporting || gogSyncing}>
                {gogSyncing ? 'Syncing...' : 'Time Sync Only'}
              </button>
              <button className="run" onClick={runGogImport} disabled={gogImporting || gogSyncing}>
                {gogImporting ? 'Importing...' : 'Import Owned GOG Games'}
              </button>
            </div>
            <div className="tip">
              Source priority: release key prefix <code>gog_*</code> + <code>isOwned=1</code> in Galaxy DB.
            </div>
            <div className="tip">
              Time sync updates play time for existing GOG-connected rows only. It does not add missing games.
            </div>
          </div>
        </div>
      )}

      {showStats && (
        <div className="modal" style={{ zIndex: 2900 }} onClick={() => setShowStats(false)}>
          <div
            role="dialog"
            aria-modal="true"
            aria-label="My Stats"
            className="modal-card stats-modal"
            onClick={e => e.stopPropagation()}
          >
            <div className="stats-header">
              <div className="stats-header-copy">
                <div style={{fontSize:24, fontWeight:700}}>My Stats</div>
                <div className="stats-copy" style={{ marginTop: 4 }}>
                  Totals are based on your tracked library and exclude hidden entries.
                </div>
              </div>
              <button onClick={() => setShowStats(false)} title={HINTS.closeModal}>
                Close
              </button>
            </div>

            <div className="stats-grid">
              <div className="stats-panel">
                <div className="stats-kicker">Total Games Owned</div>
                <div className="stats-number">{libraryStats.totalGames.toLocaleString()}</div>
                <div className="stats-copy">Games currently tracked in your library.</div>
              </div>

              <div className="stats-panel">
                <div className="stats-kicker">Total Play Time</div>
                <div className="stats-number">{hoursOrZero(libraryStats.totalPlaytimeMinutes)}</div>
                <div className="stats-copy">Combined playtime from all tracked games.</div>
              </div>

              <div className="stats-panel">
                <div className="stats-kicker">Completion</div>
                <div className="stats-number">{formatPercent(libraryStats.completionPct)}</div>
                <div className="stats-copy">
                  {libraryStats.beatenGames.toLocaleString()} beaten / {libraryStats.totalGames.toLocaleString()} owned
                </div>
                <div className="stats-progress-track">
                  <div
                    className="stats-progress-fill"
                    style={{ width:`${Math.min(100, libraryStats.completionPct)}%` }}
                  />
                </div>
              </div>
            </div>

            <div className="stats-list-panel">
              <div className="stats-list-header">
                <div style={{fontSize:18, fontWeight:700}}>Top 5 Most Played</div>
                <div className="stats-copy" style={{ marginTop: 0 }}>Ranked by tracked time played</div>
              </div>

              {libraryStats.topPlayed.length > 0 ? (
                <div className="stats-list">
                  {libraryStats.topPlayed.map((game, index) => {
                    const minutes = Math.max(0, game.playtime_minutes ?? 0)
                    const width = topPlayedMaxMinutes > 0 ? (minutes / topPlayedMaxMinutes) * 100 : 0

                    return (
                      <div key={game.id} className="stats-row">
                        <div className="stats-rank">{index + 1}</div>
                        <div style={{minWidth:0}}>
                          <div style={{fontWeight:600, whiteSpace:'nowrap', overflow:'hidden', textOverflow:'ellipsis'}}>
                            {game.title}
                          </div>
                          <div className="stats-platform">{game.platform}</div>
                          <div className="stats-bar">
                            <div
                              className="stats-bar-fill"
                              style={{ width:`${Math.max(8, width)}%` }}
                            />
                          </div>
                        </div>
                        <div className="stats-hours">{hoursOrZero(minutes)}</div>
                      </div>
                    )
                  })}
                </div>
              ) : (
                <div className="stats-empty">
                  No playtime has been tracked yet, so the top-five list is still empty.
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {showRandomGame && !card && (
        <div className="modal">
          <div
            className="modal-card random-game-modal"
            role="dialog"
            aria-modal="true"
            aria-label="Random Game Picker"
            style={{ width:780, maxWidth:'95vw' }}
          >
            <div style={{display:'flex', alignItems:'flex-start', gap:12}}>
              <div className="game-card-title-block">
                <div style={{fontSize:20, fontWeight:700}}>Random Game</div>
                <div style={{color:'var(--text-muted)', fontSize:13, marginTop:4}}>
                  Leave every filter blank for a totally random pick. Hidden games are skipped.
                </div>
              </div>
              <button
                onClick={closeRandomGameFlow}
                title={HINTS.closeModal}
              >
                Close
              </button>
            </div>

            <div className="random-grid" style={{
              display:'grid',
              gridTemplateColumns:'repeat(auto-fit, minmax(180px, 1fr))',
              gap:12,
              marginTop:18,
            }}>
              <label className="import-row" style={{display:'grid', gap:6}}>
                <span className="field-label">Platform</span>
                <select
                  value={randomPlatform}
                  onChange={e => setRandomPlatform(e.target.value)}
                >
                  <option value="">Any platform</option>
                  {randomGamePlatformOptions.map(platformName => (
                    <option key={platformName} value={platformName}>{platformName}</option>
                  ))}
                </select>
              </label>

              <label className="import-row" style={{display:'grid', gap:6}}>
                <span className="field-label">Genre</span>
                <select
                  value={randomGenre}
                  onChange={e => setRandomGenre(e.target.value)}
                >
                  <option value="">Any genre</option>
                  {randomGameGenreOptions.map(tag => (
                    <option key={tag} value={tag}>{tag}</option>
                  ))}
                </select>
              </label>

              <label className="import-row" style={{display:'grid', gap:6}}>
                <span className="field-label">Completion</span>
                <select
                  value={randomBeaten}
                  onChange={e => setRandomBeaten(e.target.value as RandomBeatenFilter)}
                >
                  <option value="">Any status</option>
                  <option value="yes">Beaten</option>
                  <option value="no">Not beaten</option>
                </select>
              </label>

              <label className="import-row" style={{display:'grid', gap:6}}>
                <span className="field-label">Estimated Length</span>
                <select
                  value={randomTtb}
                  onChange={e => setRandomTtb(e.target.value as RandomTtbFilter)}
                >
                  <option value="">Any length</option>
                  <option value="under-2">Under 2 hours</option>
                  <option value="2-5">2 to 5 hours</option>
                  <option value="5-10">5 to 10 hours</option>
                  <option value="10-20">10 to 20 hours</option>
                  <option value="20-40">20 to 40 hours</option>
                  <option value="40-plus">40+ hours</option>
                  <option value="unknown">Unknown</option>
                </select>
              </label>
            </div>

            <div className="tip" style={{ marginTop:16, lineHeight:1.5 }}>
              {randomGameCandidates.length > 0
                ? `Ready to pick from ${randomGameCandidates.length} matching game${randomGameCandidates.length === 1 ? '' : 's'}.`
                : 'No games match these filters right now. Clear one or more filters and try again.'}
            </div>

            <div className="import-actions" style={{marginTop:16}}>
              <button
                onClick={closeRandomGameFlow}
              >
                Cancel
              </button>
              <button
                className="run"
                onClick={pickRandomGame}
                disabled={randomPicking || randomGameCandidates.length === 0}
              >
                {randomPicking ? 'Getting Game...' : 'Get Game'}
              </button>
            </div>
          </div>
        </div>
      )}

      {screenView === 'queue' && (
        <>
          <section className="control-panel queue-panel">
            <div className="panel-heading">
              <div>
                <span className="panel-kicker">Game Queue</span>
                <h2>Play Next</h2>
              </div>
              <div className="panel-actions">
                <div className="status-pill">{queueStats.totalGames.toLocaleString()} queued</div>
                <div className="status-pill">{queueStats.playingCount.toLocaleString()} playing</div>
                <div className="status-pill">
                  Time to clear {formatQueueClearEstimate(queueStats.totalTtbSeconds)}
                </div>
                {queueStats.ttbUnknownCount > 0 && (
                  <div className="status-pill">
                    {queueStats.ttbUnknownCount.toLocaleString()} without TTB
                  </div>
                )}
              </div>
            </div>
            <div className="queue-finished-tracker">
              <button
                type="button"
                className="queue-finished-card"
                onClick={() => setQueueFinishedTrackerDialog({ key: 'last7', label: 'Finished Last 7 Days' })}
              >
                <span className="queue-finished-label">Finished 7 Days</span>
                <strong>{queueStats.finishedLast7Days.toLocaleString()}</strong>
                <span>{queueStats.finishedTrackedCount.toLocaleString()} tracked total</span>
              </button>
              <button
                type="button"
                className="queue-finished-card"
                onClick={() => setQueueFinishedTrackerDialog({ key: 'last30', label: 'Finished Last 30 Days' })}
              >
                <span className="queue-finished-label">Finished 30 Days</span>
                <strong>{queueStats.finishedLast30Days.toLocaleString()}</strong>
                <span>Click to view the games</span>
              </button>
              <button
                type="button"
                className="queue-finished-card"
                onClick={() => setQueueFinishedTrackerDialog({ key: 'last365', label: 'Finished Last 365 Days' })}
              >
                <span className="queue-finished-label">Finished Last Year</span>
                <strong>{queueStats.finishedLast365Days.toLocaleString()}</strong>
                <span>Click to view the games</span>
              </button>
            </div>
            <div className="queue-finished-note">
              Finish counts use each game&apos;s saved queue finish date, so completed titles still count after they leave the active queue.
            </div>
          </section>

          <table className="grid queue-grid">
            <thead>
              <tr>
                <th>Queue</th>
                <th>Cover</th>
                <th>Title</th>
                <th>Platform</th>
                <th>Time to Beat</th>
                <th>Playing</th>
                <th>Started Playing</th>
                <th>Finished</th>
                <th>Time Played</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {queueGames.length === 0 ? (
                <tr>
                  <td colSpan={10}>
                    <div className="queue-empty">
                      Your queue is empty. Add games from the library table or from any game card.
                    </div>
                  </td>
                </tr>
              ) : queueGames.map((r, index) => {
                const cover = r.cover_url ? (
                  <img
                    src={r.cover_url}
                    alt=""
                    onClick={() => openCard(r.id)}
                    style={{
                      width:84,
                      height:'auto',
                      maxHeight:84,
                      borderRadius:8,
                      objectFit:'contain',
                      background:'var(--card-bg-soft)',
                      cursor:'pointer',
                    }}
                  />
                ) : (
                  <div
                    onClick={() => openCard(r.id)}
                    style={{
                      width:84,
                      height:84,
                      borderRadius:8,
                      background:'var(--card-bg-soft)',
                      border:'1px dashed var(--card-border-soft)',
                      cursor:'pointer',
                    }}
                  />
                )
                const draftPosition = queueOrderDrafts[r.id] ?? String(index + 1)

                return (
                  <tr
                    key={r.id}
                    data-queue-id={r.id}
                    className={queueDropId === r.id && dragQueueId !== r.id ? 'queue-drop-target' : ''}
                    style={dragQueueId === r.id ? { opacity: 0.55 } : undefined}
                  >
                    <td>
                      <div className="queue-order-cell">
                        <button
                          type="button"
                          className="queue-handle"
                          onPointerDown={event => {
                            if (event.button !== 0) return
                            event.preventDefault()
                            setDragQueueId(r.id)
                            setQueueDropId(r.id)
                            queueDropIdRef.current = r.id
                            event.currentTarget.setPointerCapture(event.pointerId)
                          }}
                          onPointerMove={event => {
                            if (dragQueueId !== r.id) return
                            updateQueueDragTarget(event.clientX, event.clientY)
                          }}
                          onPointerUp={async event => {
                            if (event.currentTarget.hasPointerCapture(event.pointerId)) {
                              event.currentTarget.releasePointerCapture(event.pointerId)
                            }
                            if (dragQueueId !== r.id) return
                            await finishQueueDrag(r.id)
                          }}
                          onPointerCancel={event => {
                            if (event.currentTarget.hasPointerCapture(event.pointerId)) {
                              event.currentTarget.releasePointerCapture(event.pointerId)
                            }
                            setDragQueueId(null)
                            setQueueDropId(null)
                            queueDropIdRef.current = null
                          }}
                          title="Drag to reorder"
                        >
                          ::
                        </button>
                        <div className="queue-step-buttons" aria-label={`Move ${r.title} in queue`}>
                          <button
                            type="button"
                            onClick={() => moveQueueItemByDelta(r.id, -1)}
                            disabled={index === 0}
                            aria-label={`Move ${r.title} up in queue`}
                            title="Move up"
                          >
                            ↑
                          </button>
                          <button
                            type="button"
                            onClick={() => moveQueueItemByDelta(r.id, 1)}
                            disabled={index === queueGames.length - 1}
                            aria-label={`Move ${r.title} down in queue`}
                            title="Move down"
                          >
                            ↓
                          </button>
                        </div>
                        <input
                          type="number"
                          min={1}
                          max={queueGames.length}
                          value={draftPosition}
                          onChange={e => setQueueOrderDrafts(prev => ({ ...prev, [r.id]: e.target.value }))}
                          onBlur={async () => {
                            const nextPosition = Number.parseInt(queueOrderDrafts[r.id] ?? String(index + 1), 10)
                            if (Number.isFinite(nextPosition)) {
                              await moveQueueItemToPosition(r.id, nextPosition)
                            } else {
                              setQueueOrderDrafts(prev => {
                                const next = { ...prev }
                                delete next[r.id]
                                return next
                              })
                            }
                          }}
                          onKeyDown={async event => {
                            if (event.key !== 'Enter') return
                            event.preventDefault()
                            const nextPosition = Number.parseInt(queueOrderDrafts[r.id] ?? String(index + 1), 10)
                            if (Number.isFinite(nextPosition)) {
                              await moveQueueItemToPosition(r.id, nextPosition)
                            }
                          }}
                        />
                      </div>
                    </td>
                    <td>{cover}</td>
                    <td>
                      <div style={{fontWeight:600}}>{r.title}</div>
                      {r.hidden_at && <div style={{fontSize:12, opacity:0.68, marginTop:4}}>Hidden in library</div>}
                    </td>
                    <td>{r.platform}</td>
                    <td>{formatEstimateHours(preferredMainTtbSeconds(r))}</td>
                    <td>
                      <label className="check">
                        <input
                          type="checkbox"
                          checked={!!r.queue_is_playing}
                          onChange={e => toggleQueuePlaying(r, e.target.checked)}
                        />
                        <span>{r.queue_is_playing ? 'Playing' : 'Queued'}</span>
                      </label>
                    </td>
                    <td>
                      <input
                        type="date"
                        value={localDateInputValue(r.queue_started_at)}
                        onChange={e => updateQueueDate(r.id, 'queue_started_at', e.target.value)}
                      />
                    </td>
                    <td>
                      <input
                        type="date"
                        value={localDateInputValue(r.queue_finished_at)}
                        onChange={e => updateQueueDate(r.id, 'queue_finished_at', e.target.value)}
                      />
                    </td>
                    <td>
                      <div className="queue-time-cell">
                        <strong>{hoursOrZero(r.playtime_minutes)}</strong>
                        <button type="button" onClick={() => openQueueTimeEditor(r)}>Add Time</button>
                      </div>
                    </td>
                    <td>
                      <div className="row-action-group">
                        <button type="button" onClick={() => openCard(r.id)}>Open Card</button>
                        <button type="button" onClick={() => removeFromQueue(r)}>Remove</button>
                      </div>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </>
      )}

      {screenView === 'library' && (
        <>
      <section className="control-panel quick-add-panel">
        <div className="panel-heading">
          <div>
            <span className="panel-kicker">Quick Add</span>
            <h2>Manual Entry</h2>
          </div>
          <div className="panel-note">
            {genreOptions.length} genres and {platformOptions.length} platforms ready for manual tagging
          </div>
        </div>
        <div className="controls">
        <input className="title-field" placeholder="Add game title" value={title} onChange={e => setTitle(e.target.value)} />
        <select value={platform} onChange={e => setPlatform(e.target.value)}>
          {platformOptions.map(p =>
            <option key={p} value={p}>{p}</option>
          )}
        </select>
        <select
          value={genreValue}
          onChange={e => setGenreValue(e.target.value)}
          title={!genres ? 'Loading genres…' : 'Select a genre'}
        >
          <option value="">Genre (optional)</option>
          {genreOptions.map(g => <option key={g.id} value={g.name}>{g.name}</option>)}
        </select>
        <label className="check">
          <input type="checkbox" checked={beaten} onChange={e => setBeaten(e.target.checked)} />
          <span>Beaten</span>
        </label>

        {/* NEW: License picker for manual add */}
        <select
          value={addLicenseType}
          onChange={e => setAddLicenseType(e.target.value as any)}
          title="License type"
        >
          <option value="owned">Owned</option>
          <option value="subscription">Subscription</option>
          <option value="free">Free</option>
          <option value="trial">Trial</option>
        </select>
        {addLicenseType === 'subscription' && (
          <input
            placeholder="License source (e.g., PC Game Pass)"
            value={addLicenseSource}
            onChange={e => setAddLicenseSource(e.target.value)}
            style={{ width: 220 }}
            title="Optional source label shown in the License column"
          />
        )}

        <button className="primary-action" onClick={add} title={HINTS.addManual}>Add Manual</button>
      </div>
      </section>

      <section className="control-panel filter-panel">
        <div className="panel-heading">
          <div>
            <span className="panel-kicker">Library Filters</span>
            <h2>Browse Library</h2>
          </div>
          <div className="panel-actions">
            <div className="status-pill">{filtered.length.toLocaleString()} shown</div>
            <div className="status-pill">Enriched {enrichedCount}/{visibleGamesForCounters.length}</div>
            <div className="status-pill">{queueStats.totalGames.toLocaleString()} queued</div>
            {selectedIds.size > 0 && <div className="status-pill">{selectedIds.size} selected</div>}
            {hasActiveFilters && (
              <button className="ghost-action" onClick={resetFilters}>Reset filters</button>
            )}
            <button
              className="ghost-action"
              onClick={selectAllShown}
              disabled={pageIds.length === 0 || allVisibleSelected}
              title={HINTS.selectShown}
            >
              Select All Shown ({pageIds.length})
            </button>
            <button
              className="ghost-action"
              onClick={selectAllMatching}
              disabled={matchingIds.length === 0 || allMatchingSelected}
              title={HINTS.selectMatching}
            >
              Select All Matching ({matchingIds.length})
            </button>
            {selectedIds.size > 0 && (
              <button className="ghost-action" onClick={clearSelection}>Clear selection</button>
            )}
          </div>
        </div>
      <div className="filters" style={{display:'flex', gap:8, alignItems:'center', flexWrap:'wrap'}}>
        <input className="search-field" placeholder="Search title..." value={q} onChange={e => setQ(e.target.value)} />
        <select value={fPlatform} onChange={e => setFPlatform(e.target.value)}>
          <option value="">All Platforms</option>
          {libraryPlatformOptions.map(p => <option key={p} value={p}>{p}</option>)}
        </select>
        <select value={fBeaten} onChange={e => setFBeaten(e.target.value)}>
          <option value="">All</option>
          <option value="yes">Beaten</option>
          <option value="no">Not beaten</option>
        </select>
        <select value={fGenre} onChange={e => setFGenre(e.target.value)}>
          <option value="">All Genres</option>
          {allGenreTags.map(tag => <option key={tag} value={tag}>{tag}</option>)}
        </select>
        <input type="number" min="0" step="0.5" placeholder="Min hours" value={fMinHours} onChange={e => setFMinHours(e.target.value)} />
        {/* NEW: License filter */}
        <select value={fLicense} onChange={e => setFLicense(e.target.value)}>
          <option value="">All Licenses</option>
          <option value="owned">Owned</option>
          <option value="subscription">Subscription</option>
          <option value="free">Free</option>
          <option value="trial">Trial</option>
        </select>
        <select value={fEnriched} onChange={e => setFEnriched(e.target.value)}>
          <option value="">All Enrichment</option>
          <option value="yes">Enriched</option>
          <option value="no">Unenriched</option>
        </select>
        <select value={fReviewed} onChange={e => setFReviewed(e.target.value)}>
          <option value="">All Reviews</option>
          <option value="yes">Reviewed</option>
          <option value="no">Not reviewed</option>
        </select>
        <label className="check" title="Include hidden rows in the table">
          <input
            type="checkbox"
            checked={showHidden}
            onChange={e => setShowHidden(e.target.checked)}
          />
          <span>Show hidden</span>
        </label>
        <div style={{ opacity: 0.75 }}>
          Enriched: {enrichedCount}/{visibleGamesForCounters.length}
        </div>
        {selectedIds.size > 0 && q.trim() && (
          <div style={{ opacity: 0.75 }}>
            Keeping {selectedIds.size} selected game(s) visible while searching
          </div>
        )}

        <div className="filter-utility" style={{display:'none'}}>
          <label style={{opacity:0.8}}>Sort:</label>
          <select value={sortBy} onChange={e => setSortBy(e.target.value as SortKey)}>
            <option value="title-asc">Title (A→Z)</option>
            <option value="title-desc">Title (Z→A)</option>
            <option value="release-asc">Release (oldest→newest)</option>
            <option value="release-desc">Release (newest→oldest)</option>
            <option value="playtime-asc">Time Played (low→high)</option>
            <option value="playtime-desc">Time Played (high→low)</option>
            <option value="last-asc">Last Played (oldest→newest)</option>
            <option value="last-desc">Last Played (newest→oldest)</option>
            <option value="review-asc">My Review (low→high)</option>
            <option value="review-desc">My Review (high→low)</option>
          </select>
        </div>

        {/* Bulk actions */}
        <div className="filter-bulk" style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <label style={{ opacity: 0.8 }} title={HINTS.bulkActions}>Bulk actions:</label>
          <select
            value={bulkAction}
            onChange={e => setBulkAction(e.target.value as BulkAction)}
            disabled={selectedIds.size === 0}
            title={HINTS.bulkActions}
          >
            <option value="">Choose…</option>
            <option value="beaten">Set Selected as Beaten</option>
            <option value="unbeaten">Set Selected as Not Beaten</option>
            <option value="hide">Hide Selected</option>
            <option value="delete">Delete Selected</option>
            <option value="enrich">Enrich Selected</option>
          </select>
          <button
            className="secondary-action"
            onClick={runBulk}
            disabled={selectedIds.size === 0 || !bulkAction}
            title={HINTS.bulkRun}
          >
            Run ({selectedIds.size})
          </button>
        </div>
      </div>
      <div className="alpha-filter" aria-label="Filter library by the first significant title letter">
        <span className="alpha-filter-label">Title Index</span>
        <div className="alpha-filter-bar">
          <button
            type="button"
            className={`alpha-filter-button${fTitleIndex === '' ? ' is-active' : ''}`}
            aria-pressed={fTitleIndex === ''}
            onClick={() => setFTitleIndex('')}
          >
            All
          </button>
          {TITLE_INDEX_OPTIONS.map(letter => {
            const count = titleIndexCounts.get(letter) ?? 0
            const pressed = fTitleIndex === letter
            const title =
              letter === '#'
                ? 'Titles starting with a number or symbol'
                : `Titles whose first significant word starts with ${letter}`
            return (
              <button
                key={letter}
                type="button"
                className={`alpha-filter-button${pressed ? ' is-active' : ''}`}
                aria-pressed={pressed}
                disabled={count === 0}
                onClick={() => setFTitleIndex(letter)}
                title={title}
              >
                {letter}
              </button>
            )
          })}
        </div>
      </div>
      </section>

      <table className="grid">
        <thead>
          <tr>
            <th title={HINTS.selectAll}>
              <input type="checkbox" checked={allVisibleSelected} onChange={toggleSelectAll} />
            </th>
            <th>Cover</th>
            <th style={{cursor:'pointer'}} onClick={()=>toggleSort('title')}>Title {sortIndicator('title')}</th>
            <th>Platform</th>
            <th>License</th>
            <th>Beaten</th>
            <th style={{cursor:'pointer'}} onClick={()=>toggleSort('review')}>My Review {sortIndicator('review')}</th>
            <th>Genre</th>
            <th style={{cursor:'pointer'}} onClick={()=>toggleSort('release')}>Release {sortIndicator('release')}</th>
            <th style={{cursor:'pointer'}} onClick={()=>toggleSort('ttb')}>TTB {sortIndicator('ttb')}</th>
            <th style={{cursor:'pointer'}} onClick={()=>toggleSort('playtime')}>Time Played {sortIndicator('playtime')}</th>
            <th style={{cursor:'pointer'}} onClick={()=>toggleSort('last')}>Last Played {sortIndicator('last')}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {paged.map(r => {
            const isEdit = editingId === r.id
            const cover = r.cover_url ? (
              <img
                src={r.cover_url}
                alt=""
                onClick={() => openCard(r.id)}
                style={{
                  width:100,
                  height:'auto',
                  maxHeight:100,
                  borderRadius:8,
                  objectFit:'contain',
                  background:'var(--card-bg-soft)',
                  cursor:'pointer',
                }}
              />
            ) : (
              <div
                onClick={() => openCard(r.id)}
                style={{
                  width:100,
                  height:100,
                  borderRadius:8,
                  background:'var(--card-bg-soft)',
                  border:'1px dashed var(--card-border-soft)',
                  cursor:'pointer',
                }}
              />
            )

            if (isEdit && edit) {
              return (
                <tr key={r.id} style={r.hidden_at ? { opacity: 0.65 } : undefined}>
                  <td><input type="checkbox" checked={selectedIds.has(r.id)} onChange={() => toggleRow(r.id)} title={HINTS.rowCheckbox} /></td>
                  <td>{cover}</td>
                  <td><input value={edit.title} onChange={e=>setEdit({...edit, title:e.target.value})}/></td>
                  <td>
                    <select value={edit.platform} onChange={e=>setEdit({...edit, platform:e.target.value})}>
                      {mergeDistinctOptions(platformOptions, [edit.platform]).map(p =>
                        <option key={p} value={p}>{p}</option>
                      )}
                    </select>
                  </td>
                  {/* NEW: License editor */}
                  <td>
                    <div style={{display:'flex', alignItems:'center', gap:6}}>
                      <select
                        value={edit.licenseType}
                        onChange={e=>setEdit({...edit, licenseType: e.target.value as any})}
                      >
                        <option value="owned">Owned</option>
                        <option value="subscription">Subscription</option>
                        <option value="free">Free</option>
                        <option value="trial">Trial</option>
                      </select>
                      {edit.licenseType === 'subscription' && (
                        <input
                          placeholder="Source (e.g., PC Game Pass)"
                          value={edit.licenseSource}
                          onChange={e=>setEdit({...edit, licenseSource: e.target.value})}
                          style={{width:180}}
                        />
                      )}
                    </div>
                  </td>
                  <td><input type="checkbox" checked={edit.beaten} onChange={e=>setEdit({...edit, beaten:e.target.checked})}/></td>
                  <td style={{minWidth:110}}>{reviewCellLabel(r) || '—'}</td>
                  <td>
                    <input list="genre-list" value={edit.genre} onChange={e=>setEdit({...edit, genre:e.target.value})}/>
                    <datalist id="genre-list">
                      {allGenreTags.map(name => <option key={name} value={name} />)}
                    </datalist>
                  </td>
                  <td><input style={{width:80}} value={edit.releaseYear} onChange={e=>setEdit({...edit, releaseYear:e.target.value})} placeholder="YYYY"/></td>
                  <td>{formatEstimateHours(preferredMainTtbSeconds(r))}</td>
                  <td><input style={{width:80}} value={edit.playtimeHours} onChange={e=>setEdit({...edit, playtimeHours:e.target.value})}/></td>
                  <td><input type="date" value={edit.lastPlayed} onChange={e=>setEdit({...edit, lastPlayed:e.target.value})}/></td>
                  <td>
                    <div className="row-action-group" style={{alignItems:'center'}}>
                      <input style={{width:220}} placeholder="Cover URL" value={edit.coverUrl} onChange={e=>setEdit({...edit, coverUrl:e.target.value})}/>
                      <button onClick={saveEdit} title={HINTS.saveRow}>Save</button>
                      <button onClick={cancelEdit} title={HINTS.cancelEdit}>Cancel</button>
                    </div>
                  </td>
                </tr>
              )
            }
            return (
              <tr key={r.id} style={r.hidden_at ? { opacity: 0.65 } : undefined}>
                <td><input type="checkbox" checked={selectedIds.has(r.id)} onChange={() => toggleRow(r.id)} title={HINTS.rowCheckbox} /></td>
                <td>{cover}</td>
                <td>
                  {r.title}
                  {r.hidden_at && (
                    <span style={{ marginLeft: 8, fontSize: 12, opacity: 0.8 }}>(hidden)</span>
                  )}
                </td>
                <td>{r.platform}</td>
                {/* NEW: License display */}
                <td>{licenseLabelOf(r)}</td>
                <td>{r.beaten ? 'Yes' : 'No'}</td>
                <td>{reviewCellLabel(r) || '—'}</td>
                <td>{r.genre ?? ''}</td>
                <td>{r.release_year ?? ''}</td>
                <td>{formatEstimateHours(preferredMainTtbSeconds(r))}</td>
                <td>{hours(r.playtime_minutes)}</td>
                <td>{formatDateLabel(r.last_played_at)}</td>
                <td>
                  <div className="row-action-group">
                    {r.queue_position != null ? (
                      <button type="button" onClick={() => setScreenView('queue')}>
                        Queued #{r.queue_position}
                      </button>
                    ) : (
                      <button type="button" onClick={() => addToQueue(r)}>
                        Add to Queue
                      </button>
                    )}
                    <button onClick={()=>openEnrichChooser(r)} title={HINTS.enrichRow}>Enrich</button>
                    <button onClick={()=>startEdit(r)} title={HINTS.editRow}>Edit</button>
                    <button
                      onClick={async () => {
                        try {
                          if (r.hidden_at) {
                            await unhideGame(r.id)
                          } else {
                            if (!confirm(`Hide "${r.title}" from your library view? You can show/unhide it later.`)) return
                            await hideGame(r.id)
                          }
                          setSelectedIds(prev => {
                            const next = new Set(prev)
                            next.delete(r.id)
                            return next
                          })
                          await refresh()
                        } catch (e: any) {
                          alert(`${r.hidden_at ? 'Unhide' : 'Hide'} failed: ` + (e?.message ?? String(e)))
                        }
                      }}
                      title={r.hidden_at ? 'Unhide row' : 'Hide row'}
                    >
                      {r.hidden_at ? 'Unhide' : 'Hide'}
                    </button>
                    <button
                      onClick={async ()=>{
                        if (confirm(`Delete "${r.title}"? This cannot be undone.`)) {
                          try { await deleteGame(r.id); await refresh() } catch(e:any) { alert('Delete failed: ' + (e?.message ?? String(e))) }
                        }
                      }}
                      title={HINTS.deleteRow}
                    >Delete</button>
                  </div>
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>

      {/* Pagination controls */}
      <div className="pagination-bar">
        <label className="field field-inline field-inline-small">
          <span className="field-label">Rows</span>
        <select value={pageSize} onChange={e => setPageSize(Number(e.target.value))}>
          <option value={25}>25</option>
          <option value={50}>50</option>
          <option value={100}>100</option>
        </select>
        </label>

        <div className="pagination-summary">
          {filtered.length ? `${startIdx + 1}–${endIdx} of ${filtered.length}` : '0 of 0'}
        </div>

        <div className="pagination-controls">
        <button
          onClick={() => setPage(1)}
          disabled={page === 1}
          title="First page"
          className="pager pager-first"
        >⏮</button>
        <button
          onClick={() => setPage(p => Math.max(1, p - 1))}
          disabled={page === 1}
          title="Previous page"
          className="pager pager-prev"
        >←</button>

        <div className="pagination-label">Page {page} / {pageCount}</div>

        <button
          onClick={() => setPage(p => Math.min(pageCount, p + 1))}
          disabled={page === pageCount}
          title="Next page"
          className="pager pager-next"
        >→</button>
        <button
          onClick={() => setPage(pageCount)}
          disabled={page === pageCount}
          title="Last page"
          className="pager pager-last"
        >⏭</button>
        </div>
      </div>
        </>
      )}

      {queueTimeDialog && (
        <div className="modal">
          <div className="modal-card" style={{ width: 520, maxWidth: '95vw' }}>
            <div className="panel-heading" style={{ marginBottom: 12 }}>
              <div>
                <span className="panel-kicker">Queue Time</span>
                <h2>Add Time Played</h2>
              </div>
              <button type="button" onClick={() => setQueueTimeDialog(null)}>Close</button>
            </div>
            <div className="panel-note" style={{ marginBottom: 16 }}>
              Add playtime for <strong>{queueTimeDialog.title}</strong>. This updates the queue, library table, and stats together.
            </div>
            <div className="queue-time-form">
              <label className="import-row">
                <span className="field-label">Hours</span>
                <input
                  type="number"
                  min={0}
                  step={1}
                  value={queueTimeDialog.hours}
                  onChange={e => setQueueTimeDialog(prev => prev ? { ...prev, hours: e.target.value } : prev)}
                  placeholder="0"
                />
              </label>
              <label className="import-row">
                <span className="field-label">Minutes</span>
                <input
                  type="number"
                  min={0}
                  step={1}
                  value={queueTimeDialog.minutes}
                  onChange={e => setQueueTimeDialog(prev => prev ? { ...prev, minutes: e.target.value } : prev)}
                  placeholder="0"
                />
              </label>
            </div>
            <div className="tip" style={{ marginTop: 16 }}>
              Current total: {hoursOrZero(queueTimeDialog.currentMinutes)}.
              {' '}Adding {hoursOrZero(parseAddedQueueMinutes(queueTimeDialog.hours, queueTimeDialog.minutes))}.
              {' '}New total: {hoursOrZero(queueTimeDialog.currentMinutes + parseAddedQueueMinutes(queueTimeDialog.hours, queueTimeDialog.minutes))}.
            </div>
            <div className="import-actions" style={{ marginTop: 16 }}>
              <button type="button" onClick={() => setQueueTimeDialog(null)}>Cancel</button>
              <button type="button" className="run" onClick={saveQueueTime} disabled={queueTimeSaving}>
                {queueTimeSaving ? 'Saving...' : 'Add Time'}
              </button>
            </div>
          </div>
        </div>
      )}

      {queueFinishedTrackerDialog && (
        <div className="modal">
          <div className="modal-card" style={{ width: 760, maxWidth: '95vw' }}>
            <div className="panel-heading" style={{ marginBottom: 12 }}>
              <div>
                <span className="panel-kicker">Queue Tracker</span>
                <h2>{queueFinishedTrackerDialog.label}</h2>
              </div>
              <button type="button" onClick={() => setQueueFinishedTrackerDialog(null)}>Close</button>
            </div>
            <div className="panel-note" style={{ marginBottom: 16 }}>
              {queueFinishedTrackerGames.length.toLocaleString()} game{queueFinishedTrackerGames.length === 1 ? '' : 's'} in this rolling window.
            </div>
            {queueFinishedTrackerGames.length === 0 ? (
              <div className="queue-empty">
                No games have been logged in this finished-games window yet.
              </div>
            ) : (
              <div style={{ display: 'grid', gap: 10 }}>
                {queueFinishedTrackerGames.map(game => (
                  <div
                    key={game.id}
                    style={{
                      display: 'grid',
                      gridTemplateColumns: 'minmax(0, 1fr) auto auto',
                      gap: 12,
                      alignItems: 'center',
                      padding: '12px 14px',
                      borderRadius: 14,
                      background: 'rgba(255,255,255,0.03)',
                      border: '1px solid rgba(255,255,255,0.06)',
                    }}
                  >
                    <div>
                      <div style={{ fontWeight: 600 }}>{game.title}</div>
                      <div style={{ opacity: 0.74, fontSize: 12 }}>
                        {game.platform}
                        {game.queuePosition != null ? ` · Queue #${game.queuePosition}` : ''}
                        {game.hiddenAt ? ' · Hidden in library' : ''}
                      </div>
                    </div>
                    <div style={{ opacity: 0.82, fontSize: 13 }}>
                      {formatDateLabel(game.finishedAt)}
                    </div>
                    <button
                      type="button"
                      onClick={async () => {
                        setQueueFinishedTrackerDialog(null)
                        await openCard(game.id)
                      }}
                    >
                      Open Card
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Details Card */}
      {card && (
        <div className="modal" style={{ zIndex: 3000 }}>
          <div className="modal-card game-card-modal">
            <div className="game-card-header">
              {card.cover_url && (
                <img src={card.cover_url} alt="" className="game-card-cover" />
              )}
              <div className="game-card-title-block">
                <div className="game-card-title">{card.title}</div>
                <div className="game-card-meta">
                  {card.platform}{card.release_year ? ` · ${card.release_year}` : ''}{card.genre ? ` · ${card.genre}` : ''}
                  {/* If FullGame has license fields in your typings, you can render them here too */}
                </div>
                <div className="game-card-inline-stats">
                  <span>Critic: {card.agg_rating != null ? Math.round(card.agg_rating) : '—'}</span>
                  <span>User: {card.user_rating != null ? Math.round(card.user_rating) : '—'}</span>
                  <span>My Review: {card.review_rating != null ? `${formatReviewStars(card.review_rating)} (${card.review_rating}/5)` : (hasReviewRecord(card) ? 'Text review' : 'Unreviewed')}</span>
                  <span>TTB: {fmtHoursFromSec(preferredTtb?.main)} / {fmtHoursFromSec(preferredTtb?.extra)} / {fmtHoursFromSec(preferredTtb?.complete)}{preferredTtb?.count ? ` (${preferredTtb.count})` : ''}</span>
                </div>
                <div style={{display:'flex', alignItems:'center', gap:8, marginTop:12, flexWrap:'wrap'}}>
                  {card.queue_position != null ? (
                    <>
                      <button
                        type="button"
                        onClick={() => {
                          setScreenView('queue')
                          setCard(null)
                        }}
                        style={{
                          background:'var(--button-solid)',
                          border:'1px solid var(--button-border)',
                          color:'var(--button-solid-text)',
                          borderRadius:8,
                          padding:'6px 12px',
                        }}
                      >
                        Queue #{card.queue_position}
                      </button>
                      <button
                        type="button"
                        onClick={async () => {
                          await removeFromQueue(card)
                        }}
                        style={{
                          background:'var(--card-bg-soft)',
                          border:'1px solid var(--card-border)',
                          color:'var(--text)',
                          borderRadius:8,
                          padding:'6px 12px',
                        }}
                      >
                        Remove from Queue
                      </button>
                    </>
                  ) : (
                    <button
                      type="button"
                      onClick={async () => {
                        await addToQueue(card)
                      }}
                      style={{
                        background:'var(--button-solid)',
                        border:'1px solid var(--button-border)',
                        color:'var(--button-solid-text)',
                        borderRadius:8,
                        padding:'6px 12px',
                      }}
                    >
                      Add to Queue
                    </button>
                  )}
                </div>
                <div style={{display:'flex', alignItems:'center', gap:8, marginTop:10, flexWrap:'wrap'}}>
                  <button
                    type="button"
                    onClick={() => openCardExternalPage('igdb')}
                    style={{
                      background:'var(--card-bg-soft)',
                      border:'1px solid var(--card-border)',
                      color:'var(--text)',
                      borderRadius:8,
                      padding:'6px 12px',
                    }}
                    title="Open IGDB in your default browser"
                  >
                    Open IGDB
                  </button>
                  <button
                    type="button"
                    onClick={() => openCardExternalPage('hltb')}
                    style={{
                      background:'var(--card-bg-soft)',
                      border:'1px solid var(--card-border)',
                      color:'var(--text)',
                      borderRadius:8,
                      padding:'6px 12px',
                    }}
                    title="Open HowLongToBeat in your default browser"
                  >
                    Open HowLongToBeat
                  </button>
                  <button
                    type="button"
                    onClick={openCardGameFaqsSearch}
                    style={{
                      background:'var(--card-bg-soft)',
                      border:'1px solid var(--card-border)',
                      color:'var(--text)',
                      borderRadius:8,
                      padding:'6px 12px',
                    }}
                    title="Search for this game on GameFAQs in your default browser"
                  >
                    Search on GameFaqs
                  </button>
                </div>
                <div style={{display:'flex', alignItems:'center', gap:8, marginTop:10, flexWrap:'wrap'}}>
                  <span className="game-card-id-label">IGDB ID</span>
                  <input
                    value={cardIgdbIdInput}
                    onChange={e => setCardIgdbIdInput(e.target.value)}
                    onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); applyCardIgdbId() } }}
                    placeholder="e.g. 5901"
                    inputMode="numeric"
                    style={{
                      width:120,
                      background:'var(--card-input-bg)',
                      color:'var(--text)',
                      border:'1px solid var(--card-border)',
                      borderRadius:8,
                      padding:'6px 8px',
                    }}
                  />
                  <button
                    onClick={applyCardIgdbId}
                    disabled={cardIgdbApplying || !cardIgdbIdInput.trim()}
                    style={{
                      background:'var(--button-solid)',
                      border:'1px solid var(--button-border)',
                      color:'var(--button-solid-text)',
                      borderRadius:8,
                      padding:'6px 10px',
                    }}
                  >
                    {cardIgdbApplying ? 'Applying...' : 'Apply IGDB ID'}
                  </button>
                  {card.igdb_id != null && (
                    <span className="game-card-id-current">Current: {card.igdb_id}</span>
                  )}
                </div>
                <div style={{display:'flex', alignItems:'center', gap:8, marginTop:8, flexWrap:'wrap'}}>
                  <span className="game-card-id-label">HLTB ID</span>
                  <input
                    value={cardHltbIdInput}
                    onChange={e => setCardHltbIdInput(e.target.value)}
                    onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); applyCardHltbId() } }}
                    placeholder="e.g. 5900"
                    inputMode="numeric"
                    style={{
                      width:120,
                      background:'var(--card-input-bg)',
                      color:'var(--text)',
                      border:'1px solid var(--card-border)',
                      borderRadius:8,
                      padding:'6px 8px',
                    }}
                  />
                  <button
                    onClick={applyCardHltbId}
                    disabled={cardHltbApplying || !cardHltbIdInput.trim()}
                    style={{
                      background:'var(--button-solid)',
                      border:'1px solid var(--button-border)',
                      color:'var(--button-solid-text)',
                      borderRadius:8,
                      padding:'6px 10px',
                    }}
                  >
                    {cardHltbApplying ? 'Applying...' : 'Apply HLTB ID'}
                  </button>
                  {card.hltb_id != null && (
                    <span className="game-card-id-current">Current: {card.hltb_id}</span>
                  )}
                </div>
                <div style={{
                  marginTop:10,
                  padding:'10px 12px',
                  borderRadius:10,
                  background:'var(--section-bg-soft)',
                  border:'1px solid var(--section-border)',
                  fontSize:13,
                  lineHeight:1.5,
                }}>
                  <div style={{fontWeight:600}}>Estimated Length</div>
                  <div className="game-card-note">
                    Preferred: {fmtHoursFromSec(preferredTtb?.main)} / {fmtHoursFromSec(preferredTtb?.extra)} / {fmtHoursFromSec(preferredTtb?.complete)}
                    {hasIgdbTtb ? ' (IGDB preferred, HLTB fallback)' : (hasHltbTtb ? ' (HLTB)' : ' (No source yet)')}
                  </div>
                  {hasIgdbTtb && (
                    <div className="game-card-note">
                      IGDB: {fmtHoursFromSec(card.ttb_main)} / {fmtHoursFromSec(card.ttb_extra)} / {fmtHoursFromSec(card.ttb_complete)}{card.ttb_count ? ` (${card.ttb_count})` : ''}
                    </div>
                  )}
                  {hasHltbTtb && (
                    <div className="game-card-note">
                      HLTB: {fmtHoursFromSec(card.hltb_main)} / {fmtHoursFromSec(card.hltb_extra)} / {fmtHoursFromSec(card.hltb_complete)}{card.hltb_all_count ? ` (${card.hltb_all_count})` : ''}
                      {card.hltb_match_method ? ` [${card.hltb_match_method}]` : ''}
                    </div>
                  )}
                  {cardTtbDisagreement && (
                    <div className="game-card-emphasis">
                      IGDB is treated as the preferred source when the two time estimates disagree.
                    </div>
                  )}
                </div>
              </div>
              <button onClick={closeCardView} title={HINTS.closeModal} style={{background:'var(--card-bg-soft)', border:'1px solid var(--card-border)', color:'var(--text)', borderRadius:8, padding:'6px 10px', cursor:'pointer'}}>Close</button>
            </div>

            <div style={{display:'flex', gap:8, marginTop:16, marginBottom:14, flexWrap:'wrap'}}>
              {([
                ['details', 'Details'],
                ['notes', 'Notes'],
                ['review', 'Review'],
              ] as const).map(([key, label]) => (
                <button
                  key={key}
                  onClick={() => setCardTab(key)}
                  style={{
                    padding:'8px 12px',
                    borderRadius:999,
                    border:'1px solid var(--card-border)',
                    background: cardTab === key ? 'var(--button-solid)' : 'var(--card-bg-soft)',
                    color: cardTab === key ? 'var(--button-solid-text)' : 'var(--text)',
                    cursor:'pointer',
                  }}
                >
                  {label}
                </button>
              ))}
            </div>

            {cardTab === 'details' && (
              <div style={{marginTop:4}}>
                {(card.summary || card.hltb_summary) ? (
                  <div style={{lineHeight:1.5, whiteSpace:'pre-wrap'}}>{card.summary || card.hltb_summary}</div>
                ) : (
                  <div style={{padding:'12px 14px', borderRadius:12, background:'var(--section-bg-soft)', border:'1px solid var(--section-border)', color:'var(--text-muted)'}}>
                    No enriched game summary yet. You can still use Notes and Review right away.
                  </div>
                )}
                {card.storyline && (
                  <details style={{marginTop:12}}>
                    <summary style={{cursor:'pointer'}}>Storyline</summary>
                    <div style={{marginTop:6, whiteSpace:'pre-wrap'}}>{card.storyline}</div>
                  </details>
                )}
                {showRandomGame && (
                  <div style={{display:'flex', justifyContent:'flex-end', gap:8, marginTop:14}}>
                    <button onClick={restartRandomGameFlow} style={{background:'var(--card-bg-soft)', border:'1px solid var(--card-border)', color:'var(--text)', borderRadius:8, padding:'6px 12px'}}>Start Over</button>
                    <button onClick={closeCardView} title={HINTS.closeModal} style={{background:'var(--card-bg-soft)', border:'1px solid var(--card-border)', color:'var(--text)', borderRadius:8, padding:'6px 12px'}}>Close</button>
                  </div>
                )}
              </div>
            )}

            {cardTab === 'notes' && (
              <div style={{marginTop:4}}>
                <div className="game-card-field-heading" style={{ marginBottom: 6 }}>Notes</div>
                <textarea
                  value={notesDraft}
                  onChange={e => {
                    setNotesDraft(e.target.value)
                  }}
                  rows={6}
                  style={{width:'100%', background:'var(--card-input-bg)', color:'var(--text)', border:'1px solid var(--card-border)', borderRadius:10, padding:10, outline:'none'}}
                />
                <div style={{display:'flex', justifyContent:'flex-end', marginTop:10, gap:8}}>
                  {showRandomGame && (
                    <button onClick={restartRandomGameFlow} style={{background:'var(--card-bg-soft)', border:'1px solid var(--card-border)', color:'var(--text)', borderRadius:8, padding:'6px 12px'}}>Start Over</button>
                  )}
                  <button onClick={closeCardView} title={HINTS.closeModal} style={{background:'var(--card-bg-soft)', border:'1px solid var(--card-border)', color:'var(--text)', borderRadius:8, padding:'6px 12px'}}>Close</button>
                  <button
                    onClick={saveNotes}
                    title={HINTS.saveNotes}
                    disabled={notesSaving || notesDraft === (card.notes ?? '')}
                    style={{
                      background: (notesSaving || notesDraft === (card.notes ?? '')) ? 'var(--button-disabled-bg)' : 'var(--button-solid)',
                      border:'1px solid var(--button-border)',
                      color:'var(--button-solid-text)',
                      borderRadius:8,
                      padding:'6px 12px',
                      cursor: (notesSaving || notesDraft === (card.notes ?? '')) ? 'not-allowed' : 'pointer',
                    }}
                  >
                    {notesSaving ? 'Saving...' : (notesDraft === (card.notes ?? '') ? 'Saved' : 'Save Notes')}
                  </button>
                </div>
              </div>
            )}

            {cardTab === 'review' && (
              <div style={{marginTop:4}}>
                <div className="game-card-field-heading" style={{ marginBottom: 8 }}>Your Star Rating</div>
                <div style={{display:'flex', gap:8, flexWrap:'wrap', marginBottom:10}}>
                  {[0, 1, 2, 3, 4, 5].map(value => (
                    <button
                      key={value}
                      onClick={() => setReviewRatingDraft(value)}
                      style={{
                        padding:'8px 12px',
                        borderRadius:10,
                        border:'1px solid var(--card-border)',
                        background: reviewRatingDraft === value ? 'var(--button-solid)' : 'var(--card-bg-soft)',
                        color: reviewRatingDraft === value ? 'var(--button-solid-text)' : 'var(--text)',
                        cursor:'pointer',
                        minWidth: value === 0 ? 88 : 108,
                      }}
                    >
                      {value === 0 ? '0 Stars' : `${formatReviewStars(value)} ${value}/5`}
                    </button>
                  ))}
                </div>
                <div className="game-card-note" style={{ marginBottom: 12 }}>
                  {reviewRatingDraft == null ? 'No star rating selected yet.' : `Selected: ${formatReviewStars(reviewRatingDraft)} (${reviewRatingDraft}/5)`}
                </div>

                <div className="game-card-field-heading" style={{ marginBottom: 6 }}>Your Written Review</div>
                <textarea
                  value={reviewDraft}
                  onChange={e => setReviewDraft(e.target.value)}
                  rows={7}
                  placeholder="What did you think about this game?"
                  style={{width:'100%', background:'var(--card-input-bg)', color:'var(--text)', border:'1px solid var(--card-border)', borderRadius:10, padding:10, outline:'none'}}
                />
                <div style={{display:'flex', justifyContent:'space-between', alignItems:'center', marginTop:10, gap:8, flexWrap:'wrap'}}>
                  <button
                    onClick={() => {
                      setReviewRatingDraft(null)
                      setReviewDraft('')
                    }}
                    style={{background:'var(--card-bg-soft)', border:'1px solid var(--card-border)', color:'var(--text)', borderRadius:8, padding:'6px 12px'}}
                  >
                    Clear Draft
                  </button>
                  <div style={{display:'flex', gap:8}}>
                    {showRandomGame && (
                      <button onClick={restartRandomGameFlow} style={{background:'var(--card-bg-soft)', border:'1px solid var(--card-border)', color:'var(--text)', borderRadius:8, padding:'6px 12px'}}>Start Over</button>
                    )}
                    <button onClick={closeCardView} title={HINTS.closeModal} style={{background:'var(--card-bg-soft)', border:'1px solid var(--card-border)', color:'var(--text)', borderRadius:8, padding:'6px 12px'}}>Close</button>
                    <button
                      onClick={saveReview}
                      title={HINTS.saveReview}
                      disabled={reviewSaving || reviewUnchanged}
                      style={{
                        background: (reviewSaving || reviewUnchanged) ? 'var(--button-disabled-bg)' : 'var(--button-solid)',
                        border:'1px solid var(--button-border)',
                        color:'var(--button-solid-text)',
                        borderRadius:8,
                        padding:'6px 12px',
                        cursor: (reviewSaving || reviewUnchanged) ? 'not-allowed' : 'pointer',
                      }}
                    >
                      {reviewSaving ? 'Saving...' : (reviewUnchanged ? 'Saved' : 'Save Review')}
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      )}

      {/* IGDB Multi-match chooser (carousel) */}
      {igdbOffer && (
        <div className="modal" style={{ zIndex: 3000 }}>
          <div className="modal-card chooser-modal">
            <h3 style={{marginTop:4, marginBottom:12}}>
              Pick the right IGDB match for “{igdbOffer.title}”
              <span style={{opacity:0.7, marginLeft:8, fontSize:13}}>{igdbOffer.idx + 1} / {igdbOffer.candidates.length}</span>
              {hydratingIdx === igdbOffer.idx && <span style={{ marginLeft: 10, fontSize: 12, opacity: 0.7 }}>Loading details…</span>}
            </h3>

            {(() => {
              const pv: Npv = igdbOffer.candidates[igdbOffer.idx]
              return (
                <div className="chooser-layout">
                  <div className="chooser-nav">
                    <button
                      onClick={() => setIgdbOffer(o => o ? { ...o, idx: Math.max(0, o.idx - 1) } : o)}
                      disabled={igdbOffer.idx === 0}
                      title={HINTS.carouselPrev}
                      style={{ padding: '6px 10px', borderRadius: 8, border: '1px solid var(--card-border)', background: 'var(--card-bg-soft)', color: 'var(--text)', cursor: igdbOffer.idx === 0 ? 'not-allowed' : 'pointer' }}
                    >←</button>
                    <div>
                      {pv?.cover_url
                        ? <img src={pv.cover_url} className="chooser-cover" />
                        : <div className="chooser-cover-placeholder" />
                      }
                    </div>
                    <button
                      onClick={() => setIgdbOffer(o => o ? { ...o, idx: Math.min(o.candidates.length - 1, o.idx + 1) } : o)}
                      disabled={igdbOffer.idx === igdbOffer.candidates.length - 1}
                      title={HINTS.carouselNext}
                      style={{ padding: '6px 10px', borderRadius: 8, border: '1px solid var(--card-border)', background: 'var(--card-bg-soft)', color: 'var(--text)', cursor: igdbOffer.idx === igdbOffer.candidates.length - 1 ? 'not-allowed' : 'pointer' }}
                    >→</button>
                  </div>

                  <div className="chooser-copy">
                    <div style={{ fontSize: 18, fontWeight: 700, marginBottom: 6 }}>{pv?.title || ''}</div>
                    <ChangeRow label="Genre" cur={igdbOffer.current?.genre ?? ''} nxt={(pv?.genres || []).join(', ')} />
                    <ChangeRow label="Release Year" cur={igdbOffer.current?.release_year ?? ''} nxt={pv?.release_year ?? ''} />
                    <ChangeRow label="Cover URL" cur={igdbOffer.current?.cover_url ?? ''} nxt={pv?.cover_url ?? ''} />
                    <ChangeRow label="Summary" cur="" nxt={pv?.summary ?? ''} long />
                    <ChangeRow label="Storyline" cur="" nxt={pv?.storyline ?? ''} long />
                    <ChangeRow label="Critic" cur="" nxt={pv?.agg_rating != null ? Math.round(pv.agg_rating) : ''} />
                    <ChangeRow label="User" cur="" nxt={pv?.user_rating != null ? Math.round(pv.user_rating) : ''} />
                    <ChangeRow
                      label="TTB (main/extra/100%)"
                      cur=""
                      nxt={[pv?.ttb_main, pv?.ttb_extra, pv?.ttb_complete]
                        .map((s: any) => s && s > 0 ? (s / 3600).toFixed(1) + 'h' : '—').join(' / ')}
                    />
                  </div>
                </div>
              )
            })()}

            {/* auto-hydrate when index changes */}
            <Hydrator offer={igdbOffer} idx={igdbOffer.idx} doHydrate={hydrateCandidateByIndex} />

            <div className="chooser-actions">
              <button onClick={()=>setIgdbOffer(null)} style={{background:'var(--card-bg-soft)', border:'1px solid var(--card-border)', color:'var(--text)', borderRadius:8, padding:'6px 12px'}}>Cancel</button>
              <button
                className="run"
                title={HINTS.applyCandidate}
                onClick={async ()=>{
                  try {
                    let pv: Npv | null = igdbOffer.candidates[igdbOffer.idx]
                    if (!pv || (pv.summary == null && pv.storyline == null && pv.agg_rating == null && pv.ttb_main == null)) {
                      pv = await hydrateCandidateByIndex(igdbOffer.idx)
                      if (!pv) throw new Error('Could not load candidate details')
                    }
                    await applyIgdbDetails({
                      id: igdbOffer.forId,
                      igdb_id: pv.igdb_id,
                      igdb_url: pv.igdb_url ?? undefined,
                      genres: pv.genres,
                      cover_url: pv.cover_url ?? undefined,
                      release_year: pv.release_year ?? undefined,
                      summary: pv.summary ?? undefined,
                      storyline: pv.storyline ?? undefined,
                      agg_rating: pv.agg_rating ?? undefined,
                      user_rating: pv.user_rating ?? undefined,
                      ttb_main: pv.ttb_main ?? undefined,
                      ttb_extra: pv.ttb_extra ?? undefined,
                      ttb_complete: pv.ttb_complete ?? undefined,
                      ttb_count: pv.ttb_count ?? undefined,
                    })
                    await applyBestHltbForGame(
                      igdbOffer.forId,
                      igdbOffer.title,
                      igdbOffer.current?.platform ?? null
                    )
                    setIgdbOffer(null)
                    await refresh()
                    await refreshCardIfOpen(igdbOffer.forId)
                  } catch (e:any) {
                    alert('Apply failed: ' + (e?.message ?? String(e)))
                  }
                }}
                style={{background:'var(--button-solid)', border:'1px solid var(--button-border)', color:'var(--button-solid-text)', borderRadius:8, padding:'6px 12px'}}
              >Apply</button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

/** Triggers hydration when the selected index changes (kept tiny & isolated). */
function Hydrator({ offer, idx, doHydrate }: { offer: OfferState, idx: number, doHydrate: (i: number) => Promise<Npv | null> }) {
  useEffect(() => {
    if (!offer) return
    const c = offer.candidates[idx]
    if (c && (
      c.summary == null &&
      c.storyline == null &&
      c.agg_rating == null &&
      c.user_rating == null &&
      c.ttb_main == null &&
      c.ttb_complete == null
    )) {
      doHydrate(idx).catch(()=>{})
    }
  }, [offer, idx, doHydrate])
  return null
}

function ChangeRow({ label, cur, nxt, long }: { label: string, cur: any, nxt: any, long?: boolean }) {
  const changed = (cur ?? '') !== (nxt ?? '')
  return (
    <div className="change-row">
      <div className="change-row-label">{label}</div>
      <div>
        {changed && cur ? <div className="change-row-prev">{String(cur)}</div> : null}
        <div className={changed ? 'change-row-next is-changed' : 'change-row-next'} style={{whiteSpace: long ? 'pre-wrap' : 'normal'}}>
          {String(nxt ?? '') || '—'}
        </div>
      </div>
    </div>
  )
}

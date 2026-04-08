Param(
  [string]$CargoPath = "src-tauri\Cargo.toml"
)

if (!(Test-Path $CargoPath)) {
  Write-Error "Could not find $CargoPath. Run this from your project root."
  exit 1
}

$content = Get-Content $CargoPath -Raw

if ($content -notmatch "\[dependencies\]") {
  $content += "`r`n[dependencies]`r`n"
}

function Ensure-Line {
  param([string]$text, [string]$pattern, [string]$line)
  if ($text -notmatch $pattern) {
    return $text + $line + "`r`n"
  }
  return $text
}

# Ensure urlencoding and chrono (with serde feature)
$content = Ensure-Line $content "^\s*urlencoding\s*=" 'urlencoding = "2"'
if ($content -match "^\s*chrono\s*=") {
  # Upgrade chrono line to include serde if missing
  $content = $content -replace '(^\s*chrono\s*=\s*".*")', 'chrono = { version = "0.4", features = ["serde"] }'
} else {
  $content = Ensure-Line $content "^\s*chrono\s*=" 'chrono = { version = "0.4", features = ["serde"] }'
}

Set-Content -Path $CargoPath -Value $content -Encoding UTF8
Write-Host "Updated $CargoPath with urlencoding and chrono deps."

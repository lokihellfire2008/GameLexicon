$path = "src-tauri\Cargo.toml"
if (-Not (Test-Path $path)) { Write-Error "Cargo.toml not found at $path"; exit 1 }
$c = Get-Content $path -Raw
if ($c -notmatch "\[dependencies\]") {
  $c = $c + "`r`n[dependencies]`r`n"
}
if ($c -notmatch "(?m)^\s*quick-xml\s*=") {
  $c = $c + "quick-xml = `"0.31`"`r`n"
}
Set-Content $path $c -Encoding UTF8
Write-Output "Updated src-tauri\Cargo.toml with quick-xml dep."

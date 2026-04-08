$path = "src-tauri\Cargo.toml"
if (-Not (Test-Path $path)) { Write-Error "Cargo.toml not found at $path"; exit 1 }
$c = Get-Content $path -Raw
if ($c -notmatch "\[dependencies\]") { $c += "`r`n[dependencies]`r`n" }
if ($c -notmatch "(?m)^\s*keyring\s*=") { $c += "keyring = `"2`"`r`n" }
Set-Content $path $c -Encoding UTF8
Write-Output "Ensured keyring dependency in src-tauri\Cargo.toml"

<#
.SYNOPSIS
  Start meshd on Windows in LAN mode with a fresh API token, open the firewall for the two ports
  (asks for elevation once), and open the admin panel in the default browser.
.PARAMETER ModelsDir
  Directory with GGUF files (default %LOCALAPPDATA%\MeshAI\models).
.PARAMETER Remove
  Remove the firewall rules and exit.
#>
param(
  [string]$ModelsDir = "$env:LOCALAPPDATA\MeshAI\models",
  [string]$StateDir  = "$env:LOCALAPPDATA\MeshAI\state",
  [switch]$Remove
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$exe  = Join-Path $root "desktop\target\release\meshd.exe"
if (-not (Test-Path $exe)) { $exe = Join-Path $root "desktop\target\debug\meshd.exe" }
if (-not (Test-Path $exe)) { throw "build first: cargo build --release --manifest-path desktop\Cargo.toml" }

$rules = @(@{Name="MeshAI control 7070"; Port=7070}, @{Name="MeshAI api 8080"; Port=8080})
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if ($Remove) {
  if (-not $isAdmin) { Start-Process powershell -Verb RunAs -ArgumentList "-File `"$PSCommandPath`" -Remove"; exit }
  foreach ($r in $rules) { Remove-NetFirewallRule -DisplayName $r.Name -ErrorAction SilentlyContinue }
  Write-Host "firewall rules removed"; exit
}
$missing = $rules | Where-Object { -not (Get-NetFirewallRule -DisplayName $_.Name -ErrorAction SilentlyContinue) }
if ($missing) {
  if (-not $isAdmin) {
    Write-Host "adding firewall rules (elevation prompt)…"
    $cmd = ($missing | ForEach-Object { "New-NetFirewallRule -DisplayName '$($_.Name)' -Direction Inbound -Protocol TCP -LocalPort $($_.Port) -Action Allow -Profile Private" }) -join "; "
    Start-Process powershell -Verb RunAs -Wait -ArgumentList "-Command", $cmd
  } else { foreach ($r in $missing) { New-NetFirewallRule -DisplayName $r.Name -Direction Inbound -Protocol TCP -LocalPort $r.Port -Action Allow -Profile Private | Out-Null } }
}

New-Item -ItemType Directory -Force -Path $ModelsDir, $StateDir | Out-Null
$token = -join ((1..24) | ForEach-Object { "abcdefghijklmnopqrstuvwxyz0123456789"[(Get-Random -Maximum 36)] })
$env:MESHAI_API_TOKEN = $token
$env:RUST_LOG = "meshd=info"
Write-Host "admin: http://localhost:8080/admin/?token=$token"
Start-Process "http://localhost:8080/admin/?token=$token"
& $exe --models $ModelsDir --state $StateDir serve --lan

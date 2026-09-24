<#
.SYNOPSIS
  Fetch llama.cpp Windows binaries (built with GGML_RPC=ON) into third_party\llama.cpp\build-host\bin,
  and optionally the demo models into the MeshAI models directory.
.PARAMETER Tag
  llama.cpp release tag to download (default: the tag matching the commit pinned in docs/RESEARCH.md).
.PARAMETER Models
  Also download the small test model (Qwen3-0.6B Q8_0) into %LOCALAPPDATA%\MeshAI\models.
#>
param(
  [string]$Tag = "b6537",
  [switch]$Models,
  [string]$ModelsDir = "$env:LOCALAPPDATA\MeshAI\models"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$bin  = Join-Path $root "third_party\llama.cpp\build-host\bin"
New-Item -ItemType Directory -Force -Path $bin | Out-Null

# The official CPU build ships llama-server.exe; the RPC server is included in the same zip when the
# release was built with GGML_RPC=ON (all b6xxx releases are). Fall back to a CMake build otherwise.
$zip = "llama-$Tag-bin-win-cpu-x64.zip"
$url = "https://github.com/ggml-org/llama.cpp/releases/download/$Tag/$zip"
$tmp = Join-Path $env:TEMP $zip
Write-Host "downloading $url"
Invoke-WebRequest -Uri $url -OutFile $tmp
Expand-Archive -Path $tmp -DestinationPath $bin -Force
$need = @("llama-server.exe", "ggml-rpc-server.exe")
foreach ($n in $need) {
  $f = Get-ChildItem -Path $bin -Recurse -Filter $n | Select-Object -First 1
  if (-not $f) {
    Write-Warning "$n is not in $zip — build llama.cpp with: cmake -S third_party\llama.cpp -B third_party\llama.cpp\build-host -DGGML_RPC=ON && cmake --build third_party\llama.cpp\build-host --config Release --target llama-server ggml-rpc-server llama-bench"
  } elseif ($f.DirectoryName -ne $bin) { Move-Item -Force $f.FullName $bin }
}
Get-ChildItem -Path $bin -Filter *.dll -Recurse | Where-Object { $_.DirectoryName -ne $bin } | ForEach-Object { Move-Item -Force $_.FullName $bin }
Write-Host "binaries in $bin"

if ($Models) {
  New-Item -ItemType Directory -Force -Path $ModelsDir | Out-Null
  $m = "Qwen3-0.6B-Q8_0.gguf"
  $murl = "https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/$m"
  if (-not (Test-Path (Join-Path $ModelsDir $m))) { Write-Host "downloading $m"; Invoke-WebRequest -Uri $murl -OutFile (Join-Path $ModelsDir $m) }
  Write-Host "models in $ModelsDir — start meshd with --models `"$ModelsDir`""
}

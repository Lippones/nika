<#
.SYNOPSIS
  Compila o shim do Discord (crate discord-shim) e o instala em
  src-tauri/resources/discord/version.dll.

.DESCRIPTION
  O shim é a version.dll própria do Nika (docs/discord-dll.md). É Windows-only
  (MSVC), fica fora do workspace do app e vira um recurso empacotado. Rode este
  script antes de `npm run app:build` se quiser o proxy do Discord no instalador.

.EXAMPLE
  pwsh scripts/build-shim.ps1
#>
[CmdletBinding()]
param(
  [string]$Target = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$shim = Join-Path $root "src-tauri/discord-shim"
$dest = Join-Path $root "src-tauri/resources/discord"

Push-Location $shim
try {
  cargo build --release --target $Target
} finally {
  Pop-Location
}

$built = Join-Path $shim "target/$Target/release/version.dll"
if (-not (Test-Path $built)) {
  throw "Não encontrei o artefato em $built. O build falhou?"
}

New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item $built (Join-Path $dest "version.dll") -Force

$sha = (Get-FileHash $built -Algorithm SHA256).Hash.ToLower()
Write-Host "shim instalado: src-tauri/resources/discord/version.dll"
Write-Host "sha256: $sha"
Write-Host ""
Write-Host "ATENCAO: o shim NAO foi carregado dentro de um Discord real por este"
Write-Host "script. Antes de confiar, feche os gates de docs/discord-dll.md §12"
Write-Host "(import estatico de version.dll, antivirus, CFG/CET)."

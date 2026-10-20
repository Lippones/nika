<#
.SYNOPSIS
  Baixa o WinDivert (driver + DLL) e o instala em src-tauri/resources/windivert/.

.DESCRIPTION
  Componente do túnel do Discord (docs/discord-tunnel.md T-07/T-11). Mesmo
  desenho do fetch-tor.ps1: versão e SHA-256 fixados em scripts/windivert.lock.json;
  qualquer byte diferente é recusado. O projeto não assina os zips com GPG; a
  âncora é o driver ser assinado pela Microsoft (senão o Windows não o carrega)
  e o hash do zip oficial ter sido fixado por uma pessoa no -Update.

.EXAMPLE
  pwsh scripts/fetch-windivert.ps1
  pwsh scripts/fetch-windivert.ps1 -Version 2.2.2 -Update
#>
[CmdletBinding()]
param(
  [string]$Version,
  [switch]$Update
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$lockPath = Join-Path $PSScriptRoot "windivert.lock.json"
$out = Join-Path $root "src-tauri/resources/windivert"

$lock = Get-Content $lockPath -Raw | ConvertFrom-Json
if (-not $Version) { $Version = $lock.version }
if (-not $Version) {
  throw "Informe -Version (veja https://github.com/basil00/WinDivert/releases)."
}

$archive = "WinDivert-$Version-A.zip"
$url = "https://github.com/basil00/WinDivert/releases/download/v$Version/$archive"
$work = Join-Path ([System.IO.Path]::GetTempPath()) "nika-windivert-$Version"
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $work | Out-Null

Write-Host "→ baixando $archive"
$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest $url -OutFile (Join-Path $work $archive)
$hash = (Get-FileHash (Join-Path $work $archive) -Algorithm SHA256).Hash.ToLower()

if (-not $Update) {
  if (-not $lock.sha256 -or $lock.version -ne $Version) {
    throw "sem checksum fixado para $Version — rode com -Update."
  }
  if ($hash -ne $lock.sha256) {
    throw "sha256 $hash difere do fixado ($($lock.sha256)). NÃO use este arquivo."
  }
  Write-Host "→ sha256 confere com o fixado em windivert.lock.json"
}

Write-Host "→ extraindo x64/WinDivert.dll e x64/WinDivert64.sys"
$extract = Join-Path $work "zip"
Expand-Archive (Join-Path $work $archive) -DestinationPath $extract -Force
$x64 = Join-Path $extract "WinDivert-$Version-A/x64"
$dll = Join-Path $x64 "WinDivert.dll"
$sys = Join-Path $x64 "WinDivert64.sys"
$license = Join-Path $extract "WinDivert-$Version-A/LICENSE"

$dllHash = (Get-FileHash $dll -Algorithm SHA256).Hash.ToLower()
$sysHash = (Get-FileHash $sys -Algorithm SHA256).Hash.ToLower()

if ($Update) {
  $lock.version = $Version
  $lock.sha256 = $hash
  $lock.files = [ordered]@{ "WinDivert.dll" = $dllHash; "WinDivert64.sys" = $sysHash }
  $lock | ConvertTo-Json -Depth 5 | Set-Content $lockPath
  Write-Host "→ lock atualizado: $Version / $hash"
} else {
  if ($dllHash -ne $lock.files."WinDivert.dll") { throw "WinDivert.dll extraída não bate com o lock" }
  if ($sysHash -ne $lock.files."WinDivert64.sys") { throw "WinDivert64.sys extraído não bate com o lock" }
}

Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $out | Out-Null
Copy-Item $dll, $sys, $license $out -Force

Write-Host "✓ WinDivert $Version instalado em $out"

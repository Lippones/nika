<#
.SYNOPSIS
  Baixa o Tor Expert Bundle e o instala em src-tauri/resources/.

.DESCRIPTION
  Dois modos, com garantias diferentes:

    reproduzir (padrão)  a versão e o SHA-256 já estão fixados no lock; baixa e
                         recusa qualquer byte diferente. É o que a CI faz.
    atualizar (-Update)  muda o que está fixado; aqui a assinatura GPG do
                         projeto Tor é obrigatória, porque é o único momento em
                         que uma âncora de confiança nova entra no repositório.

.EXAMPLE
  pwsh scripts/fetch-tor.ps1
  pwsh scripts/fetch-tor.ps1 -Version 15.0.19 -Update
#>
[CmdletBinding()]
param(
  [string]$Version,
  [switch]$Update
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$lockPath = Join-Path $PSScriptRoot "tor-bundle.lock.json"
$resources = Join-Path $root "src-tauri/resources"

# Chave "Tor Browser Developers", que assina os pacotes oficiais.
$signingKey = "EF6E286DDA85EA2A4BA7DE684E2C6E8793298290"

$lock = Get-Content $lockPath -Raw | ConvertFrom-Json
if (-not $Version) { $Version = $lock.version }
if (-not $Version) {
  throw "Informe -Version (veja a versão atual em https://www.torproject.org/download/tor/)."
}

$pinned = $lock.sha256 -and ($lock.version -eq $Version) -and (-not $Update)

$archive = "tor-expert-bundle-windows-x86_64-$Version.tar.gz"
$base = "https://archive.torproject.org/tor-package-archive/torbrowser/$Version"
$work = Join-Path ([System.IO.Path]::GetTempPath()) "nika-tor-$Version"
New-Item -ItemType Directory -Force -Path $work | Out-Null

Write-Host "→ baixando $archive"
$ProgressPreference = "SilentlyContinue"
Invoke-WebRequest "$base/$archive" -OutFile (Join-Path $work $archive)

$hash = (Get-FileHash (Join-Path $work $archive) -Algorithm SHA256).Hash.ToLower()

if ($pinned) {
  # O checksum fixado já foi conferido contra a assinatura por uma pessoa, no
  # -Update. Bytes idênticos = mesma garantia, sem depender de keyserver.
  if ($hash -ne $lock.sha256) {
    throw "sha256 $hash difere do fixado ($($lock.sha256)). NÃO use este arquivo."
  }
  Write-Host "→ sha256 confere com o fixado em tor-bundle.lock.json"
} else {
  if (-not $Update) { throw "sem checksum fixado para $Version — rode com -Update." }
  if (-not (Get-Command gpg -ErrorAction SilentlyContinue)) {
    throw "gpg é obrigatório para fixar uma versão nova. Instale o Gpg4win."
  }

  Write-Host "→ verificando assinatura GPG"
  Invoke-WebRequest "$base/$archive.asc" -OutFile (Join-Path $work "$archive.asc")
  gpg --keyserver keys.openpgp.org --recv-keys $signingKey 2>$null | Out-Null
  gpg --verify (Join-Path $work "$archive.asc") (Join-Path $work $archive)
  if ($LASTEXITCODE -ne 0) { throw "assinatura GPG inválida — NÃO use este arquivo." }

  $lock.version = $Version
  $lock.sha256 = $hash
  $lock | ConvertTo-Json -Depth 5 | Set-Content $lockPath
  Write-Host "→ lock atualizado: $Version / $hash"
}

Write-Host "→ extraindo"
$extract = Join-Path $work "bundle"
Remove-Item -Recurse -Force $extract -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $extract | Out-Null
tar -xzf (Join-Path $work $archive) -C $extract

# Só o que a Fase 1 usa: o tor e as DLLs que ele carregue (versões recentes do
# bundle são estáticas e não trazem nenhuma). Ficam de fora o tor-gencert.exe
# (ferramenta de autoridade de diretório) e pluggable_transports/ (~30 MB, entra
# junto com o suporte a bridges, RF-25).
$torOut = Join-Path $resources "tor"
Remove-Item -Recurse -Force $torOut -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $torOut | Out-Null
Copy-Item (Join-Path $extract "tor/tor.exe") $torOut -Force
Get-ChildItem (Join-Path $extract "tor") -Filter *.dll -ErrorAction SilentlyContinue |
  Copy-Item -Destination $torOut -Force

Copy-Item (Join-Path $extract "data/geoip") $resources -Force
Copy-Item (Join-Path $extract "data/geoip6") $resources -Force

Write-Host "✓ Tor $Version instalado em $resources"

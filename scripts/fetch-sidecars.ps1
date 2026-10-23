# Downloads pinned sidecar binaries into src-tauri/binaries and verifies them.
# Usage: powershell -ExecutionPolicy Bypass -File scripts/fetch-sidecars.ps1
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Triple = 'x86_64-pc-windows-msvc'
$Root = Split-Path -Parent $PSScriptRoot
$BinDir = Join-Path $Root 'src-tauri\binaries'
New-Item -ItemType Directory -Force $BinDir | Out-Null

$YtDlpVersion = '2026.10.21'
$YtDlpBase = "https://github.com/yt-dlp/yt-dlp/releases/download/$YtDlpVersion"

function Get-ExpectedHash([string]$SumsUrl, [string]$FileName) {
    $sums = (Invoke-WebRequest -Uri $SumsUrl -UseBasicParsing -Headers @{ 'User-Agent' = 'rabbit-build' }).Content
    if ($sums -is [byte[]]) { $sums = [Text.Encoding]::UTF8.GetString($sums) }
    foreach ($line in $sums -split "`n") {
        $parts = $line.Trim() -split '\s+', 2
        if ($parts.Count -eq 2 -and $parts[1].TrimStart('*') -eq $FileName) { return $parts[0].ToLowerInvariant() }
    }
    throw "No checksum for $FileName in $SumsUrl"
}

function Install-Verified([string]$Url, [string]$Expected, [string]$Dest) {
    $tmp = "$Dest.download"
    Invoke-WebRequest -Uri $Url -OutFile $tmp -UseBasicParsing -Headers @{ 'User-Agent' = 'rabbit-build' }
    $actual = (Get-FileHash -Algorithm SHA256 $tmp).Hash.ToLowerInvariant()
    if ($actual -ne $Expected) {
        Remove-Item $tmp -Force
        throw "Checksum mismatch for $Url`n expected $Expected`n actual   $actual"
    }
    Move-Item -Force $tmp $Dest
    Write-Host "ok  $(Split-Path -Leaf $Dest)  sha256=$actual"
}

# FFmpeg: BtbN's LGPL shared build (LGPLv3), pinned to a dated release. Only ffmpeg.exe, ffprobe.exe and
# the DLLs they load are kept (no ffplay). The hash is the one GitHub publishes for this asset.
$FfTag = 'autobuild-2026-10-22-13-06'
$FfAsset = 'ffmpeg-n9.0.2-33-g4cf298e66a-win64-lgpl-shared-9.0.zip'
$FfHash = '14d2da681ed1999a6232de61f99d869a46df437dd910d69c1fb5c9c83172242e'
$FfDlls = 'avcodec-63.dll', 'avdevice-63.dll', 'avfilter-12.dll', 'avformat-63.dll', 'avutil-61.dll', 'swresample-7.dll', 'swscale-10.dll'

$ytDest = Join-Path $BinDir "yt-dlp-$Triple.exe"
$ytHash = Get-ExpectedHash "$YtDlpBase/SHA2-256SUMS" 'yt-dlp.exe'
if ((Test-Path $ytDest) -and ((Get-FileHash -Algorithm SHA256 $ytDest).Hash.ToLowerInvariant() -eq $ytHash)) {
    Write-Host "ok  yt-dlp $YtDlpVersion already present"
} else {
    Install-Verified "$YtDlpBase/yt-dlp.exe" $ytHash $ytDest
}

$FfDir = Join-Path $BinDir 'ffmpeg'
$FfStamp = Join-Path $FfDir 'SOURCE.txt'
if ((Test-Path $FfStamp) -and ((Get-Content $FfStamp -TotalCount 1) -eq $FfAsset)) {
    Write-Host "ok  ffmpeg $FfAsset already present"
} else {
    # Short temp path: the archive's nested folder names exceed MAX_PATH under deep project folders.
    $tmp = Join-Path $env:TEMP "rabbit-ff-$PID"
    New-Item -ItemType Directory -Force $tmp | Out-Null
    try {
        $zip = Join-Path $tmp 'ff.zip'
        Install-Verified "https://github.com/BtbN/FFmpeg-Builds/releases/download/$FfTag/$FfAsset" $FfHash $zip
        tar -xf $zip -C $tmp
        if ($LASTEXITCODE -ne 0) { throw "couldn't extract $FfAsset" }
        $src = Join-Path (Get-ChildItem $tmp -Directory | Select-Object -First 1).FullName 'bin'
        New-Item -ItemType Directory -Force $FfDir | Out-Null
        Get-ChildItem $FfDir -Filter *.dll | Remove-Item -Force
        foreach ($dll in $FfDlls) { Copy-Item (Join-Path $src $dll) $FfDir }
        Copy-Item (Join-Path $src 'ffmpeg.exe') (Join-Path $BinDir "ffmpeg-$Triple.exe") -Force
        Copy-Item (Join-Path $src 'ffprobe.exe') (Join-Path $BinDir "ffprobe-$Triple.exe") -Force
        Copy-Item (Join-Path (Split-Path $src) 'LICENSE.txt') (Join-Path $FfDir 'LICENSE.txt') -Force
        Set-Content -Encoding utf8 $FfStamp @($FfAsset, "https://github.com/BtbN/FFmpeg-Builds/releases/tag/$FfTag", "sha256 $FfHash")
        Write-Host "ok  ffmpeg + ffprobe + $($FfDlls.Count) DLLs from $FfAsset"
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}

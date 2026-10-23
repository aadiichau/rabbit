# Packs the portable ZIP from a release build: rabbit.exe, the bundled tools, FFmpeg's DLLs, the licences and the
# portable.txt marker that switches Rabbit's data to a `data` folder next to the exe (src-tauri/src/sys/portable.rs).
# Usage: ./scripts/make-portable.ps1 -Exe <path to release rabbit.exe> -OutDir <folder>
param(
    [Parameter(Mandatory)] [string]$Exe,
    [Parameter(Mandatory)] [string]$OutDir
)
$ErrorActionPreference = 'Stop'

$Root = Split-Path -Parent $PSScriptRoot
$Bin = Join-Path $Root 'src-tauri\binaries'
$Triple = 'x86_64-pc-windows-msvc'
$Version = (Get-Content (Join-Path $Root 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json).version
$Name = "Rabbit_${Version}_x64-portable"

$Stage = Join-Path ([IO.Path]::GetTempPath()) "rabbit-portable-$PID"
$App = Join-Path $Stage 'Rabbit'
New-Item -ItemType Directory -Force (Join-Path $App 'licenses') | Out-Null

Copy-Item $Exe (Join-Path $App 'rabbit.exe')
foreach ($tool in 'yt-dlp', 'ffmpeg', 'ffprobe') {
    Copy-Item (Join-Path $Bin "$tool-$Triple.exe") (Join-Path $App "$tool.exe")
}
Copy-Item (Join-Path $Bin 'ffmpeg\*.dll') $App
Copy-Item (Join-Path $Bin 'ffmpeg\LICENSE.txt') (Join-Path $App 'licenses\FFmpeg-LICENSE.txt')
Copy-Item (Join-Path $Bin 'ffmpeg\SOURCE.txt') (Join-Path $App 'licenses\FFmpeg-SOURCE.txt')
Copy-Item (Join-Path $Root 'src-tauri\licenses\yt-dlp-LICENSE.txt') (Join-Path $App 'licenses\yt-dlp-LICENSE.txt')
Copy-Item (Join-Path $Root 'THIRD_PARTY_NOTICES.md') (Join-Path $App 'licenses\THIRD_PARTY_NOTICES.md')
Copy-Item (Join-Path $Root 'LICENSE') (Join-Path $App 'licenses\Rabbit-LICENSE.txt')

@"
Rabbit $Version (portable)

This file switches Rabbit to portable mode: the library, settings, yt-dlp updates and browser data are kept in
the "data" folder next to rabbit.exe, so you can move this folder or run it from a USB drive. Delete this file to
use the normal per-user folders instead.

Start Rabbit with rabbit.exe. Downloads go to your Downloads\Rabbit folder unless you choose another one in
Settings. Requires Windows 10 or 11 (64-bit) with Microsoft Edge WebView2, which Windows 11 includes; on
Windows 10 without it, use the installer instead, which adds it.

To remove Rabbit, close it and delete this folder.
"@ | Out-File -Encoding utf8 (Join-Path $App 'portable.txt')

New-Item -ItemType Directory -Force $OutDir | Out-Null
$Zip = Join-Path (Resolve-Path $OutDir) "$Name.zip"
if (Test-Path $Zip) { Remove-Item $Zip }
Compress-Archive -Path $App -DestinationPath $Zip -CompressionLevel Optimal
Remove-Item $Stage -Recurse -Force
Get-Item $Zip

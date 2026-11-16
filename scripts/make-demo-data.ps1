# Creates a fictional folder tree for screenshots and demos.
# Files are NTFS sparse files: they report large sizes but use almost no disk space.
# Usage: pwsh -File scripts/make-demo-data.ps1 -Root <empty folder on an NTFS drive>
param([Parameter(Mandatory = $true)][string]$Root)

$ErrorActionPreference = 'Stop'
if (Test-Path $Root) {
    if (Get-ChildItem -LiteralPath $Root -Force | Select-Object -First 1) { throw "Root '$Root' must be empty or absent." }
} else {
    New-Item -ItemType Directory -Path $Root | Out-Null
}

function New-SparseFile([string]$Path, [long]$Bytes) {
    $dir = Split-Path $Path -Parent
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $null = New-Item -ItemType File -Path $Path -Force
    fsutil sparse setflag "$Path" | Out-Null
    $fs = [System.IO.File]::Open($Path, 'Open', 'Write')
    try { $fs.SetLength($Bytes) } finally { $fs.Close() }
}

$GB = 1GB; $MB = 1MB
$rand = [System.Random]::new(42)

# Games
New-SparseFile "$Root\Games\Starfall Odyssey\data\world.pak" (38 * $GB)
New-SparseFile "$Root\Games\Starfall Odyssey\data\textures_hd.pak" (21 * $GB)
New-SparseFile "$Root\Games\Starfall Odyssey\bin\game.exe" (180 * $MB)
New-SparseFile "$Root\Games\Racing Pulse\content\tracks.pak" (17 * $GB)
New-SparseFile "$Root\Games\Racing Pulse\content\cars.pak" (9 * $GB)
New-SparseFile "$Root\Games\Pixel Farm\assets.bin" (2 * $GB)

# Videos
foreach ($i in 1..14) { New-SparseFile ("$Root\Videos\Trips\trip_{0:D2}.mp4" -f $i) ([long]($rand.Next(900, 4200)) * $MB) }
foreach ($i in 1..6) { New-SparseFile ("$Root\Videos\Screen recordings\rec_{0:D2}.mkv" -f $i) ([long]($rand.Next(400, 2500)) * $MB) }
New-SparseFile "$Root\Videos\Family\wedding_4k.mov" (27 * $GB)

# Projects with node_modules and build caches
foreach ($p in 'web-shop', 'portfolio', 'chat-bot') {
    foreach ($i in 1..40) { New-SparseFile ("$Root\Projects\$p\node_modules\pkg-{0:D3}\index.js" -f $i) ([long]($rand.Next(50, 9000)) * 1KB) }
    New-SparseFile "$Root\Projects\$p\node_modules\.cache\babel.bin" ([long]($rand.Next(200, 900)) * $MB)
    New-SparseFile "$Root\Projects\$p\dist\bundle.js" ([long]($rand.Next(2, 30)) * $MB)
    foreach ($i in 1..25) { New-SparseFile ("$Root\Projects\$p\src\module_{0:D2}.ts" -f $i) ([long]($rand.Next(2, 60)) * 1KB) }
}
New-SparseFile "$Root\Projects\rust-tool\target\release\deps\big.rlib" (3 * $GB)
New-SparseFile "$Root\Projects\rust-tool\target\debug\incremental.bin" (5 * $GB)

# Photos
foreach ($y in 2022..2025) { foreach ($i in 1..60) { New-SparseFile ("$Root\Photos\$y\IMG_{0:D4}.jpg" -f $i) ([long]($rand.Next(3, 14)) * $MB) } }
foreach ($i in 1..30) { New-SparseFile ("$Root\Photos\RAW\DSC_{0:D4}.arw" -f $i) ([long]($rand.Next(40, 70)) * $MB) }

# Downloads and archives
New-SparseFile "$Root\Downloads\linux-distro.iso" (5 * $GB)
New-SparseFile "$Root\Downloads\installer_setup.exe" (850 * $MB)
New-SparseFile "$Root\Downloads\dataset.zip" (12 * $GB)
New-SparseFile "$Root\Archive\backup-2024.7z" (46 * $GB)
New-SparseFile "$Root\Archive\old-phone-backup.tar" (19 * $GB)

# Caches and temp
New-SparseFile "$Root\AppData\Browser\Cache\data_1" (2400 * $MB)
New-SparseFile "$Root\AppData\Browser\Cache\data_2" (1700 * $MB)
New-SparseFile "$Root\AppData\Package Manager\cache\packages.db" (6 * $GB)
foreach ($i in 1..50) { New-SparseFile ("$Root\Temp\tmp{0:D3}.tmp" -f $i) ([long]($rand.Next(1, 120)) * $MB) }

# Documents
foreach ($i in 1..80) { New-SparseFile ("$Root\Documents\Work\report_{0:D3}.pdf" -f $i) ([long]($rand.Next(200, 9000)) * 1KB) }
New-SparseFile "$Root\Documents\Virtual Machines\dev-vm.vhdx" (64 * $GB)

$files = (Get-ChildItem -LiteralPath $Root -Recurse -File).Count
Write-Output "Created $files sparse files under $Root"

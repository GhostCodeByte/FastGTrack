$FontUrl = "https://github.com/google/fonts/raw/main/ofl/sora/Sora%5Bwght%5D.ttf"
$ExtractDir = "..\ui\fonts"
$DestPath = "$ExtractDir\Sora.ttf"

Set-Location -Path $PSScriptRoot

if (-not (Test-Path -Path $ExtractDir)) {
    New-Item -ItemType Directory -Path $ExtractDir -Force | Out-Null
}

Write-Host "Downloading direct Sora TTF font..."
Invoke-WebRequest -Uri $FontUrl -OutFile $DestPath

Write-Host "Done! Font downloaded successfully."

$ErrorActionPreference = 'Stop'

$repo       = "WendellOttoni/Noctune"
$installDir = "$env:LOCALAPPDATA\Programs\noctune"

Write-Host "Fetching latest release..."
$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$version = $release.tag_name
$asset   = $release.assets | Where-Object { $_.name -eq "noctune-windows-x64.exe" }

if (-not $asset) {
    Write-Error "Windows binary not found in release $version. Check https://github.com/$repo/releases"
    exit 1
}

Write-Host "Downloading noctune $version..."
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
$dest = "$installDir\noctune.exe"
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $dest

# Add install dir to user PATH if not already present
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$installDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$installDir", "User")
    Write-Host "Added $installDir to PATH."
} else {
    Write-Host "noctune $version installed."
}

# yt-dlp — required for YouTube/playlist playback
Write-Host ""
if (Get-Command yt-dlp -ErrorAction SilentlyContinue) {
    Write-Host "yt-dlp found — YouTube playback is ready."
} else {
    Write-Host "yt-dlp is not installed."
    Write-Host "It is required to add YouTube links and playlists."
    Write-Host "Without it, only local files and radio streams will work."
    Write-Host ""

    $answer = Read-Host "Install yt-dlp now? [Y/n]"
    if ($answer -eq '' -or $answer -match '^[Yy]') {
        if (Get-Command winget -ErrorAction SilentlyContinue) {
            Write-Host "Installing yt-dlp via winget..."
            winget install yt-dlp.yt-dlp --silent --accept-package-agreements --accept-source-agreements
            if ($LASTEXITCODE -eq 0) {
                Write-Host "yt-dlp installed successfully."
            } else {
                Write-Host "winget install failed. Try manually: winget install yt-dlp.yt-dlp"
            }
        } else {
            Write-Host "winget is not available on this machine."
            Write-Host "Install yt-dlp manually from: https://github.com/yt-dlp/yt-dlp/releases/latest"
            Write-Host "Download yt-dlp.exe and place it somewhere in your PATH."
        }
    } else {
        Write-Host "Skipped. You can install it later with: winget install yt-dlp.yt-dlp"
    }
}

Write-Host ""
Write-Host "Done! Restart your terminal, then run: noctune"
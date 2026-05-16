$ErrorActionPreference = 'Stop'

$repo    = "WendellOttoni/Noctune"
$installDir = "$env:LOCALAPPDATA\Programs\noctune"

Write-Host "Fetching latest release..."
$release  = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$version  = $release.tag_name
$asset    = $release.assets | Where-Object { $_.name -eq "noctune-windows-x64.exe" }

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
    Write-Host "Restart your terminal, then run: noctune"
} else {
    Write-Host "noctune $version installed. Run: noctune"
}

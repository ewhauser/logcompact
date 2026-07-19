$ErrorActionPreference = "Stop"

$distVersion = "0.32.0"
$installerSha256 = "a3435e9944f1a1297add11c6a8ac1f543c14a5ea88879ee05b24ff8218d46d87"
$installerUrl = "https://github.com/axodotdev/cargo-dist/releases/download/v$distVersion/cargo-dist-installer.ps1"
$installer = Join-Path ([System.IO.Path]::GetTempPath()) "cargo-dist-installer-$PID.ps1"

try {
    Invoke-WebRequest -Uri $installerUrl -OutFile $installer
    $actualSha256 = (Get-FileHash -Algorithm SHA256 -Path $installer).Hash.ToLowerInvariant()
    if ($actualSha256 -ne $installerSha256) {
        throw "cargo-dist installer checksum mismatch: expected $installerSha256, got $actualSha256"
    }
    & $installer
} finally {
    Remove-Item -Path $installer -Force -ErrorAction SilentlyContinue
}

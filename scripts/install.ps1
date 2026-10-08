# Builds Yazi from this checkout and installs `yazi`, `ya`, and a `y` copy of `yazi` into Cargo's bin directory.
$ErrorActionPreference = "Stop"

$root = Split-Path $PSScriptRoot -Parent

# Windows builds require unwinding, which the plain `release` profile disables.
foreach ($crate in "yazi-fm", "yazi-cli") {
	cargo install --path (Join-Path $root $crate) --profile release-windows --locked --force
	if ($LASTEXITCODE) { exit $LASTEXITCODE }
}

$bin = if ($env:CARGO_INSTALL_ROOT) {
	Join-Path $env:CARGO_INSTALL_ROOT "bin"
} elseif ($env:CARGO_HOME) {
	Join-Path $env:CARGO_HOME "bin"
} else {
	Join-Path $env:USERPROFILE ".cargo\bin"
}

Copy-Item (Join-Path $bin "yazi.exe") (Join-Path $bin "y.exe") -Force
Write-Host "Installed yazi.exe, ya.exe, and y.exe to $bin"

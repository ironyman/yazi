# Ships the current commit to `myyazi` users: moves the `shipped` tag to HEAD, pushes it,
# and publishes the `myyazi` installer crate if its version isn't on crates.io yet.
# Assumes `cargo login` has already been done.
param(
	[string]$Remote = "mine"
)
$ErrorActionPreference = "Stop"

$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
	if (git status --porcelain) { throw "Working tree is not clean; commit or stash changes first." }

	# The installer clones the `shipped` tag, so moving it is what ships new code.
	git tag -f shipped HEAD
	git push $Remote HEAD
	if ($LASTEXITCODE) { exit $LASTEXITCODE }
	git push -f $Remote refs/tags/shipped
	if ($LASTEXITCODE) { exit $LASTEXITCODE }

	$meta = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
	$version = ($meta.packages | Where-Object name -eq "myyazi").version

	$published = $false
	try {
		Invoke-RestMethod "https://crates.io/api/v1/crates/myyazi/$version" -UserAgent "myyazi-publish" | Out-Null
		$published = $true
	} catch {
		if ($_.Exception.Response.StatusCode.value__ -ne 404) { throw }
	}

	if ($published) {
		Write-Host "myyazi $version is already on crates.io; 'cargo install --force myyazi' now installs the updated shipped tag."
	} else {
		cargo publish -p myyazi --locked
		if ($LASTEXITCODE) { exit $LASTEXITCODE }
	}
} finally {
	Pop-Location
}

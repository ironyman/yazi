# Ships the current commit to `myyazi` users: moves the `shipped` tag to HEAD, pushes it,
# and publishes the `myyazi` installer crate if its version isn't on crates.io yet.
# Pass -Major, -Minor, or -Revision to bump the installer's version and commit it before shipping.
# Assumes `cargo login` has already been done.
param(
	[string]$Remote = "mine",
	[switch]$Major,
	[switch]$Minor,
	[switch]$Revision
)
$ErrorActionPreference = "Stop"

if ($Major.IsPresent + $Minor.IsPresent + $Revision.IsPresent -gt 1) { throw "Pass only one of -Major, -Minor, or -Revision." }

$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
	if (git status --porcelain) { throw "Working tree is not clean; commit or stash changes first." }

	if ($Major -or $Minor -or $Revision) {
		$manifest = Join-Path $root "yazi-build\Cargo.toml"
		$text = [IO.File]::ReadAllText($manifest)
		$line = [regex]::Match($text, '(?m)^version(\s*=\s*)"([^"]+)"')
		if (!$line.Success) { throw "Could not find the version in $manifest" }

		$old = [version]$line.Groups[2].Value
		$new = if ($Major) {
			"$($old.Major + 1).0.0"
		} elseif ($Minor) {
			"$($old.Major).$($old.Minor + 1).0"
		} else {
			"$($old.Major).$($old.Minor).$($old.Build + 1)"
		}

		$text = $text.Remove($line.Index, $line.Length).Insert($line.Index, "version$($line.Groups[1].Value)`"$new`"")
		[IO.File]::WriteAllText($manifest, $text, [Text.UTF8Encoding]::new($false))

		# Refresh the lockfile entry so `cargo publish --locked` accepts the new version.
		cargo update -w --offline
		if ($LASTEXITCODE) { exit $LASTEXITCODE }

		git commit -m "Bump myyazi to $new" -- $manifest Cargo.lock
		if ($LASTEXITCODE) { exit $LASTEXITCODE }
	}

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

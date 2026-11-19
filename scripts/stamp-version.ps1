# Stamps the workspace version as <version>+<channel>.<sha7> for a build that
# is not a release (main-build.yml), so the window, vmm.exe and the installer
# all name the commit they came from. The release's own build is never stamped.
#
# Only Cargo.toml's [workspace.package] version and the five workspace entries
# in Cargo.lock change, refreshed offline, so `--locked` builds keep every
# dependency exactly as committed. Run it in a checkout whose changes are
# thrown away afterwards (CI); never commit the result.
param(
    [Parameter(Mandatory)] [ValidatePattern('^[0-9a-f]{40}$')] [string] $Sha,
    [ValidatePattern('^[a-z]+$')] [string] $Channel = 'main'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
Push-Location $root
try {
    $members = (cargo metadata --no-deps --format-version 1 --locked | ConvertFrom-Json).packages
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $version = $members | Where-Object name -eq 'vaexil-mod-manager' | Select-Object -ExpandProperty version
    if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "The workspace version $version is not MAJOR.MINOR.PATCH." }
    $stamped = "$version+$Channel.$($Sha.Substring(0, 7))"

    # The offline refresh below needs the registry index cached.
    cargo fetch --locked --quiet
    if ($LASTEXITCODE -ne 0) { throw 'cargo fetch failed' }

    $toml = Join-Path $root 'Cargo.toml'
    $text = [IO.File]::ReadAllText($toml)
    $line = '(?m)^version = "' + [regex]::Escape($version) + '"(?=\r?$)'
    $found = [regex]::Matches($text, $line).Count
    if ($found -ne 1) { throw "Expected one workspace version line in Cargo.toml, found $found." }
    [IO.File]::WriteAllText($toml, [regex]::Replace($text, $line, "version = `"$stamped`""))

    cargo update --workspace --offline --quiet
    if ($LASTEXITCODE -ne 0) { throw 'cargo update --workspace failed' }

    # Exactly the workspace's own entries moved, and nothing else.
    $lock = git diff --numstat -- Cargo.lock
    $count = @($members).Count
    if ($lock -notmatch "^$count\s+$count\s+Cargo\.lock$") { throw "Cargo.lock changed beyond the workspace versions: $lock" }
    $changed = @(git diff --name-only) | Sort-Object
    if (Compare-Object $changed @('Cargo.lock', 'Cargo.toml')) { throw "Unexpected changes: $($changed -join ', ')" }
    $now = (cargo metadata --no-deps --format-version 1 --locked | ConvertFrom-Json).packages |
        Select-Object -ExpandProperty version -Unique
    if ($now -ne $stamped) { throw "The workspace reads $now after stamping, not $stamped." }
    $stamped
} finally {
    Pop-Location
}

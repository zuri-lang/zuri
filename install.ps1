<#
Installs Zuri on Windows from its published releases.

  irm https://zuri-lang.github.io/zuri/install.ps1 | iex

Options need the script as a script block:

  & ([scriptblock]::Create((irm https://zuri-lang.github.io/zuri/install.ps1))) -Version 0.2.0

  -Version <version>  the release to install; the newest one by default
  -Prerelease         consider pre-releases when picking the newest
  -NoModifyPath       leave the user's PATH alone

The archive for this machine is downloaded, checked against the checksum
published beside it, and run once to confirm the version it reports. Only
then does it replace $env:ZURI_HOME\runtime (~\.zuri\runtime by default),
which holds zuri.exe with the standard library and the shipped commands
beside it, where the runtime looks for them. That directory goes on the
user's PATH, and so does $env:ZURI_HOME\bin, where globally installed
tools put their launchers.

Running it again installs over the top. `zuri upgrade` does the same
from inside an installation.

ZURI_RELEASES_URL lists releases in place of GitHub, in the same shape,
for a mirror. GITHUB_TOKEN, when set, is sent to GitHub's API, where it
raises the rate limit.
#>
param(
  [string]$Version = 'latest',
  [switch]$Prerelease,
  [switch]$NoModifyPath
)

$ErrorActionPreference = 'Stop'

# Windows PowerShell draws a progress bar for every block it downloads,
# which slows a large download to a crawl.
$ProgressPreference = 'SilentlyContinue'

$ReleasesUrl = if ($env:ZURI_RELEASES_URL) {
  $env:ZURI_RELEASES_URL
} else {
  'https://api.github.com/repos/zuri-lang/zuri/releases'
}

$Target = 'x86_64-pc-windows-msvc'

# Semantic version precedence: -1, 0 or 1.
function Compare-SemVer([string]$A, [string]$B) {
  $a = ($A -split '\+')[0]
  $b = ($B -split '\+')[0]
  $aPre = ''
  $bPre = ''

  if ($a.Contains('-')) {
    $aPre = $a.Substring($a.IndexOf('-') + 1)
    $a = $a.Substring(0, $a.IndexOf('-'))
  }

  if ($b.Contains('-')) {
    $bPre = $b.Substring($b.IndexOf('-') + 1)
    $b = $b.Substring(0, $b.IndexOf('-'))
  }

  $aCore = $a.Split('.') | ForEach-Object { [long]$_ }
  $bCore = $b.Split('.') | ForEach-Object { [long]$_ }

  for ($i = 0; $i -lt 3; $i++) {
    if ($aCore[$i] -ne $bCore[$i]) {
      if ($aCore[$i] -lt $bCore[$i]) { return -1 } else { return 1 }
    }
  }

  if ($aPre -eq $bPre) { return 0 }
  if ($aPre -eq '') { return 1 }
  if ($bPre -eq '') { return -1 }

  $aParts = $aPre.Split('.')
  $bParts = $bPre.Split('.')

  for ($i = 0; $i -lt [Math]::Max($aParts.Count, $bParts.Count); $i++) {
    if ($i -ge $aParts.Count) { return -1 }
    if ($i -ge $bParts.Count) { return 1 }

    $x = $aParts[$i]
    $y = $bParts[$i]

    if ($x -ceq $y) { continue }

    $xNumeric = $x -match '^\d+$'
    $yNumeric = $y -match '^\d+$'

    if ($xNumeric -and $yNumeric) {
      if ([long]$x -lt [long]$y) { return -1 } else { return 1 }
    }

    if ($xNumeric) { return -1 }
    if ($yNumeric) { return 1 }
    if ([string]::CompareOrdinal($x, $y) -lt 0) { return -1 } else { return 1 }
  }

  return 0
}

# The release to install as @{ Version; Archive; Checksum }, or $null.
# Mirrors what `zuri upgrade` accepts: drafts are skipped, a release
# offers this platform only when both its archive and the archive's
# .sha256 are attached, and a pre-release, by flag or by version, is
# only picked when asked for or named.
function Select-Release($Listing, [string]$Wanted, [bool]$AllowPre) {
  $archives = @{}
  $sums = @{}
  $pre = @{}
  $suffix = "-$Target.zip"

  foreach ($release in $Listing) {
    if ($release.draft -eq $true) {
      continue
    }

    foreach ($asset in $release.assets) {
      $name = [string]$asset.name
      $isChecksum = $name.EndsWith('.sha256')

      if ($isChecksum) {
        $name = $name.Substring(0, $name.Length - 7)
      }

      if (-not $name.StartsWith('zuri-') -or -not $name.EndsWith($suffix)) {
        continue
      }

      $found = $name.Substring(5, $name.Length - 5 - $suffix.Length)

      if ($found -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$') {
        continue
      }

      if ($isChecksum) {
        $sums[$found] = $asset.browser_download_url
      } else {
        $archives[$found] = $asset.browser_download_url
      }

      if ($release.prerelease -eq $true -or $found -match '^[^+]*-') {
        $pre[$found] = $true
      }
    }
  }

  $best = $null

  foreach ($candidate in $archives.Keys) {
    if (-not $sums.ContainsKey($candidate)) {
      continue
    }

    if ($Wanted -ne 'latest') {
      if ($pre.ContainsKey($candidate) -and -not $AllowPre -and $Wanted -notmatch '^[^+]*-') {
        continue
      }

      if ((Compare-SemVer $candidate $Wanted) -eq 0) {
        $best = $candidate
      }

      continue
    }

    if ($pre.ContainsKey($candidate) -and -not $AllowPre) {
      continue
    }

    if ($null -eq $best -or (Compare-SemVer $candidate $best) -gt 0) {
      $best = $candidate
    }
  }

  if ($null -eq $best) {
    return $null
  }

  return @{ Version = $best; Archive = $archives[$best]; Checksum = $sums[$best] }
}

# Tells running programs that the user's environment changed, so a
# terminal opened afterwards sees the new PATH.
function Publish-Environment {
  if (-not ('Zuri.Installer.NativeMethods' -as [type])) {
    Add-Type -Namespace Zuri.Installer -Name NativeMethods -MemberDefinition @'
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
public static extern IntPtr SendMessageTimeout(
  IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
  uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
'@
  }

  $result = [UIntPtr]::Zero

  [Zuri.Installer.NativeMethods]::SendMessageTimeout(
    [IntPtr]0xffff, 0x1a, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result
  ) | Out-Null
}

# Puts each of $Directories at the front of the user's PATH, unless it is
# there already. Returns the ones it added.
function Add-UserPath([string[]]$Directories) {
  $key = Get-Item -Path 'HKCU:\Environment'

  # Read unexpanded, so an entry written as %USERPROFILE%\... stays that way.
  $current = [string]$key.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
  $entries = @($current.Split(';') | Where-Object { $_ -ne '' })
  $added = @()

  foreach ($directory in $Directories) {
    $present = $entries | Where-Object {
      [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') -ieq $directory.TrimEnd('\')
    }

    if (-not $present) {
      $added += $directory
    }
  }

  if ($added.Count -gt 0) {
    $updated = (@($added) + $entries) -join ';'

    Set-ItemProperty -Path 'HKCU:\Environment' -Name 'Path' -Value $updated -Type ExpandString
    Publish-Environment
  }

  return $added
}

function Install-Zuri {
  $staging = $null

  try {
    if ($PSVersionTable.PSVersion.Major -ge 6 -and -not $IsWindows) {
      throw 'on Linux and macOS, install with: curl -fsSL https://zuri-lang.github.io/zuri/install.sh | sh'
    }

    if (-not [Environment]::Is64BitOperatingSystem) {
      throw 'there is no Zuri release for 32-bit Windows'
    }

    # Windows PowerShell offers older TLS versions first, which GitHub
    # refuses.
    if ($PSVersionTable.PSVersion.Major -lt 6) {
      [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    }

    $wanted = $Version.TrimStart('v')
    $zuriHome = if ($env:ZURI_HOME) { $env:ZURI_HOME } else { Join-Path $HOME '.zuri' }
    $zuriHome = [IO.Path]::GetFullPath($zuriHome)
    $runtime = Join-Path $zuriHome 'runtime'
    $bin = Join-Path $zuriHome 'bin'

    New-Item -ItemType Directory -Force -Path $zuriHome | Out-Null

    # Everything is staged beside the installation, so the final move is
    # a rename on one volume rather than a copy.
    $staging = Join-Path $zuriHome ('.install.' + [guid]::NewGuid().ToString('N').Substring(0, 8))

    New-Item -ItemType Directory -Path $staging | Out-Null

    $headers = @{ Accept = 'application/vnd.github+json'; 'User-Agent' = 'zuri-install' }

    # The token goes to GitHub's API and nowhere else.
    if ($env:GITHUB_TOKEN -and $ReleasesUrl.StartsWith('https://api.github.com/')) {
      $headers['Authorization'] = "Bearer $env:GITHUB_TOKEN"
    }

    $separator = if ($ReleasesUrl.Contains('?')) { '&' } else { '?' }

    try {
      $listing = Invoke-RestMethod -Uri "$ReleasesUrl${separator}per_page=100" -Headers $headers -UseBasicParsing
    } catch {
      throw "could not read the releases at ${ReleasesUrl}: $($_.Exception.Message)"
    }

    $release = Select-Release $listing $wanted $Prerelease.IsPresent

    if ($null -eq $release) {
      if ($wanted -eq 'latest') {
        throw "there is no release of zuri for $Target yet"
      }

      throw "there is no release of zuri $wanted for $Target"
    }

    $found = $release.Version
    $asset = "zuri-$found-$Target.zip"
    $archive = Join-Path $staging $asset
    $checksum = "$archive.sha256"
    $download = @{ 'User-Agent' = 'zuri-install' }

    Write-Host "Downloading zuri $found for $Target"

    try {
      Invoke-WebRequest -Uri $release.Archive -OutFile $archive -Headers $download -UseBasicParsing
      Invoke-WebRequest -Uri $release.Checksum -OutFile $checksum -Headers $download -UseBasicParsing
    } catch {
      throw "could not download ${asset}: $($_.Exception.Message)"
    }

    $expected = ((Get-Content -Raw -Path $checksum).Trim() -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 -Path $archive).Hash.ToLowerInvariant()

    if ($expected -ne $actual) {
      throw "$asset does not match its published checksum; nothing was installed"
    }

    $unpackedRoot = Join-Path $staging 'unpacked'

    Expand-Archive -Path $archive -DestinationPath $unpackedRoot

    $unpacked = Join-Path $unpackedRoot "zuri-$found-$Target"

    foreach ($part in 'zuri.exe', 'libs', 'cmds') {
      if (-not (Test-Path (Join-Path $unpacked $part))) {
        throw "$asset has no $part in it; nothing was installed"
      }
    }

    $reported = ''

    try {
      $reported = (& (Join-Path $unpacked 'zuri.exe') --version 2> $null | Out-String).Trim()
    } catch {
      $reported = ''
    }

    if ($reported -eq '') {
      throw 'the downloaded runtime did not start on this machine; nothing was installed'
    }

    if ($reported -notmatch "Zuri $([regex]::Escape($found))(\s|$)") {
      throw "the downloaded runtime says it is '$(($reported -split "`n")[0].Trim())', not zuri $found"
    }

    # The old runtime is moved aside rather than deleted, so a failed
    # move puts it back.
    $previous = Join-Path $staging 'previous'

    if (Test-Path $runtime) {
      try {
        Move-Item -Path $runtime -Destination $previous
      } catch {
        throw "could not replace $runtime; close any running zuri, such as an editor's language server, and run the installer again"
      }
    }

    try {
      Move-Item -Path $unpacked -Destination $runtime
    } catch {
      if (Test-Path $previous) {
        Move-Item -Path $previous -Destination $runtime
      }

      throw "could not move the runtime into $runtime; the previous one is back in place"
    }

    New-Item -ItemType Directory -Force -Path $bin | Out-Null

    Write-Host "Installed zuri $found in $runtime"

    if ($NoModifyPath) {
      Write-Host ''
      Write-Host "Add $runtime and $bin to PATH to run zuri from anywhere."
    } else {
      $added = Add-UserPath @($runtime, $bin)

      if ($added.Count -gt 0) {
        Write-Host "Added $($added -join ' and ') to your PATH"
      }

      # This session too, so zuri works here straight away when the
      # installer was pasted into it.
      foreach ($directory in $bin, $runtime) {
        if (-not (($env:Path -split ';') -contains $directory)) {
          $env:Path = "$directory;$env:Path"
        }
      }

      # A new terminal puts the system's PATH before the user's, so a zuri
      # installed for every user wins over this one.
      $system = [Environment]::GetEnvironmentVariable('Path', 'Machine')

      foreach ($directory in @(([string]$system).Split(';') | Where-Object { $_ -ne '' })) {
        $other = Join-Path ([Environment]::ExpandEnvironmentVariables($directory)) 'zuri.exe'

        if (Test-Path $other) {
          Write-Host ''
          Write-Host "$other is on the system PATH, which comes before yours, so it is the zuri new terminals run."

          break
        }
      }
    }

    Write-Host ''
    Write-Host 'Get started with:'
    Write-Host ''
    Write-Host '  zuri --help'

    return $true
  } catch {
    Write-Host "zuri install: $($_.Exception.Message)" -ForegroundColor Red

    return $false
  } finally {
    if ($staging) {
      Remove-Item -Recurse -Force -Path $staging -ErrorAction SilentlyContinue
    }
  }
}

# A pasted installer runs in the user's own session, which `exit` would
# close; only a script run as a file reports failure through its exit code.
if (-not (Install-Zuri) -and $PSCommandPath) {
  exit 1
}

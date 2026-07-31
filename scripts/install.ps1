[CmdletBinding()]
param(
    [ValidateSet(
        "auto",
        "all",
        "universal",
        "a3s-code",
        "codex",
        "claude-code",
        "cursor",
        "gemini-cli",
        "github-copilot",
        "opencode",
        "cline",
        "roo",
        "windsurf"
    )]
    [string]$Agent = "auto",

    [string]$Version = "",

    [string]$InstallDir = "",

    [string]$SkillDir = "",

    [switch]$SkillOnly,

    [switch]$CliOnly
)

$ErrorActionPreference = "Stop"

if ($SkillOnly -and $CliOnly) {
    throw "--skill-only and --cli-only cannot be combined"
}

$InstallCli = -not $SkillOnly
$InstallSkill = -not $CliOnly
$Repository = if ($env:A3S_TEST_REPOSITORY) { $env:A3S_TEST_REPOSITORY } else { "A3S-Lab/Test" }
$ReleasesUrl = if ($env:A3S_TEST_RELEASES_URL) {
    $env:A3S_TEST_RELEASES_URL.TrimEnd("/")
} else {
    "https://github.com/$Repository/releases"
}

$KnownAgents = @(
    "a3s-code",
    "codex",
    "claude-code",
    "cursor",
    "gemini-cli",
    "github-copilot",
    "opencode",
    "cline",
    "roo",
    "windsurf"
)

function Test-CommandAvailable {
    param([Parameter(Mandatory = $true)][string]$Name)

    return $null -ne (Get-Command -Name $Name -ErrorAction SilentlyContinue)
}

function Test-AgentInstalled {
    param([Parameter(Mandatory = $true)][string]$AgentName)

    switch ($AgentName) {
        "a3s-code" {
            return (
                [bool]$env:A3S_HOME -or
                (Test-Path -LiteralPath (Join-Path $HOME ".a3s") -PathType Container) -or
                (Test-CommandAvailable "a3s")
            )
        }
        "codex" {
            return (
                [bool]$env:CODEX_HOME -or
                (Test-Path -LiteralPath (Join-Path $HOME ".codex") -PathType Container) -or
                (Test-CommandAvailable "codex")
            )
        }
        "claude-code" {
            return (
                [bool]$env:CLAUDE_CONFIG_DIR -or
                (Test-Path -LiteralPath (Join-Path $HOME ".claude") -PathType Container) -or
                (Test-CommandAvailable "claude")
            )
        }
        "cursor" {
            return (
                [bool]$env:CURSOR_HOME -or
                (Test-Path -LiteralPath (Join-Path $HOME ".cursor") -PathType Container) -or
                (Test-CommandAvailable "cursor")
            )
        }
        "gemini-cli" {
            return (
                [bool]$env:GEMINI_HOME -or
                (Test-Path -LiteralPath (Join-Path $HOME ".gemini") -PathType Container) -or
                (Test-CommandAvailable "gemini")
            )
        }
        "github-copilot" {
            return (
                [bool]$env:COPILOT_HOME -or
                (Test-Path -LiteralPath (Join-Path $HOME ".copilot") -PathType Container) -or
                (Test-CommandAvailable "copilot")
            )
        }
        "opencode" {
            $ConfigRoot = if ($env:OPENCODE_CONFIG_DIR) {
                $env:OPENCODE_CONFIG_DIR
            } elseif ($env:XDG_CONFIG_HOME) {
                Join-Path $env:XDG_CONFIG_HOME "opencode"
            } else {
                Join-Path $HOME ".config\opencode"
            }
            return (
                [bool]$env:OPENCODE_CONFIG_DIR -or
                (Test-Path -LiteralPath $ConfigRoot -PathType Container) -or
                (Test-CommandAvailable "opencode")
            )
        }
        "cline" {
            return (
                [bool]$env:CLINE_HOME -or
                (Test-Path -LiteralPath (Join-Path $HOME ".cline") -PathType Container) -or
                (Test-CommandAvailable "cline")
            )
        }
        "roo" {
            return (
                [bool]$env:ROO_HOME -or
                (Test-Path -LiteralPath (Join-Path $HOME ".roo") -PathType Container) -or
                (Test-CommandAvailable "roo")
            )
        }
        "windsurf" {
            return (
                [bool]$env:WINDSURF_HOME -or
                (Test-Path -LiteralPath (Join-Path $HOME ".codeium\windsurf") -PathType Container) -or
                (Test-CommandAvailable "windsurf")
            )
        }
    }
    return $false
}

$DetectedAgents = @()
if ($InstallSkill -and -not $SkillDir -and $Agent -eq "auto") {
    $DetectedAgents = @($KnownAgents | Where-Object { Test-AgentInstalled $_ })
    if ($DetectedAgents.Count -eq 0) {
        $DetectedAgents = @("universal")
        Write-Host "No known coding agent detected; using the universal Agent Skills directory."
    }
}

if (-not $InstallDir) {
    if ($env:A3S_TEST_INSTALL_DIR) {
        $InstallDir = $env:A3S_TEST_INSTALL_DIR
    } else {
        $InstallDir = Join-Path $env:LOCALAPPDATA "A3S\bin"
    }
}

function Copy-Download {
    param(
        [Parameter(Mandatory = $true)][string]$Url,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    $Uri = [Uri]$Url
    if ($Uri.IsFile) {
        Copy-Item -LiteralPath $Uri.LocalPath -Destination $Destination -Force
    } else {
        Invoke-WebRequest -Uri $Url -OutFile $Destination -UseBasicParsing
    }
}

function Resolve-LatestVersion {
    $Response = Invoke-WebRequest -Uri "$ReleasesUrl/latest" -UseBasicParsing
    $FinalUri = $null
    if ($Response.BaseResponse.ResponseUri) {
        $FinalUri = $Response.BaseResponse.ResponseUri.AbsoluteUri
    } elseif ($Response.BaseResponse.RequestMessage.RequestUri) {
        $FinalUri = $Response.BaseResponse.RequestMessage.RequestUri.AbsoluteUri
    }
    if (-not $FinalUri -or $FinalUri -notmatch "/tag/(v[^/?#]+)") {
        throw "Could not resolve the latest A3S Test release"
    }
    return $Matches[1]
}

if (-not $Version) {
    $Version = Resolve-LatestVersion
}
if (-not $Version.StartsWith("v")) {
    $Version = "v$Version"
}

$Target = "x86_64-pc-windows-msvc"
$DownloadBase = "$ReleasesUrl/download/$Version"
$TempRoot = Join-Path ([IO.Path]::GetTempPath()) ("a3s-test-install-" + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $TempRoot | Out-Null

function Assert-Checksum {
    param(
        [Parameter(Mandatory = $true)][string]$Artifact,
        [Parameter(Mandatory = $true)][string]$ChecksumFile
    )

    $Expected = ((Get-Content -LiteralPath $ChecksumFile -Raw).Trim() -split "\s+")[0].ToLowerInvariant()
    $Actual = (Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($Expected -ne $Actual) {
        throw "Checksum verification failed for $(Split-Path -Leaf $Artifact)"
    }
}

function Install-SkillAt {
    param(
        [Parameter(Mandatory = $true)][string]$AgentName,
        [Parameter(Mandatory = $true)][string]$SkillsParent
    )

    $Destination = Join-Path $SkillsParent "a3s-test"
    $Staging = Join-Path $SkillsParent (".a3s-test.install." + [Guid]::NewGuid())
    $Backup = Join-Path $SkillsParent (".a3s-test.backup." + [Guid]::NewGuid())
    New-Item -ItemType Directory -Path $SkillsParent -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $TempRoot "skill\a3s-test") -Destination $Staging -Recurse

    try {
        if (Test-Path -LiteralPath $Destination) {
            Move-Item -LiteralPath $Destination -Destination $Backup
        }
        Move-Item -LiteralPath $Staging -Destination $Destination
        if (Test-Path -LiteralPath $Backup) {
            Remove-Item -LiteralPath $Backup -Recurse -Force
        }
    } catch {
        if (Test-Path -LiteralPath $Staging) {
            Remove-Item -LiteralPath $Staging -Recurse -Force
        }
        if (Test-Path -LiteralPath $Backup) {
            Move-Item -LiteralPath $Backup -Destination $Destination
        }
        throw
    }
    Write-Host "Installed $AgentName Skill: $Destination"
}

function Install-AgentSkill {
    param([Parameter(Mandatory = $true)][string]$AgentName)

    $SkillsParent = switch ($AgentName) {
        "a3s-code" {
            Join-Path $(if ($env:A3S_HOME) { $env:A3S_HOME } else { Join-Path $HOME ".a3s" }) "skills"
        }
        "codex" {
            Join-Path $(if ($env:CODEX_HOME) { $env:CODEX_HOME } else { Join-Path $HOME ".codex" }) "skills"
        }
        "claude-code" {
            Join-Path $(if ($env:CLAUDE_CONFIG_DIR) { $env:CLAUDE_CONFIG_DIR } else { Join-Path $HOME ".claude" }) "skills"
        }
        "cursor" {
            Join-Path $(if ($env:CURSOR_HOME) { $env:CURSOR_HOME } else { Join-Path $HOME ".cursor" }) "skills"
        }
        "gemini-cli" {
            Join-Path $(if ($env:GEMINI_HOME) { $env:GEMINI_HOME } else { Join-Path $HOME ".gemini" }) "skills"
        }
        "github-copilot" {
            Join-Path $(if ($env:COPILOT_HOME) { $env:COPILOT_HOME } else { Join-Path $HOME ".copilot" }) "skills"
        }
        "opencode" {
            $ConfigRoot = if ($env:OPENCODE_CONFIG_DIR) {
                $env:OPENCODE_CONFIG_DIR
            } elseif ($env:XDG_CONFIG_HOME) {
                Join-Path $env:XDG_CONFIG_HOME "opencode"
            } else {
                Join-Path $HOME ".config\opencode"
            }
            Join-Path $ConfigRoot "skills"
        }
        "universal" {
            Join-Path $(if ($env:AGENTS_HOME) { $env:AGENTS_HOME } else { Join-Path $HOME ".agents" }) "skills"
        }
        "cline" {
            Join-Path $(if ($env:CLINE_HOME) { $env:CLINE_HOME } else { Join-Path $HOME ".cline" }) "skills"
        }
        "roo" {
            Join-Path $(if ($env:ROO_HOME) { $env:ROO_HOME } else { Join-Path $HOME ".roo" }) "skills"
        }
        "windsurf" {
            Join-Path $(if ($env:WINDSURF_HOME) { $env:WINDSURF_HOME } else { Join-Path $HOME ".codeium\windsurf" }) "skills"
        }
    }
    Install-SkillAt $AgentName $SkillsParent
}

try {
    if ($InstallCli) {
        $ArchiveName = "a3s-test-$Version-$Target.zip"
        $Archive = Join-Path $TempRoot $ArchiveName
        $ChecksumName = "a3s-test-$Version-$Target.sha256"
        $Checksum = Join-Path $TempRoot $ChecksumName
        Copy-Download "$DownloadBase/$ArchiveName" $Archive
        Copy-Download "$DownloadBase/$ChecksumName" $Checksum
        Assert-Checksum $Archive $Checksum

        $CliExtract = Join-Path $TempRoot "cli"
        Expand-Archive -LiteralPath $Archive -DestinationPath $CliExtract
        $Binary = Get-ChildItem -LiteralPath $CliExtract -Filter "a3s-test.exe" -File -Recurse |
            Select-Object -First 1
        if (-not $Binary) {
            throw "CLI archive did not contain a3s-test.exe"
        }
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Copy-Item -LiteralPath $Binary.FullName -Destination (Join-Path $InstallDir "a3s-test.exe") -Force
        Write-Host "Installed CLI: $(Join-Path $InstallDir 'a3s-test.exe')"

        if (-not $env:A3S_TEST_SKIP_PATH_UPDATE) {
            $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
            $PathEntries = @($UserPath -split ";" | Where-Object { $_ })
            if ($PathEntries -notcontains $InstallDir) {
                $UpdatedPath = (@($PathEntries) + $InstallDir) -join ";"
                [Environment]::SetEnvironmentVariable("Path", $UpdatedPath, "User")
            }
            if (($env:Path -split ";") -notcontains $InstallDir) {
                $env:Path = "$InstallDir;$env:Path"
            }
        }
    }

    if ($InstallSkill) {
        $SkillArchive = Join-Path $TempRoot "a3s-test.skill"
        $SkillZip = Join-Path $TempRoot "a3s-test-skill.zip"
        $SkillChecksum = Join-Path $TempRoot "a3s-test.skill.sha256"
        Copy-Download "$DownloadBase/a3s-test.skill" $SkillArchive
        Copy-Download "$DownloadBase/a3s-test.skill.sha256" $SkillChecksum
        Assert-Checksum $SkillArchive $SkillChecksum
        Copy-Item -LiteralPath $SkillArchive -Destination $SkillZip
        Expand-Archive -LiteralPath $SkillZip -DestinationPath (Join-Path $TempRoot "skill")

        $SkillSource = Join-Path $TempRoot "skill\a3s-test\SKILL.md"
        if (-not (Test-Path -LiteralPath $SkillSource)) {
            throw "Skill archive is missing a3s-test/SKILL.md"
        }

        if ($SkillDir) {
            Install-SkillAt "custom" $SkillDir
        } elseif ($Agent -eq "auto") {
            foreach ($AgentName in $DetectedAgents) {
                Install-AgentSkill $AgentName
            }
        } elseif ($Agent -eq "all") {
            foreach ($AgentName in @(
                "universal",
                "a3s-code",
                "codex",
                "claude-code",
                "cursor",
                "gemini-cli",
                "github-copilot",
                "opencode",
                "cline",
                "roo",
                "windsurf"
            )) {
                Install-AgentSkill $AgentName
            }
        } else {
            Install-AgentSkill $Agent
        }
    }
} finally {
    if (Test-Path -LiteralPath $TempRoot) {
        Remove-Item -LiteralPath $TempRoot -Recurse -Force
    }
}

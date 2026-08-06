$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent $PSScriptRoot
$TempRoot = Join-Path ([IO.Path]::GetTempPath()) ("a3s-test-installer-test-" + [Guid]::NewGuid())
$Version = "v0.5.1"
$Target = "x86_64-pc-windows-msvc"
$PayloadName = "a3s-test-$Version-$Target"
$ReleaseDir = Join-Path $TempRoot "releases\download\$Version"

try {
    New-Item -ItemType Directory -Path (Join-Path $TempRoot "payload\$PayloadName") -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $TempRoot "skill\a3s-test") -Force | Out-Null
    New-Item -ItemType Directory -Path $ReleaseDir -Force | Out-Null

    Set-Content -LiteralPath (Join-Path $TempRoot "payload\$PayloadName\a3s-test.exe") -Value "fixture"
    $CliArchive = Join-Path $ReleaseDir "$PayloadName.zip"
    Compress-Archive -LiteralPath (Join-Path $TempRoot "payload\$PayloadName") -DestinationPath $CliArchive

    Set-Content -LiteralPath (Join-Path $TempRoot "skill\a3s-test\SKILL.md") -Value "# A3S Test fixture Skill"
    $SkillZip = Join-Path $ReleaseDir "a3s-test-skill.zip"
    Compress-Archive -LiteralPath (Join-Path $TempRoot "skill\a3s-test") -DestinationPath $SkillZip
    $SkillArchive = Join-Path $ReleaseDir "a3s-test.skill"
    Move-Item -LiteralPath $SkillZip -Destination $SkillArchive

    $CliHash = (Get-FileHash -LiteralPath $CliArchive -Algorithm SHA256).Hash.ToLowerInvariant()
    $CliChecksum = Join-Path $ReleaseDir "$PayloadName.sha256"
    Set-Content -LiteralPath $CliChecksum -Value "$CliHash  $PayloadName.zip"
    $SkillHash = (Get-FileHash -LiteralPath $SkillArchive -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -LiteralPath (Join-Path $ReleaseDir "a3s-test.skill.sha256") -Value "$SkillHash  a3s-test.skill"

    $env:A3S_TEST_RELEASES_URL = ([Uri](Join-Path $TempRoot "releases")).AbsoluteUri.TrimEnd("/")
    $env:A3S_TEST_SKIP_PATH_UPDATE = "1"
    $env:A3S_HOME = $null
    $env:CODEX_HOME = Join-Path $TempRoot "codex"
    $env:CLAUDE_CONFIG_DIR = $null
    $env:CURSOR_HOME = $null
    $env:GEMINI_HOME = $null
    $env:COPILOT_HOME = $null
    $env:OPENCODE_CONFIG_DIR = $null
    $env:AGENTS_HOME = $null
    $env:CLINE_HOME = $null
    $env:ROO_HOME = $null
    $env:WINDSURF_HOME = $null
    $env:Path = "$env:SystemRoot\System32;$env:SystemRoot"
    $InstallDir = Join-Path $TempRoot "bin"

    & (Join-Path $Root "scripts\install.ps1") `
        -Version $Version `
        -InstallDir $InstallDir

    if (-not (Test-Path -LiteralPath (Join-Path $InstallDir "a3s-test.exe"))) {
        throw "CLI was not installed"
    }
    if (-not (Test-Path -LiteralPath (Join-Path $env:CODEX_HOME "skills\a3s-test\SKILL.md"))) {
        throw "Codex Skill was not installed"
    }
    if (Test-Path -LiteralPath (Join-Path $TempRoot "a3s\skills\a3s-test")) {
        throw "A3S Code Skill should not have been installed"
    }

    $env:CODEX_HOME = $null
    $env:AGENTS_HOME = Join-Path $TempRoot "auto-universal"
    & (Join-Path $Root "scripts\install.ps1") `
        -Version $Version `
        -SkillOnly
    if (-not (Test-Path -LiteralPath (Join-Path $env:AGENTS_HOME "skills\a3s-test\SKILL.md"))) {
        throw "Automatic installation did not use the universal Skill directory"
    }

    $CliOnlyDir = Join-Path $TempRoot "cli-only-bin"
    & (Join-Path $Root "scripts\install.ps1") `
        -Version $Version `
        -CliOnly `
        -InstallDir $CliOnlyDir
    if (-not (Test-Path -LiteralPath (Join-Path $CliOnlyDir "a3s-test.exe"))) {
        throw "CLI-only installation failed without a detected coding agent"
    }

    $env:A3S_HOME = Join-Path $TempRoot "a3s"
    $env:CODEX_HOME = Join-Path $TempRoot "codex"
    $env:CLAUDE_CONFIG_DIR = Join-Path $TempRoot "claude"
    $env:CURSOR_HOME = Join-Path $TempRoot "cursor"
    $env:GEMINI_HOME = Join-Path $TempRoot "gemini"
    $env:COPILOT_HOME = Join-Path $TempRoot "copilot"
    $env:OPENCODE_CONFIG_DIR = Join-Path $TempRoot "opencode"
    $env:AGENTS_HOME = Join-Path $TempRoot "agents"
    $env:CLINE_HOME = Join-Path $TempRoot "cline"
    $env:ROO_HOME = Join-Path $TempRoot "roo"
    $env:WINDSURF_HOME = Join-Path $TempRoot "windsurf"

    & (Join-Path $Root "scripts\install.ps1") `
        -Version $Version `
        -Agent universal `
        -SkillOnly
    if (-not (Test-Path -LiteralPath (Join-Path $env:AGENTS_HOME "skills\a3s-test\SKILL.md"))) {
        throw "Universal Skill was not installed"
    }

    Set-Content -LiteralPath (Join-Path $env:CODEX_HOME "skills\a3s-test\stale.txt") -Value "stale"
    & (Join-Path $Root "scripts\install.ps1") `
        -Version $Version `
        -Agent all `
        -SkillOnly

    foreach ($SkillsParent in @(
        (Join-Path $env:A3S_HOME "skills"),
        (Join-Path $env:CODEX_HOME "skills"),
        (Join-Path $env:CLAUDE_CONFIG_DIR "skills"),
        (Join-Path $env:CURSOR_HOME "skills"),
        (Join-Path $env:GEMINI_HOME "skills"),
        (Join-Path $env:COPILOT_HOME "skills"),
        (Join-Path $env:OPENCODE_CONFIG_DIR "skills"),
        (Join-Path $env:AGENTS_HOME "skills"),
        (Join-Path $env:CLINE_HOME "skills"),
        (Join-Path $env:ROO_HOME "skills"),
        (Join-Path $env:WINDSURF_HOME "skills")
    )) {
        if (-not (Test-Path -LiteralPath (Join-Path $SkillsParent "a3s-test\SKILL.md"))) {
            throw "Skill was not installed under $SkillsParent"
        }
    }
    if (Test-Path -LiteralPath (Join-Path $env:CODEX_HOME "skills\a3s-test\stale.txt")) {
        throw "Stale Skill contents survived replacement"
    }

    $CustomSkills = Join-Path $TempRoot "custom-skills"
    & (Join-Path $Root "scripts\install.ps1") `
        -Version $Version `
        -Agent codex `
        -SkillDir $CustomSkills `
        -SkillOnly
    if (-not (Test-Path -LiteralPath (Join-Path $CustomSkills "a3s-test\SKILL.md"))) {
        throw "Custom Skill directory was not installed"
    }

    Write-Host "Windows installer test passed"
} finally {
    if (Test-Path -LiteralPath $TempRoot) {
        Remove-Item -LiteralPath $TempRoot -Recurse -Force
    }
}

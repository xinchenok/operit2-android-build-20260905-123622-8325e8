$ErrorActionPreference = 'Stop'
$repoRoot = (Get-Location).Path
$scriptFiles = @(Get-ChildItem client -Filter *.ps1) + @(Get-Item builder/restore_update_signing.ps1)
foreach ($file in $scriptFiles) {
    $tokens = $null; $errors = $null
    $null = [System.Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$errors)
    if ($errors.Count) { throw ($errors | Out-String) }
}
. ./client/Signing.ps1
$bundle = New-OperitSigningBundle
$data = $bundle | ConvertFrom-Json
Write-Output "::add-mask::$($data.password)"
Write-Output "::add-mask::$($data.pfx_base64)"
$temp = Join-Path $env:RUNNER_TEMP ([Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $temp | Out-Null
try {
    $backup = Join-Path $temp 'signing.dpapi.xml'
    ConvertTo-SecureString $bundle -AsPlainText -Force | Export-Clixml $backup
    $restored = [pscredential]::new('key',(Import-Clixml $backup)).GetNetworkCredential().Password
    if ($restored -ne $bundle) { throw 'Windows encrypted signing backup did not round-trip.' }
    $env:OPERIT2_UPDATE_SIGNING = $restored
    Set-Location $temp
    & (Join-Path $repoRoot 'builder/restore_update_signing.ps1')
    if (-not (Test-Path 'tools/release/secrets/operit2-release.keystore')) { throw 'JKS was not produced.' }
    if (-not (Test-Path 'tools/release/secrets/android-signing.properties')) { throw 'Signing properties were not produced.' }
    Write-Host 'PASS: PowerShell parsing, .NET key generation, Windows DPAPI backup, Java PKCS12/JKS import, certificate identity.'
} finally {
    Set-Location $repoRoot
    Remove-Item -LiteralPath $temp -Recurse -Force
    $env:OPERIT2_UPDATE_SIGNING = $null
}

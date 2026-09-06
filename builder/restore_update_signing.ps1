# Only runs on the GitHub Actions runner. Never uploads the keystore as an artifact.
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
if (-not $env:OPERIT2_UPDATE_SIGNING) { throw 'Fixed signing secret is missing.' }
$signing = $env:OPERIT2_UPDATE_SIGNING | ConvertFrom-Json
if ($signing.schema -ne 1) { throw 'Unsupported signing secret format.' }
$env:OPERIT_KEY_PASS = [string]$signing.password
Write-Output "::add-mask::$env:OPERIT_KEY_PASS"
$dir = Join-Path (Get-Location) 'tools/release/secrets'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$pfx = Join-Path $dir 'update-source.p12'
$store = Join-Path $dir 'operit2-release.keystore'
[IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($signing.pfx_base64))
$keytool = Join-Path $env:JAVA_HOME 'bin/keytool.exe'
if (-not (Test-Path -LiteralPath $keytool)) { $keytool = Join-Path $env:JAVA_HOME 'bin/keytool' }
# .NET and Java may choose different PKCS#12 alias names. Discover instead of guessing.
$list = & $keytool -J-Duser.language=en -J-Duser.country=US -list -v -storetype PKCS12 -keystore $pfx -storepass:env OPERIT_KEY_PASS 2>&1
if ($LASTEXITCODE -ne 0) { throw 'Unable to open the supplied PKCS#12 signing key.' }
$aliases = @([regex]::Matches(($list -join "`n"), '(?m)^Alias name: (.+)\r?$'))
if ($aliases.Count -ne 1 -or ($list -join "`n") -notmatch 'PrivateKeyEntry') { throw 'Expected exactly one private signing key.' }
$sourceAlias = $aliases[0].Groups[1].Value.Trim()
if (Test-Path -LiteralPath $store) { Remove-Item -LiteralPath $store -Force }
& $keytool -importkeystore -noprompt -srckeystore $pfx -srcstoretype PKCS12 -srcalias $sourceAlias -srcstorepass:env OPERIT_KEY_PASS -srckeypass:env OPERIT_KEY_PASS -destkeystore $store -deststoretype JKS -destalias operit2-update -deststorepass:env OPERIT_KEY_PASS -destkeypass:env OPERIT_KEY_PASS
if ($LASTEXITCODE -ne 0) { throw 'Fixed signing key import failed.' }
$cert = Join-Path $dir 'update-certificate.der'
& $keytool -exportcert -keystore $store -storepass:env OPERIT_KEY_PASS -alias operit2-update -file $cert
if ($LASTEXITCODE -ne 0) { throw 'Unable to verify the imported certificate.' }
$actual = (Get-FileHash -LiteralPath $cert -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $signing.certificate_sha256.ToLowerInvariant()) { throw 'Imported signing certificate fingerprint mismatch.' }
@(
    'RELEASE_STORE_FILE=operit2-release.keystore'
    "RELEASE_STORE_PASSWORD=$env:OPERIT_KEY_PASS"
    'RELEASE_KEY_ALIAS=operit2-update'
    "RELEASE_KEY_PASSWORD=$env:OPERIT_KEY_PASS"
) | Set-Content -LiteralPath (Join-Path $dir 'android-signing.properties') -Encoding utf8
Remove-Item -LiteralPath $pfx, $cert -Force
$env:OPERIT_KEY_PASS = $null
$env:OPERIT2_UPDATE_SIGNING = $null
Write-Host "Fixed update signing key restored. Public certificate SHA-256: $actual"

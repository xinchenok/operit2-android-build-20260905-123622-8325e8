#Requires -Version 7.2
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$repo = 'xinchenok/operit2-android-build-20260905-123622-8325e8'
$workflow = 'update-android.yml'
$secretName = 'OPERIT2_UPDATE_SIGNING'
$apkName = 'operit2-android-arm64-personal-test.apk'
$lock = $null
. (Join-Path $PSScriptRoot 'Signing.ps1')

function Get-GhJson([string[]]$Arguments) {
    for ($i = 0; $i -lt 4; $i++) {
        $text = (& gh @Arguments 2>&1 | Out-String)
        if ($LASTEXITCODE -eq 0) { return ($text | ConvertFrom-Json -AsHashtable) }
        if ($i -lt 3) { Start-Sleep -Seconds (2 + $i * 2) }
    }
    throw "GitHub 查询失败，未把它判定为构建失败。再次运行会继续查询。`n$text"
}
function Save-Session($Value) {
    $Value | ConvertTo-Json | Set-Content -LiteralPath "$sessionFile.tmp" -Encoding utf8
    Move-Item -LiteralPath "$sessionFile.tmp" -Destination $sessionFile -Force
}
function Set-SigningSecret([string]$Json) {
    # Feed the secret through stdin, never via a command argument or a public file.
    $start = [Diagnostics.ProcessStartInfo]::new((Get-Command gh).Source)
    foreach ($arg in @('secret', 'set', $secretName, '--repo', $repo)) { $start.ArgumentList.Add($arg) }
    $start.UseShellExecute = $false
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [Diagnostics.Process]::Start($start)
    try {
        $output = $process.StandardOutput.ReadToEndAsync()
        $errorOutput = $process.StandardError.ReadToEndAsync()
        $process.StandardInput.Write($Json)
        $process.StandardInput.Close()
        if (-not $process.WaitForExit(120000)) {
            $process.Kill($true)
            throw '签名 Secret 上传未能确认，本机加密备份已经保存；再次运行会检查并复用它。'
        }
        if ($process.ExitCode -ne 0) { throw ('签名 Secret 上传失败：' + $errorOutput.GetAwaiter().GetResult()) }
    } finally { $process.Dispose() }
}

try {
    if (-not $IsWindows) { throw '这个启动器针对你的 Windows 电脑。' }
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw '没有找到已安装的 GitHub CLI。这个脚本不会自动安装软件。' }
    & gh auth status --hostname github.com *> $null
    if ($LASTEXITCODE -ne 0) { throw 'GitHub CLI 尚未登录。先执行 gh auth login，再运行更新。' }
    $homeDir = Join-Path $env:LOCALAPPDATA ('Operit2Updater\' + $repo.Split('/')[1])
    New-Item -ItemType Directory -Force -Path $homeDir | Out-Null
    try { $lock = [IO.File]::Open((Join-Path $homeDir 'updater.lock'), 'OpenOrCreate', 'ReadWrite', 'None') }
    catch { throw '另一个更新启动器还在运行，请使用原来的窗口，避免重复构建。' }
    $sessionFile = Join-Path $homeDir 'update-session.json'
    $keyBackup = Join-Path $homeDir 'signing.dpapi.xml'
    Write-Host "目标仓库：$repo"
    Write-Host '本机不安装开发环境；云端跟随上游默认分支的最新提交。'
    $null = Get-GhJson -Arguments @('api', "repos/$repo/actions/workflows/$workflow")
    $names = @(Get-GhJson -Arguments @('secret','list','--repo',$repo,'--json','name'))
    if ($secretName -notin @($names | ForEach-Object { $_.name })) {
        if (Test-Path -LiteralPath $keyBackup) {
            Write-Host '复用本机保存的固定签名，不生成新密钥。'
            $secure = Import-Clixml -LiteralPath $keyBackup
            $bundle = [pscredential]::new('signing', $secure).GetNetworkCredential().Password
        } else {
            Write-Host '首次初始化：生成固定签名，保存当前 Windows 账号加密备份，并上传到本仓库的 Actions Secret。'
            Write-Host '旧的一次性签名 APK 不能被这套新签名直接覆盖；请先备份手机应用数据。'
            $bundle = New-OperitSigningBundle
            ConvertTo-SecureString -String $bundle -AsPlainText -Force | Export-Clixml -LiteralPath "$keyBackup.tmp"
            Move-Item -LiteralPath "$keyBackup.tmp" -Destination $keyBackup -Force
        }
        $fingerprint = ($bundle | ConvertFrom-Json).certificate_sha256
        Set-SigningSecret -Json $bundle
        $bundle = $null
        Write-Host "固定签名已配置。证书 SHA-256：$fingerprint"
        Write-Host "本机加密备份：$keyBackup（只能由此 Windows 账号解密）"
    } else {
        Write-Host '云端已有固定签名，保持不变。'
    }

    $session = $null
    if (Test-Path -LiteralPath $sessionFile) {
        $saved = Get-Content -LiteralPath $sessionFile -Raw | ConvertFrom-Json -AsHashtable
        if ($saved.repo -eq $repo -and $saved.state -eq 'pending') { $session = $saved }
    }
    if (-not $session) {
        $session = @{repo=$repo; request_id=[Guid]::NewGuid().ToString('N'); state='pending'; run_id=$null}
        Save-Session $session
        & gh workflow run $workflow --repo $repo --ref main -f "request_id=$($session.request_id)"
        if ($LASTEXITCODE -ne 0) { throw '提交结果未能确认。记录已保存，避免自动重复提交；在 Actions 页面核对后再处理。' }
    } else { Write-Host '继续上次更新，不重新提交编译。' }

    if (-not $session.run_id) {
        for ($i=0; $i -lt 30; $i++) {
            $runs = Get-GhJson -Arguments @('api',"repos/$repo/actions/workflows/$workflow/runs?event=workflow_dispatch&per_page=100")
            $matches = @($runs.workflow_runs | Where-Object { $_.display_title -eq "Update Android $($session.request_id)" })
            if ($matches.Count) { $session.run_id = [string]$matches[0].id; Save-Session $session; break }
            Start-Sleep -Seconds 3
        }
        if (-not $session.run_id) { throw '暂未找到本次请求的运行记录；没有重复提交任务。再次运行会继续查找。' }
    }
    $run = $session.run_id
    Write-Host "构建记录：https://github.com/$repo/actions/runs/$run"
    Write-Host '关闭窗口不取消云端构建；下次双击可继续查询和下载。'
    while ($true) {
        $status = Get-GhJson -Arguments @('api',"repos/$repo/actions/runs/$run")
        if ($status.status -eq 'completed') { break }
        Write-Host ("{0}  {1}（此窗口只查询状态）" -f (Get-Date -Format HH:mm:ss), $status.status)
        Start-Sleep -Seconds 30
    }
    if ($status.conclusion -ne 'success') {
        $session.state = 'failed'; Save-Session $session
        $log = Join-Path $homeDir "failed-$run.log"
        & gh run view $run --repo $repo --log-failed 2>&1 | Tee-Object -FilePath $log
        throw "这轮构建结果：$($status.conclusion)。上一版没有替换。失败日志：$log"
    }
    $release = Get-GhJson -Arguments @('api', "repos/$repo/releases/latest")
    $tag = [string]$release.tag_name
    if ($tag -notmatch '^android-[0-9]+-[0-9a-f]+$') { throw '最新版 Release 不是本更新流程生成的版本，未下载。' }
    $out = Join-Path $env:USERPROFILE "Downloads\Operit2-updates\$tag"
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    & gh release download $tag --repo $repo --pattern $apkName --pattern UPDATE.json --pattern SHA256SUMS --pattern signature.txt --dir $out --clobber
    if ($LASTEXITCODE -ne 0) { throw '构建已成功，下载尚未完成。再次运行会继续下载，不重建。' }
    $info = Get-Content -LiteralPath (Join-Path $out 'UPDATE.json') -Raw | ConvertFrom-Json
    $hash = (Get-FileHash -LiteralPath (Join-Path $out $apkName) -Algorithm SHA256).Hash.ToLowerInvariant()
    $checksums = Get-Content -LiteralPath (Join-Path $out 'SHA256SUMS') -Raw
    if ($hash -ne $info.apk_sha256 -or -not $checksums.Contains("$hash  $apkName")) { throw 'APK 下载校验不一致，未标记为完成。' }
    $session.state = 'downloaded'; Save-Session $session
    Write-Host "完成。上游提交：$($info.source_commit)"
    Write-Host "Android versionCode：$($info.version_code)"
    Write-Host "APK：$(Join-Path $out $apkName)"
    Write-Host '把 APK 传到手机并确认安装。本启动器不会自动卸载、安装或操作手机。'
    Invoke-Item $out
} catch {
    Write-Host "`n$($_.Exception.Message)" -ForegroundColor Red
    exit 1
} finally { if ($lock) { $lock.Dispose() } }

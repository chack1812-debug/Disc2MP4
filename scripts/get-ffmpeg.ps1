<#
.SYNOPSIS
  ffmpeg(Windows 用ビルド)をダウンロードして、このフォルダの ffmpeg\ に置く。
.DESCRIPTION
  disc2mp4.exe は、同じフォルダの ffmpeg\ffmpeg.exe を自動で使います。
  ビルド: gyan.dev の Windows 用ビルド(ffmpeg 公式サイトが案内しているもの)。
  ダウンロード元は、① GitHub 上の同じビルド(速いことが多い)→ ② gyan.dev の順に試します。
  どちらも、gyan.dev が公表している SHA-256 と一致したものだけを使います。
  既に ffmpeg がある場合は何もしません。入れ直すときは -Force を付けます。
  ffmpeg は GPL ライセンスの別ソフトです。ライセンスは ffmpeg\LICENSE.txt に保存されます。
#>
[CmdletBinding()]
param([switch]$Force)

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$gyanUrl = 'https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip'
$dest = Join-Path $PSScriptRoot 'ffmpeg'
$ffmpegExe = Join-Path $dest 'ffmpeg.exe'

if ((Test-Path -LiteralPath $ffmpegExe) -and -not $Force) {
    Write-Host "ffmpeg は既に入っています: $ffmpegExe"
    Write-Host '入れ直す場合は、-Force を付けて実行してください。'
    return
}

$tmp = Join-Path ([IO.Path]::GetTempPath()) ('get-ffmpeg-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    $zip = Join-Path $tmp 'ffmpeg.zip'
    $ProgressPreference = 'SilentlyContinue'   # Windows PowerShell 5.1 では進捗表示がダウンロードを極端に遅くする

    # 配布元(gyan.dev)が公表している SHA-256 を先に取得する(これが検証の基準になる)
    Write-Host '検証用のハッシュ値(SHA-256)を取得しています...'
    $shaResponse = (Invoke-WebRequest -Uri "$gyanUrl.sha256" -UseBasicParsing).Content
    if ($shaResponse -is [byte[]]) { $shaResponse = [Text.Encoding]::ASCII.GetString($shaResponse) }
    if ($shaResponse -notmatch '[0-9a-fA-F]{64}') { throw 'ハッシュ値(SHA-256)を取得できませんでした。' }
    $expected = $Matches[0].ToUpperInvariant()

    # ダウンロード元: ① GitHub 上の同じビルド ② gyan.dev
    $sources = @()
    try {
        $rel = Invoke-RestMethod -Uri 'https://api.github.com/repos/GyanD/codexffmpeg/releases/latest'
        $asset = $rel.assets | Where-Object { $_.name -like 'ffmpeg-*-essentials_build.zip' } | Select-Object -First 1
        if ($asset) { $sources += $asset.browser_download_url }
    } catch { }
    $sources += $gyanUrl

    $verified = $false
    foreach ($src in $sources) {
        try {
            Write-Host "ffmpeg をダウンロードしています(約 120 MB。数分かかることがあります): $src"
            Remove-Item -LiteralPath $zip -Force -ErrorAction SilentlyContinue
            Invoke-WebRequest -Uri $src -OutFile $zip -UseBasicParsing
            $actual = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToUpperInvariant()
            if ($actual -eq $expected) { $verified = $true; break }
            Write-Host '  公開されている SHA-256 と一致しませんでした。別のダウンロード元を試します。'
        }
        catch {
            Write-Host "  ダウンロードに失敗しました: $($_.Exception.Message)"
        }
    }
    if (-not $verified) {
        throw 'ffmpeg を取得できませんでした(ダウンロードの失敗、または公開されている SHA-256 との不一致)。しばらくしてからもう一度お試しください。'
    }
    Write-Host '検証に成功しました(SHA-256 一致)。'

    Write-Host '展開しています...'
    $extract = Join-Path $tmp 'x'
    Expand-Archive -LiteralPath $zip -DestinationPath $extract

    $found = Get-ChildItem -LiteralPath $extract -Recurse -Filter 'ffmpeg.exe' | Select-Object -First 1
    if (-not $found) { throw 'ダウンロードしたファイルに ffmpeg.exe が見つかりません。' }
    $bin = $found.Directory.FullName
    $top = $found.Directory.Parent.FullName

    New-Item -ItemType Directory -Path $dest -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $bin 'ffmpeg.exe') -Destination $dest -Force
    Copy-Item -LiteralPath (Join-Path $bin 'ffprobe.exe') -Destination $dest -Force
    foreach ($doc in 'LICENSE', 'README.txt') {
        $p = Join-Path $top $doc
        if (Test-Path -LiteralPath $p) {
            $name = if ($doc -eq 'LICENSE') { 'LICENSE.txt' } else { 'README.txt' }
            Copy-Item -LiteralPath $p -Destination (Join-Path $dest $name) -Force
        }
    }

    $version = (& $ffmpegExe -version | Select-Object -First 1)
    Write-Host ''
    Write-Host "完了しました: $dest"
    Write-Host $version
}
finally {
    Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

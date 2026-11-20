# Disc2MP4

自作(ホームビデオ)の DVD(`VIDEO_TS`)・ブルーレイ(`BDMV`)を、画質を整えた `.mp4`(H.264 + AAC)に変換するコマンドラインツールです。変換エンジンは ffmpeg を使います。

**使い方の詳細は [取扱説明書](docs/MANUAL.md) を参照してください。**

## 前提

- ffmpeg / ffprobe が必要です。配布 zip 内の `get-ffmpeg.bat` をダブルクリックすると、exe の隣の `ffmpeg\` に自動で用意されます(公開ハッシュ値で検証します)。すでに持っている場合は PATH に入れるか、`--ffmpeg <パス>` で指定してください。探す順番は、`--ffmpeg` → exe の隣の `ffmpeg\` → PATH です
- コピーガード(CSS / AACS)のあるディスクは対象外です。ISO ファイルは直接読めないので、Windows でマウントしたドライブ(またはフォルダ)を指定してください

## ビルド

```
cargo build --release
```

`target\release\disc2mp4.exe` ができます。

## 使い方

```
disc2mp4 list    <ディスクのパス>                  # タイトル一覧(番号・長さ・解像度)
disc2mp4 convert <ディスクのパス>                  # 最長のタイトルを mp4 に変換
disc2mp4 convert <ディスクのパス> --all            # 全タイトルを個別の mp4 に変換
disc2mp4 convert <ディスクのパス> --title 2 -o D:\out\movie.mp4
```

`<ディスクのパス>` は `VIDEO_TS` / `BDMV` フォルダ自身でも、それを含むフォルダ(ドライブ直下を含む)でも構いません。

| オプション | 意味 | 既定 |
|---|---|---|
| `-o, --out` | 出力先(`.mp4` ファイル、またはフォルダ) | ディスクのフォルダの隣に `<フォルダ名>.mp4`(ドライブ直下ならカレントに `disc.mp4`) |
| `--title N` | 変換するタイトル番号 | 最長のタイトル |
| `--all` | 全タイトルを `_titleNN.mp4` で個別に変換 | off |
| `--height` | 出力の高さ `720` / `1080` / `keep`(元より小さくなる指定は拡大しない) | 1080 |
| `--deinterlace` | `auto` / `on` / `off`(auto は映像がインターレースのときだけ解除) | auto |
| `--quality` | `standard`(crf 20、速い) / `high`(crf 17、遅い) | high |
| `--force` | 既存の出力を上書きする | off |
| `--ffmpeg` | ffmpeg のパス | exe の隣の `ffmpeg\` → PATH 上の ffmpeg |

10 秒未満のタイトルは(メニューの残りなどとみなして)変換対象から除外されます。

## 画質処理

デインターレース(bwdif)→ 軽いノイズ除去(hqdn3d)→ 縦横比の補正と拡大(Lanczos)→ 軽いシャープ(unsharp)の順に処理します。

## 既知の制限(v1)

- BD は `.m2ts` ファイル単位で扱います(MPLS プレイリストは解析しません)
- メニュー、字幕、複数音声トラックは再現しません(音声は最初の 1 本)
- AI 超解像・GUI は未対応です

詳細は `docs/superpowers/specs/2026-11-20-disc2mp4-design.md` を参照してください。

## 配布版

配布用 zip(`disc2mp4.exe`・`get-ffmpeg.bat`・取扱説明書・`LICENSE`・`THIRD-PARTY-NOTICES.txt`)は `scripts\package.ps1` で作れます(`dist\Disc2MP4_v<版>.zip`)。`THIRD-PARTY-NOTICES.txt` は `python -I scripts/gen_third_party_notices.py` で作り直せます。

## ライセンス

[GNU General Public License v3.0 以降](LICENSE)(GPL-3.0-or-later)。Copyright (C) 2026 宇摩コードワークス(UmaCodeWorks)。無保証です。ffmpeg は同梱しておらず、別途ダウンロードして使う外部ソフトです。

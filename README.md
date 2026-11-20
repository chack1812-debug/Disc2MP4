# Disc2MP4

自作(ホームビデオ)の DVD(`VIDEO_TS`)・ブルーレイ(`BDMV`)を、画質を整えた `.mp4`(H.264 + AAC)に変換するコマンドラインツールです。変換エンジンは ffmpeg を使います。

## 前提

- ffmpeg / ffprobe が PATH 上にあること(または `--ffmpeg <ffmpeg のパス>` で指定。ffprobe は同じフォルダから探します)
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
| `--ffmpeg` | ffmpeg のパス | PATH 上の ffmpeg |

10 秒未満のタイトルは(メニューの残りなどとみなして)変換対象から除外されます。

## 画質処理

デインターレース(bwdif)→ 軽いノイズ除去(hqdn3d)→ 縦横比の補正と拡大(Lanczos)→ 軽いシャープ(unsharp)の順に処理します。

## 既知の制限(v1)

- BD は `.m2ts` ファイル単位で扱います(MPLS プレイリストは解析しません)
- メニュー、字幕、複数音声トラックは再現しません(音声は最初の 1 本)
- AI 超解像・GUI は未対応です

詳細は `docs/superpowers/specs/2026-11-20-disc2mp4-design.md` を参照してください。

use clap::{Parser, Subcommand};
use disc2mp4::commands::{convert, list, ConvertArgs};
use disc2mp4::plan::{Deinterlace, Height, Options, Quality};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "disc2mp4", version, about = "自作 DVD/ブルーレイを画質を整えた mp4 に変換する")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 変換候補のタイトル一覧を表示する
    List {
        /// VIDEO_TS / BDMV フォルダ、またはそれを含むフォルダ
        path: PathBuf,
        #[arg(long, default_value = "ffmpeg")]
        ffmpeg: PathBuf,
    },
    /// mp4 に変換する(既定は最長のタイトル)
    Convert {
        path: PathBuf,
        /// 出力先(.mp4 ファイル、またはフォルダ)
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// 変換するタイトル番号(list で確認)
        #[arg(long)]
        title: Option<usize>,
        /// 全タイトルを個別の mp4 に変換する
        #[arg(long)]
        all: bool,
        /// 出力の高さ: 720 / 1080 / keep(元のまま)
        #[arg(long, default_value = "1080", value_parser = parse_height)]
        height: Height,
        /// インターレース解除: auto / on / off
        #[arg(long, default_value = "auto", value_parser = parse_deinterlace)]
        deinterlace: Deinterlace,
        /// 画質: standard / high
        #[arg(long, default_value = "high", value_parser = parse_quality)]
        quality: Quality,
        /// 既存の出力を上書きする
        #[arg(long)]
        force: bool,
        #[arg(long, default_value = "ffmpeg")]
        ffmpeg: PathBuf,
    },
}

fn parse_height(s: &str) -> Result<Height, String> {
    if s.eq_ignore_ascii_case("keep") {
        return Ok(Height::Keep);
    }
    s.parse::<u32>()
        .ok()
        .filter(|h| *h > 0)
        .map(Height::Px)
        .ok_or_else(|| "720 / 1080 などの数値か keep を指定してください".to_string())
}

fn parse_deinterlace(s: &str) -> Result<Deinterlace, String> {
    match s.to_ascii_lowercase().as_str() {
        "auto" => Ok(Deinterlace::Auto),
        "on" => Ok(Deinterlace::On),
        "off" => Ok(Deinterlace::Off),
        _ => Err("auto / on / off のいずれかを指定してください".into()),
    }
}

fn parse_quality(s: &str) -> Result<Quality, String> {
    match s.to_ascii_lowercase().as_str() {
        "standard" => Ok(Quality::Standard),
        "high" => Ok(Quality::High),
        _ => Err("standard / high のいずれかを指定してください".into()),
    }
}

/// ffprobe is looked up next to a given ffmpeg path, otherwise on PATH.
fn ffprobe_for(ffmpeg: &Path) -> PathBuf {
    match ffmpeg.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join("ffprobe"),
        _ => PathBuf::from("ffprobe"),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::List { path, ffmpeg } => list(&path, &ffprobe_for(&ffmpeg)),
        Command::Convert { path, out, title, all, height, deinterlace, quality, force, ffmpeg } => {
            convert(ConvertArgs {
                path,
                out,
                title,
                all,
                options: Options { height, deinterlace, quality },
                force,
                ffprobe: ffprobe_for(&ffmpeg),
                ffmpeg,
            })
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("エラー: {e:#}");
            ExitCode::FAILURE
        }
    }
}

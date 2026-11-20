use crate::plan::{ffmpeg_args, Options};
use crate::probe::probe;
use crate::run::run_ffmpeg;
use crate::scan::scan;
use crate::{Disc, MediaInfo};
use anyhow::{anyhow, bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Titles shorter than this are skipped by default.
const MIN_TITLE_SECS: f64 = 10.0;

/// Fail early, with a clear message, when ffprobe cannot be launched at all.
pub fn ensure_ffprobe(ffprobe: &Path) -> Result<()> {
    std::process::Command::new(ffprobe)
        .arg("-version")
        .output()
        .map(|_| ())
        .with_context(|| {
            format!(
                "ffprobe を起動できません({})。ffmpeg を PATH に入れるか --ffmpeg でパスを指定してください",
                ffprobe.display()
            )
        })
}

/// Temporary sibling the encoder writes to; renamed to the real output only on success.
pub fn partial_path(output: &Path) -> PathBuf {
    let mut name = output.file_name().unwrap_or_default().to_os_string();
    name.push(".partial");
    output.with_file_name(name)
}

/// Probe a title. Stream properties come from the first file; the duration is the sum of every
/// file's own duration, because ffprobe on a `concat:` input can report only the last segment
/// when the VOBs' timestamps restart.
pub fn probe_title(title: &crate::Title, ffprobe: &Path) -> Result<MediaInfo> {
    let mut info = probe(ffprobe, &title.files[0].to_string_lossy())?;
    if title.files.len() > 1 {
        let mut total = 0.0;
        let mut known = true;
        for f in &title.files {
            match probe(ffprobe, &f.to_string_lossy()).ok().and_then(|i| i.duration_secs) {
                Some(d) => total += d,
                None => known = false,
            }
        }
        info.duration_secs = known.then_some(total);
    }
    Ok(info)
}

/// Pick the titles to convert from `(index, duration_secs, size_bytes)` rows.
pub fn select_titles(
    rows: &[(usize, Option<f64>, u64)],
    title: Option<usize>,
    all: bool,
) -> Result<Vec<usize>> {
    if let Some(n) = title {
        if rows.iter().any(|r| r.0 == n) {
            return Ok(vec![n]);
        }
        bail!("タイトル {n} は存在しません(list で一覧を確認してください)");
    }
    let candidates: Vec<&(usize, Option<f64>, u64)> = rows
        .iter()
        .filter(|r| r.1.map_or(true, |d| d >= MIN_TITLE_SECS))
        .collect();
    if candidates.is_empty() {
        bail!("変換対象のタイトルがありません({MIN_TITLE_SECS} 秒未満のタイトルは除外されます)");
    }
    if all {
        return Ok(candidates.iter().map(|r| r.0).collect());
    }
    let best = if candidates.iter().all(|r| r.1.is_some()) {
        candidates.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
    } else {
        candidates.iter().max_by_key(|r| r.2)
    };
    Ok(vec![best.expect("non-empty").0])
}

/// Output path for a title. The default is `<disc folder name>.mp4` next to the disc folder
/// (the parent of `VIDEO_TS`/`BDMV`); a nameless disc folder (drive root) falls back to `./disc.mp4`.
pub fn output_path(disc_root: &Path, index: usize, multiple: bool, out: Option<&Path>) -> PathBuf {
    let is_disc_dir = disc_root
        .file_name()
        .map(|n| {
            let n = n.to_string_lossy();
            n.eq_ignore_ascii_case("VIDEO_TS") || n.eq_ignore_ascii_case("BDMV")
        })
        .unwrap_or(false);
    let disc_dir = if is_disc_dir {
        disc_root.parent().unwrap_or(Path::new("")).to_path_buf()
    } else {
        disc_root.to_path_buf()
    };
    let (default_dir, stem) = match disc_dir.file_name() {
        Some(name) => (
            disc_dir.parent().unwrap_or(Path::new("")).to_path_buf(),
            name.to_string_lossy().into_owned(),
        ),
        None => (PathBuf::from("."), "disc".to_string()),
    };
    let suffixed = |stem: &str| {
        if multiple {
            format!("{stem}_title{index:02}.mp4")
        } else {
            format!("{stem}.mp4")
        }
    };
    match out {
        Some(o) if o.extension().map_or(false, |e| e.to_string_lossy().eq_ignore_ascii_case("mp4")) => {
            let o_stem = o.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            o.with_file_name(suffixed(&o_stem))
        }
        Some(dir) => dir.join(suffixed(&stem)),
        None => default_dir.join(suffixed(&stem)),
    }
}

pub fn format_duration(secs: Option<f64>) -> String {
    match secs {
        Some(s) => {
            let total = s.round() as u64;
            format!("{}:{:02}", total / 60, total % 60)
        }
        None => "?".to_string(),
    }
}

fn probe_titles(disc: &Disc, ffprobe: &Path) -> Vec<Result<MediaInfo>> {
    disc.titles.iter().map(|t| probe_title(t, ffprobe)).collect()
}

pub fn list(path: &Path, ffprobe: &Path) -> Result<()> {
    ensure_ffprobe(ffprobe)?;
    let disc = scan(path)?;
    println!("{:?}: {}", disc.kind, disc.root.display());
    let infos = probe_titles(&disc, ffprobe);
    for (t, info) in disc.titles.iter().zip(&infos) {
        let info = info.as_ref().ok();
        let dur = info.and_then(|i| i.duration_secs);
        let res = info
            .map_or("?".to_string(), |i| format!("{}x{}", i.width, i.height));
        let note = if dur.map_or(false, |d| d < MIN_TITLE_SECS) {
            "  (短いため既定では除外)"
        } else {
            ""
        };
        println!(
            "  [{}] {:<14} {:>8}  {}  {} ファイル{}",
            t.index,
            t.label,
            format_duration(dur),
            res,
            t.files.len(),
            note
        );
    }
    Ok(())
}

pub struct ConvertArgs {
    pub path: PathBuf,
    pub out: Option<PathBuf>,
    pub title: Option<usize>,
    pub all: bool,
    pub options: Options,
    pub force: bool,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

pub fn convert(args: ConvertArgs) -> Result<()> {
    ensure_ffprobe(&args.ffprobe)?;
    let disc = scan(&args.path)?;
    let infos = probe_titles(&disc, &args.ffprobe);
    let rows: Vec<(usize, Option<f64>, u64)> = disc
        .titles
        .iter()
        .zip(&infos)
        .map(|(t, i)| (t.index, i.as_ref().ok().and_then(|i| i.duration_secs), t.size_bytes))
        .collect();
    let selected = select_titles(&rows, args.title, args.all)?;
    let multiple = selected.len() > 1;

    let jobs: Vec<(usize, PathBuf)> = selected
        .iter()
        .map(|&idx| (idx, output_path(&disc.root, idx, multiple, args.out.as_deref())))
        .collect();
    if !args.force {
        if let Some((_, existing)) = jobs.iter().find(|(_, p)| p.exists()) {
            bail!(
                "出力先が既に存在します: {}(上書きするには --force を指定してください)",
                existing.display()
            );
        }
    }

    for (n, (idx, output)) in jobs.iter().enumerate() {
        let pos = disc.titles.iter().position(|t| t.index == *idx).expect("selected exists");
        let title = &disc.titles[pos];
        let info = infos[pos].as_ref().map_err(|e| {
            anyhow!("{} の映像情報を取得できません(ファイルが壊れている可能性があります): {e:#}", title.label)
        })?;
        if let Some(dir) = output.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("出力フォルダを作れません: {}", dir.display()))?;
        }
        println!("[{}/{}] {} -> {}", n + 1, jobs.len(), title.label, output.display());
        // Encode into a temporary file and move it into place only on success, so a failure or
        // an interruption never destroys an existing output nor leaves a half-written .mp4.
        let partial = partial_path(output);
        let _ = std::fs::remove_file(&partial);
        let ff_args = ffmpeg_args(&title.input_arg(), &partial, info, &args.options);
        let mut on_progress = |p: f64| {
            print!("\r  {:>3}%", (p * 100.0) as u32);
            let _ = std::io::stdout().flush();
        };
        let result = run_ffmpeg(&args.ffmpeg, &ff_args, info.duration_secs, &mut on_progress);
        println!();
        match result {
            Ok(()) => std::fs::rename(&partial, output)
                .with_context(|| format!("出力ファイルを作れません: {}", output.display()))?,
            Err(e) => {
                let _ = std::fs::remove_file(&partial);
                return Err(e);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn main_title_is_the_longest() {
        let t = [(1, Some(60.0), 10), (2, Some(300.0), 5)];
        assert_eq!(select_titles(&t, None, false).unwrap(), vec![2]);
    }

    #[test]
    fn falls_back_to_size_when_duration_unknown() {
        let t = [(1, None, 10), (2, None, 50)];
        assert_eq!(select_titles(&t, None, false).unwrap(), vec![2]);
    }

    #[test]
    fn short_titles_excluded_even_with_all() {
        let t = [(1, Some(5.0), 1), (2, Some(120.0), 1), (3, Some(30.0), 1)];
        assert_eq!(select_titles(&t, None, true).unwrap(), vec![2, 3]);
    }

    #[test]
    fn errors_when_nothing_convertible() {
        let short = [(1, Some(5.0), 1)];
        assert!(select_titles(&short, None, false).is_err());
        let ok = [(1, Some(100.0), 1)];
        assert!(select_titles(&ok, Some(9), false).is_err());
        assert_eq!(select_titles(&ok, Some(1), false).unwrap(), vec![1]);
    }

    #[test]
    fn output_path_defaults_next_to_disc_and_suffixes_multiple() {
        let root = Path::new("base/X/VIDEO_TS");
        assert_eq!(output_path(root, 1, false, None), Path::new("base/X.mp4"));
        assert_eq!(output_path(root, 2, true, None), Path::new("base/X_title02.mp4"));
        assert_eq!(
            output_path(Path::new("base/X/BDMV"), 1, false, None),
            Path::new("base/X.mp4")
        );
    }

    #[test]
    fn output_path_honours_explicit_out() {
        let root = Path::new("base/X/VIDEO_TS");
        assert_eq!(
            output_path(root, 1, false, Some(Path::new("o/movie.mp4"))),
            Path::new("o/movie.mp4")
        );
        assert_eq!(
            output_path(root, 2, true, Some(Path::new("o/movie.mp4"))),
            Path::new("o/movie_title02.mp4")
        );
        assert_eq!(
            output_path(root, 1, false, Some(Path::new("outdir"))),
            Path::new("outdir/X.mp4")
        );
    }

    #[test]
    fn format_duration_is_mm_ss() {
        assert_eq!(format_duration(Some(125.4)), "2:05");
        assert_eq!(format_duration(None), "?");
    }

    #[test]
    fn ensure_ffprobe_errors_when_missing() {
        let msg = ensure_ffprobe(Path::new("Z:/nope/ffprobe")).unwrap_err().to_string();
        assert!(msg.contains("ffprobe"), "{msg}");
    }

    #[test]
    fn partial_path_is_a_hidden_sibling_that_cannot_be_mistaken_for_output() {
        assert_eq!(partial_path(Path::new("o/a.mp4")), Path::new("o/a.mp4.partial"));
    }
}

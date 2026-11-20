use disc2mp4::commands::{convert, list, ConvertArgs};
use disc2mp4::plan::Options;
use std::path::{Path, PathBuf};
use std::process::Command;

fn have_ffmpeg() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
        && Command::new("ffprobe").arg("-version").output().is_ok()
}

/// Generate an 11 s interlaced-looking 720x480 MPEG-2 program stream with audio.
fn make_vob(path: &Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let st = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=720x480:rate=30000/1001:duration=11"])
        .args(["-f", "lavfi", "-i", "sine=duration=11", "-c:v", "mpeg2video", "-c:a", "mp2", "-f", "vob"])
        .arg(path)
        .status()
        .unwrap();
    assert!(st.success());
}

fn args(path: &Path, out: Option<PathBuf>, force: bool) -> ConvertArgs {
    ConvertArgs {
        path: path.to_path_buf(),
        out,
        title: None,
        all: false,
        options: Options::default(),
        force,
        ffmpeg: PathBuf::from("ffmpeg"),
        ffprobe: PathBuf::from("ffprobe"),
    }
}

fn probe_height_and_duration(mp4: &Path) -> (u64, f64) {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=height:format=duration", "-of", "csv=p=0"])
        .arg(mp4)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let nums: Vec<&str> = text.split(|c| c == ',' || c == '\n').filter(|s| !s.trim().is_empty()).collect();
    (nums[0].trim().parse().unwrap(), nums[1].trim().parse().unwrap())
}

#[test]
fn dvd_to_mp4_end_to_end() {
    if !have_ffmpeg() { eprintln!("skip: ffmpeg not found"); return; }
    let t = tempfile::tempdir().unwrap();
    let disc = t.path().join("日本語 テスト");
    make_vob(&disc.join("VIDEO_TS/VTS_01_1.VOB"));
    make_vob(&disc.join("VIDEO_TS/VTS_01_2.VOB"));
    let out = t.path().join("out").join("movie.mp4");
    convert(args(&disc, Some(out.clone()), false)).unwrap();
    let (height, dur) = probe_height_and_duration(&out);
    assert_eq!(height, 720);
    assert!((21.5..=22.5).contains(&dur), "duration {dur}");
}

#[test]
fn refuses_to_overwrite_without_force() {
    if !have_ffmpeg() { eprintln!("skip: ffmpeg not found"); return; }
    let t = tempfile::tempdir().unwrap();
    let disc = t.path().join("disc");
    make_vob(&disc.join("VIDEO_TS/VTS_01_1.VOB"));
    let out = t.path().join("a.mp4");
    convert(args(&disc, Some(out.clone()), false)).unwrap();
    assert!(convert(args(&disc, Some(out.clone()), false)).is_err());
    convert(args(&disc, Some(out), true)).unwrap();
}

#[test]
fn failed_conversion_leaves_no_output() {
    if !have_ffmpeg() { eprintln!("skip: ffmpeg not found"); return; }
    let t = tempfile::tempdir().unwrap();
    let disc = t.path().join("bad");
    std::fs::create_dir_all(disc.join("VIDEO_TS")).unwrap();
    std::fs::write(disc.join("VIDEO_TS/VTS_01_1.VOB"), b"not a video at all").unwrap();
    let out = t.path().join("bad.mp4");
    assert!(convert(args(&disc, Some(out.clone()), false)).is_err());
    assert!(!out.exists());
}

#[test]
fn list_reports_titles() {
    if !have_ffmpeg() { eprintln!("skip: ffmpeg not found"); return; }
    let t = tempfile::tempdir().unwrap();
    let disc = t.path().join("disc");
    make_vob(&disc.join("VIDEO_TS/VTS_01_1.VOB"));
    list(&disc, Path::new("ffprobe")).unwrap();
}

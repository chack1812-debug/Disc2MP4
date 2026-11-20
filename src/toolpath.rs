use std::path::{Path, PathBuf};

/// Decide which ffmpeg to run: an explicit `--ffmpeg` wins; otherwise a copy placed next to the
/// executable (`ffmpeg\ffmpeg.exe`, as installed by get-ffmpeg.ps1, or `ffmpeg.exe`); otherwise
/// whatever `ffmpeg` resolves to on PATH.
pub fn resolve_ffmpeg(explicit: Option<PathBuf>, exe_dir: Option<&Path>) -> PathBuf {
    if let Some(p) = explicit {
        return p;
    }
    if let Some(dir) = exe_dir {
        for candidate in [dir.join("ffmpeg").join("ffmpeg.exe"), dir.join("ffmpeg.exe")] {
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("ffmpeg")
}

/// ffprobe is looked up next to the given ffmpeg path, otherwise on PATH.
pub fn ffprobe_for(ffmpeg: &Path) -> PathBuf {
    match ffmpeg.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join("ffprobe"),
        _ => PathBuf::from("ffprobe"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    #[test]
    fn explicit_path_always_wins() {
        let t = tempfile::tempdir().unwrap();
        touch(&t.path().join("ffmpeg").join("ffmpeg.exe"));
        let got = resolve_ffmpeg(Some(PathBuf::from("X:/my/ffmpeg.exe")), Some(t.path()));
        assert_eq!(got, PathBuf::from("X:/my/ffmpeg.exe"));
    }

    #[test]
    fn finds_ffmpeg_in_ffmpeg_folder_next_to_exe() {
        let t = tempfile::tempdir().unwrap();
        let want = t.path().join("ffmpeg").join("ffmpeg.exe");
        touch(&want);
        assert_eq!(resolve_ffmpeg(None, Some(t.path())), want);
    }

    #[test]
    fn finds_ffmpeg_directly_next_to_exe() {
        let t = tempfile::tempdir().unwrap();
        let want = t.path().join("ffmpeg.exe");
        touch(&want);
        assert_eq!(resolve_ffmpeg(None, Some(t.path())), want);
    }

    #[test]
    fn falls_back_to_path_lookup() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(resolve_ffmpeg(None, Some(t.path())), PathBuf::from("ffmpeg"));
        assert_eq!(resolve_ffmpeg(None, None), PathBuf::from("ffmpeg"));
    }

    #[test]
    fn ffprobe_sits_next_to_ffmpeg_or_on_path() {
        assert_eq!(
            ffprobe_for(Path::new("C:/tools/ffmpeg/ffmpeg.exe")),
            PathBuf::from("C:/tools/ffmpeg/ffprobe")
        );
        assert_eq!(ffprobe_for(Path::new("ffmpeg")), PathBuf::from("ffprobe"));
    }
}

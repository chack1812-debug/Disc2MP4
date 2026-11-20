use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscKind {
    Dvd,
    BluRay,
}

/// One convertible unit of a disc (a DVD title set or a Blu-ray m2ts file).
#[derive(Debug, Clone)]
pub struct Title {
    /// 1-based
    pub index: usize,
    pub label: String,
    pub files: Vec<PathBuf>,
    pub size_bytes: u64,
}

impl Title {
    /// ffmpeg `-i` argument: the plain path for one file, `concat:a|b|c` for several.
    pub fn input_arg(&self) -> String {
        let parts: Vec<String> = self
            .files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        if parts.len() == 1 {
            parts.into_iter().next().unwrap()
        } else {
            format!("concat:{}", parts.join("|"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Disc {
    pub kind: DiscKind,
    /// The `VIDEO_TS` / `BDMV` folder itself.
    pub root: PathBuf,
    pub titles: Vec<Title>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub duration_secs: Option<f64>,
    pub width: u32,
    pub height: u32,
    /// Sample aspect ratio; (1, 1) when unknown.
    pub sar: (u32, u32),
    pub interlaced: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn title(files: &[&str]) -> Title {
        Title {
            index: 1,
            label: "VTS_01".into(),
            files: files.iter().map(PathBuf::from).collect(),
            size_bytes: 0,
        }
    }

    #[test]
    fn input_arg_single_file_is_plain_path() {
        assert_eq!(title(&["a.vob"]).input_arg(), "a.vob");
    }

    #[test]
    fn input_arg_multiple_files_uses_concat() {
        assert_eq!(title(&["a.vob", "b.vob"]).input_arg(), "concat:a.vob|b.vob");
    }
}

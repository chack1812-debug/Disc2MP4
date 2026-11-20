use crate::{Disc, DiscKind, Title};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Find `name` (case-insensitive) as `path` itself or as a direct child directory of `path`.
fn find_dir(path: &Path, name: &str) -> Option<PathBuf> {
    let is_named = |p: &Path| {
        p.file_name()
            .map(|n| n.to_string_lossy().eq_ignore_ascii_case(name))
            .unwrap_or(false)
    };
    if path.is_dir() && is_named(path) {
        return Some(path.to_path_buf());
    }
    fs::read_dir(path)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_dir() && is_named(p))
}

/// Parse `VTS_nn_k.VOB` (case-insensitive) into (nn, k).
fn parse_vts(file_name: &str) -> Option<(u32, u32)> {
    let lower = file_name.to_ascii_lowercase();
    let stem = lower.strip_prefix("vts_")?.strip_suffix(".vob")?;
    let (nn, k) = stem.split_once('_')?;
    Some((nn.parse().ok()?, k.parse().ok()?))
}

fn file_size(p: &Path) -> u64 {
    fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn scan_dvd(root: PathBuf) -> Result<Disc> {
    let mut sets: BTreeMap<u32, Vec<(u32, PathBuf)>> = BTreeMap::new();
    for entry in fs::read_dir(&root).with_context(|| format!("{} を読めません", root.display()))? {
        let path = entry?.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        // k == 0 is the menu of the title set
        if let Some((nn, k)) = parse_vts(&name).filter(|&(_, k)| k >= 1) {
            sets.entry(nn).or_default().push((k, path));
        }
    }
    let titles: Vec<Title> = sets
        .into_iter()
        .enumerate()
        .map(|(i, (nn, mut parts))| {
            parts.sort_by_key(|(k, _)| *k);
            let files: Vec<PathBuf> = parts.into_iter().map(|(_, p)| p).collect();
            Title {
                index: i + 1,
                label: format!("VTS_{nn:02}"),
                size_bytes: files.iter().map(|f| file_size(f)).sum(),
                files,
            }
        })
        .collect();
    finish(DiscKind::Dvd, root, titles)
}

fn scan_bluray(root: PathBuf) -> Result<Disc> {
    let stream = find_dir(&root, "STREAM")
        .with_context(|| format!("{} に STREAM フォルダがありません", root.display()))?;
    let mut files: Vec<PathBuf> = fs::read_dir(&stream)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|e| e.to_string_lossy().eq_ignore_ascii_case("m2ts"))
                .unwrap_or(false)
        })
        .collect();
    files.sort_by_key(|p| p.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase());
    let titles = files
        .into_iter()
        .enumerate()
        .map(|(i, f)| Title {
            index: i + 1,
            label: f.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            size_bytes: file_size(&f),
            files: vec![f],
        })
        .collect();
    finish(DiscKind::BluRay, root, titles)
}

fn finish(kind: DiscKind, root: PathBuf, titles: Vec<Title>) -> Result<Disc> {
    if titles.is_empty() {
        bail!("{} に変換できる映像ファイルがありません", root.display());
    }
    Ok(Disc { kind, root, titles })
}

/// Scan a `VIDEO_TS`/`BDMV` folder, or a folder containing one, into a list of titles.
/// If both exist, the DVD is used.
pub fn scan(path: &Path) -> Result<Disc> {
    if let Some(dir) = find_dir(path, "VIDEO_TS") {
        return scan_dvd(dir);
    }
    if let Some(dir) = find_dir(path, "BDMV") {
        return scan_bluray(dir);
    }
    bail!(
        "VIDEO_TS も BDMV も見つかりません: {}(ディスクのフォルダを指定してください)",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, b"x").unwrap();
    }

    fn names(files: &[std::path::PathBuf]) -> Vec<String> {
        files
            .iter()
            .map(|f| f.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn dvd_groups_vobs_by_title_set_and_skips_menus() {
        let t = tempfile::tempdir().unwrap();
        for f in [
            "VIDEO_TS.VOB",
            "VTS_01_0.VOB",
            "VTS_01_2.VOB",
            "VTS_01_1.VOB",
            "VTS_02_1.VOB",
        ] {
            touch(t.path(), &format!("VIDEO_TS/{f}"));
        }
        let disc = scan(t.path()).unwrap();
        assert_eq!(disc.kind, DiscKind::Dvd);
        assert_eq!(disc.titles.len(), 2);
        assert_eq!(disc.titles[0].index, 1);
        assert_eq!(disc.titles[0].label, "VTS_01");
        assert_eq!(names(&disc.titles[0].files), ["VTS_01_1.VOB", "VTS_01_2.VOB"]);
        assert_eq!(disc.titles[0].size_bytes, 2);
        assert_eq!(disc.titles[1].label, "VTS_02");
    }

    #[test]
    fn bluray_lists_m2ts_in_name_order() {
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), "BDMV/STREAM/00001.m2ts");
        touch(t.path(), "BDMV/STREAM/00000.m2ts");
        let disc = scan(t.path()).unwrap();
        assert_eq!(disc.kind, DiscKind::BluRay);
        let labels: Vec<_> = disc.titles.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(labels, ["00000.m2ts", "00001.m2ts"]);
    }

    #[test]
    fn accepts_parent_folder_and_the_folder_itself() {
        let t = tempfile::tempdir().unwrap();
        touch(t.path(), "VIDEO_TS/VTS_01_1.VOB");
        let a = scan(t.path()).unwrap();
        let b = scan(&t.path().join("VIDEO_TS")).unwrap();
        assert_eq!(a.kind, DiscKind::Dvd);
        assert_eq!(b.kind, DiscKind::Dvd);
        assert_eq!(a.titles.len(), b.titles.len());
    }

    #[test]
    fn accepts_japanese_and_space_in_path() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("日本語 フォルダ");
        touch(&root, "VIDEO_TS/VTS_01_1.VOB");
        assert_eq!(scan(&root).unwrap().titles.len(), 1);
    }

    #[test]
    fn errors_when_neither_found() {
        let t = tempfile::tempdir().unwrap();
        let msg = scan(t.path()).unwrap_err().to_string();
        assert!(msg.contains("VIDEO_TS") && msg.contains("BDMV"), "{msg}");
    }
}

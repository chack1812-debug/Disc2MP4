use crate::MediaInfo;
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

fn parse_secs(v: Option<&Value>) -> Option<f64> {
    v?.as_str()?.parse::<f64>().ok().filter(|d| d.is_finite() && *d >= 0.0)
}

fn parse_sar(v: Option<&Value>) -> (u32, u32) {
    let parsed = v.and_then(Value::as_str).and_then(|s| {
        let (a, b) = s.split_once(':')?;
        Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))
    });
    match parsed {
        Some((a, b)) if a > 0 && b > 0 => (a, b),
        _ => (1, 1),
    }
}

/// Extract the first video stream's properties from `ffprobe -print_format json` output.
pub fn parse_probe(json: &str) -> Result<MediaInfo> {
    let root: Value = serde_json::from_str(json).context("ffprobe の出力を解釈できません")?;
    let stream = root["streams"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["codec_type"] == "video"))
        .ok_or_else(|| anyhow!("映像ストリームが見つかりません"))?;
    let dim = |k: &str| stream[k].as_u64().unwrap_or(0) as u32;
    let interlaced = matches!(
        stream["field_order"].as_str(),
        Some("tt" | "bb" | "tb" | "bt")
    );
    Ok(MediaInfo {
        duration_secs: parse_secs(stream.get("duration")).or_else(|| parse_secs(root["format"].get("duration"))),
        width: dim("width"),
        height: dim("height"),
        sar: parse_sar(stream.get("sample_aspect_ratio")),
        interlaced,
    })
}

/// Run ffprobe on `input` (a path or an ffmpeg `concat:` string) and parse the result.
pub fn probe(ffprobe: &Path, input: &str) -> Result<MediaInfo> {
    let out = Command::new(ffprobe)
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", input])
        .output()
        .with_context(|| {
            format!(
                "ffprobe を起動できません({})。ffmpeg を PATH に入れるか --ffmpeg でパスを指定してください",
                ffprobe.display()
            )
        })?;
    if !out.status.success() {
        bail!(
            "ffprobe が失敗しました: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    parse_probe(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_interlaced_dvd_stream() {
        let json = r#"{"streams":[
            {"codec_type":"audio"},
            {"codec_type":"video","width":720,"height":480,
             "sample_aspect_ratio":"32:27","field_order":"tt"}],
          "format":{"duration":"62.5"}}"#;
        let info = parse_probe(json).unwrap();
        assert_eq!(
            info,
            MediaInfo {
                duration_secs: Some(62.5),
                width: 720,
                height: 480,
                sar: (32, 27),
                interlaced: true
            }
        );
    }

    #[test]
    fn progressive_and_missing_fields() {
        let json = r#"{"streams":[{"codec_type":"video","width":1280,"height":720,
            "sample_aspect_ratio":"0:1","field_order":"progressive","duration":"N/A"}],
          "format":{"duration":"N/A"}}"#;
        let info = parse_probe(json).unwrap();
        assert!(!info.interlaced);
        assert_eq!(info.duration_secs, None);
        assert_eq!(info.sar, (1, 1));
    }

    #[test]
    fn errors_without_video_stream() {
        let json = r#"{"streams":[{"codec_type":"audio"}],"format":{}}"#;
        assert!(parse_probe(json).is_err());
    }
}

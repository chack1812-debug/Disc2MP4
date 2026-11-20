use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};

/// Parse one `-progress` line; returns the output time in seconds for `out_time_us=<n>`.
pub fn parse_progress_line(line: &str) -> Option<f64> {
    let us: i64 = line.trim().strip_prefix("out_time_us=")?.parse().ok()?;
    Some(us.max(0) as f64 / 1_000_000.0)
}

/// Run ffmpeg, reporting progress (0.0-1.0) when the total duration is known.
pub fn run_ffmpeg(
    ffmpeg: &Path,
    args: &[String],
    duration_secs: Option<f64>,
    on_progress: &mut dyn FnMut(f64),
) -> Result<()> {
    let mut child = Command::new(ffmpeg)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "ffmpeg を起動できません({})。PATH に入れるか --ffmpeg でパスを指定してください",
                ffmpeg.display()
            )
        })?;

    let mut stderr = child.stderr.take().expect("stderr piped");
    let stderr_thread = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });

    let stdout = child.stdout.take().expect("stdout piped");
    for line in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
        if let (Some(t), Some(d)) = (parse_progress_line(&line), duration_secs) {
            if d > 0.0 {
                on_progress((t / d).min(1.0));
            }
        }
    }

    let status = child.wait()?;
    let err_text = stderr_thread.join().unwrap_or_default();
    if !status.success() {
        let tail: Vec<&str> = err_text.lines().rev().take(5).collect();
        let tail = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
        bail!("ffmpeg が失敗しました(終了コード {:?}):\n{}", status.code(), tail);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_progress_line_reads_out_time_us() {
        assert_eq!(parse_progress_line("out_time_us=1500000"), Some(1.5));
    }

    #[test]
    fn parse_progress_line_ignores_others() {
        assert_eq!(parse_progress_line("progress=continue"), None);
        assert_eq!(parse_progress_line("out_time_us=N/A"), None);
    }
}

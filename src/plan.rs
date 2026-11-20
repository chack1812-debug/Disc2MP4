use crate::MediaInfo;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deinterlace {
    Auto,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Standard,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Height {
    Keep,
    Px(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub height: Height,
    pub deinterlace: Deinterlace,
    pub quality: Quality,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            height: Height::Px(1080),
            deinterlace: Deinterlace::Auto,
            quality: Quality::High,
        }
    }
}

/// Output size after correcting the sample aspect ratio, optionally scaled to `target_height`.
/// The width is derived from the display aspect ratio in one step and rounded to an even number,
/// so 16:9 DVD material becomes exactly 1920x1080.
pub fn scaled_size(info: &MediaInfo, target_height: Option<u32>) -> (u32, u32) {
    let h = target_height.unwrap_or(info.height);
    let width = h as f64 * info.width as f64 * info.sar.0 as f64
        / (info.height as f64 * info.sar.1 as f64);
    (((width / 2.0).round() as u32) * 2, h)
}

/// Filter chain: deinterlace -> denoise -> square pixels / upscale -> sharpen.
pub fn video_filter(info: &MediaInfo, opts: &Options) -> String {
    let mut f: Vec<String> = Vec::new();

    let deinterlace = match opts.deinterlace {
        Deinterlace::Auto => info.interlaced,
        Deinterlace::On => true,
        Deinterlace::Off => false,
    };
    if deinterlace {
        f.push("bwdif=mode=send_frame".into());
    }

    f.push("hqdn3d=2:1.5:3:2.5".into());

    let upscale_to = match opts.height {
        Height::Px(target) if info.height < target => Some(target),
        _ => None,
    };
    let sar_needs_fix = info.sar != (1, 1);
    if info.height > 0 && info.width > 0 && (upscale_to.is_some() || sar_needs_fix) {
        let (w, h) = scaled_size(info, upscale_to);
        let mut scale = format!("scale={w}:{h}:flags=lanczos");
        if sar_needs_fix {
            scale.push_str(",setsar=1");
        }
        f.push(scale);
    }

    f.push("unsharp=5:5:0.5".into());
    f.join(",")
}

pub fn ffmpeg_args(input: &str, output: &Path, info: &MediaInfo, opts: &Options) -> Vec<String> {
    let (crf, preset) = match opts.quality {
        Quality::Standard => ("20", "medium"),
        Quality::High => ("17", "slow"),
    };
    let mut a: Vec<String> = [
        "-hide_banner", "-nostats", "-progress", "pipe:1", "-y", "-i",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push(input.to_string());
    a.extend(
        [
            "-map", "0:v:0", "-map", "0:a:0?", "-vf",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    a.push(video_filter(info, opts));
    a.extend(
        [
            "-c:v", "libx264", "-crf", crf, "-preset", preset, "-pix_fmt", "yuv420p", "-c:a",
            "aac", "-b:a", "192k", "-movflags", "+faststart", "-f", "mp4",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    a.push(output.to_string_lossy().into_owned());
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn dvd(interlaced: bool) -> MediaInfo {
        MediaInfo {
            duration_secs: Some(60.0),
            width: 720,
            height: 480,
            sar: (32, 27),
            interlaced,
        }
    }

    fn pos(haystack: &str, needle: &str) -> usize {
        haystack
            .find(needle)
            .unwrap_or_else(|| panic!("{needle:?} not in {haystack:?}"))
    }

    #[test]
    fn interlaced_auto_adds_bwdif_first() {
        let f = video_filter(&dvd(true), &Options::default());
        assert!(f.starts_with("bwdif=mode=send_frame"), "{f}");
    }

    #[test]
    fn progressive_auto_has_no_deinterlace() {
        let f = video_filter(&dvd(false), &Options::default());
        assert!(!f.contains("bwdif"), "{f}");
    }

    #[test]
    fn on_forces_and_off_suppresses() {
        let on = Options { deinterlace: Deinterlace::On, ..Options::default() };
        let off = Options { deinterlace: Deinterlace::Off, ..Options::default() };
        assert!(video_filter(&dvd(false), &on).contains("bwdif"));
        assert!(!video_filter(&dvd(true), &off).contains("bwdif"));
    }

    #[test]
    fn order_is_deinterlace_denoise_scale_sharpen() {
        let f = video_filter(&dvd(true), &Options::default());
        let (b, h, s, u) = (
            pos(&f, "bwdif"),
            pos(&f, "hqdn3d=2:1.5:3:2.5"),
            pos(&f, "scale=1920:1080"),
            pos(&f, "unsharp=5:5:0.5"),
        );
        assert!(b < h && h < s && s < u, "{f}");
    }

    #[test]
    fn no_upscale_when_already_tall_enough() {
        let info = MediaInfo { width: 1920, height: 1080, sar: (1, 1), ..dvd(false) };
        let f = video_filter(&info, &Options::default());
        assert!(!f.contains("scale="), "{f}");
    }

    #[test]
    fn unit_sar_skips_sar_correction() {
        let info = MediaInfo { sar: (1, 1), ..dvd(false) };
        assert!(!video_filter(&info, &Options::default()).contains("setsar=1"));
        assert!(video_filter(&dvd(false), &Options::default()).contains("setsar=1"));
    }

    #[test]
    fn keep_height_skips_scaling() {
        let opts = Options { height: Height::Keep, ..Options::default() };
        let square = MediaInfo { sar: (1, 1), ..dvd(false) };
        assert!(!video_filter(&square, &opts).contains("scale="));
    }

    #[test]
    fn quality_maps_to_crf_and_preset() {
        let has = |args: &[String], k: &str, v: &str| {
            args.windows(2).any(|w| w[0] == k && w[1] == v)
        };
        let standard = Options { quality: Quality::Standard, ..Options::default() };
        let std_args = ffmpeg_args("in.vob", Path::new("o.mp4"), &dvd(false), &standard);
        assert!(has(&std_args, "-crf", "20") && has(&std_args, "-preset", "medium"));
        let high = Options { quality: Quality::High, ..Options::default() };
        let high_args = ffmpeg_args("in.vob", Path::new("o.mp4"), &dvd(false), &high);
        assert!(has(&high_args, "-crf", "17") && has(&high_args, "-preset", "slow"));
    }

    #[test]
    fn args_end_with_output_and_map_audio_optional() {
        let args = ffmpeg_args("in.vob", Path::new("o.mp4"), &dvd(false), &Options::default());
        assert_eq!(args.last().unwrap(), "o.mp4");
        assert!(args.iter().any(|a| a == "0:a:0?"));
    }

    #[test]
    fn writes_mp4_container_explicitly() {
        let args = ffmpeg_args("in.vob", Path::new("o.mp4.partial"), &dvd(false), &Options::default());
        assert!(args.windows(2).any(|w| w[0] == "-f" && w[1] == "mp4"));
    }

    #[test]
    fn defaults_are_1080p_high_quality() {
        let d = Options::default();
        assert_eq!((d.height, d.quality, d.deinterlace), (Height::Px(1080), Quality::High, Deinterlace::Auto));
    }

    #[test]
    fn scaled_size_gives_exact_even_widths() {
        // 16:9 DVD (SAR 32:27) -> exactly 1920x1080, 4:3 DVD (SAR 8:9) -> 1440x1080
        assert_eq!(scaled_size(&dvd(false), Some(1080)), (1920, 1080));
        let four_three = MediaInfo { sar: (8, 9), ..dvd(false) };
        assert_eq!(scaled_size(&four_three, Some(1080)), (1440, 1080));
        let square = MediaInfo { width: 1280, height: 720, sar: (1, 1), ..dvd(false) };
        assert_eq!(scaled_size(&square, Some(1080)), (1920, 1080));
    }

    #[test]
    fn keep_height_still_corrects_sar() {
        let opts = Options { height: Height::Keep, ..Options::default() };
        let f = video_filter(&dvd(false), &opts);
        assert!(f.contains("scale=854:480:flags=lanczos,setsar=1"), "{f}");
    }

    #[test]
    fn upscale_and_sar_correction_happen_in_one_step() {
        let f = video_filter(&dvd(false), &Options::default());
        assert!(f.contains("scale=1920:1080:flags=lanczos,setsar=1"), "{f}");
        assert_eq!(f.matches("scale=").count(), 1, "{f}");
    }
}

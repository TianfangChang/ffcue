//! CUE 扫描与轨道边界计算 / CUE Scanning and Track Boundary Calculation
//!
//! 提供计算精确的轨道边界和检测 HTOA（隐藏第一轨音频）的实用工具。
//! Provides utilities to calculate precise track boundaries and detect HTOA (Hidden Track One Audio).

use std::time::Duration;

use crate::models::{CueFile, CueTrackType};

/// HTOA（隐藏第一轨）的最小时长阈值（秒）。
/// Minimum duration threshold for HTOA (Hidden Track One Audio) in seconds.
///
/// 小于此值的 pre-gap 通常只是静音间隔，不作为独立轨道。
/// A pre-gap shorter than this is usually just a silent interval and not a separate track.
pub const HTOA_MIN_DURATION_SECS: u64 = 4;

/// 计算 CUE 轨道时间边界：(start, end)。
/// Calculates CUE track time boundaries: (start, end).
///
/// * `end` 为 `None` 表示它是当前文件的最后一轨（需播放到文件结尾）。
///   `end` being `None` indicates it's the last track of the current file (play to EOF).
/// * 起始时间取当前轨道的 `INDEX 01`。
///   Start time is `INDEX 01` of the current track.
/// * 结束时间取下一轨道的 `INDEX 00`，若无则取 `INDEX 01`。
///   End time is `INDEX 00` of the next track, or `INDEX 01` if `INDEX 00` is missing.
pub fn get_track_boundaries(
    cue_file: &CueFile,
    track_idx: usize,
) -> Option<(Duration, Option<Duration>)> {
    let track = cue_file.tracks.get(track_idx)?;
    let start = track.indices.iter()
        .find(|idx| idx.number == 1)
        .map(|idx| idx.position.to_duration())?;

    let end = if track_idx + 1 < cue_file.tracks.len() {
        let next = &cue_file.tracks[track_idx + 1];
        next.indices.iter()
            .find(|idx| idx.number == 0)
            .or_else(|| next.indices.iter().find(|idx| idx.number == 1))
            .map(|idx| idx.position.to_duration())
    } else {
        None
    };

    Some((start, end))
}

/// 检测文件中是否存在 HTOA（Hidden Track One Audio, 隐藏第一轨音频）并返回其边界。
/// Detects if there is HTOA (Hidden Track One Audio) in the file and returns its boundaries.
///
/// HTOA 仅当当前文件的第一个音频轨道的 `INDEX 01` 之前的间隔长度大于设定的阈值时被认为有效。
/// HTOA is considered valid only when the gap before `INDEX 01` of the first audio track 
/// is longer than the defined threshold (`HTOA_MIN_DURATION_SECS`).
///
/// 返回值：若存在 HTOA，则返回 `Some(end_duration)`，即第一轨 `INDEX 01` 的时间点。起始时间默认为 0。
/// Returns: `Some(end_duration)` if HTOA exists, which corresponds to `INDEX 01` of the first track. 
/// Start time is implicitly 0. Returns `None` otherwise.
pub fn get_htoa_boundary(cue_file: &CueFile) -> Option<Duration> {
    let first_track = cue_file.tracks.first()?;
    
    if first_track.track_type != CueTrackType::Audio {
        return None;
    }
    
    let idx01 = first_track.indices.iter().find(|i| i.number == 1)?;
    let htoa_end = idx01.position.to_duration();
    
    if htoa_end.as_secs() >= HTOA_MIN_DURATION_SECS {
        Some(htoa_end)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CueTrack, CueIndex, CueTimestamp};

    fn make_idx(number: u32, secs: u32) -> CueIndex {
        CueIndex {
            number,
            position: CueTimestamp { minutes: secs / 60, seconds: secs % 60, frames: 0 },
        }
    }

    fn make_track(number: u32, indices: Vec<CueIndex>) -> CueTrack {
        CueTrack {
            number,
            track_type: CueTrackType::Audio,
            title: None, performer: None, songwriter: None, isrc: None,
            flags: vec![], remarks: vec![], pregap: None, postgap: None,
            indices,
        }
    }

    #[test]
    fn test_track_boundaries() {
        let t1 = make_track(1, vec![make_idx(1, 10)]);
        let t2 = make_track(2, vec![make_idx(0, 50), make_idx(1, 52)]);
        let t3 = make_track(3, vec![make_idx(1, 120)]);
        
        let file = CueFile {
            filename: "audio.wav".into(),
            filetype: crate::models::CueFileType::Wave,
            tracks: vec![t1, t2, t3],
        };

        // t1: start at 10s, ends at t2 index 00 (50s)
        let (s1, e1) = get_track_boundaries(&file, 0).unwrap();
        assert_eq!(s1.as_secs(), 10);
        assert_eq!(e1.unwrap().as_secs(), 50);

        // t2: starts at 52s, ends at t3 index 01 (120s, missing index 00 fallback)
        let (s2, e2) = get_track_boundaries(&file, 1).unwrap();
        assert_eq!(s2.as_secs(), 52);
        assert_eq!(e2.unwrap().as_secs(), 120);

        // t3: starts at 120s, no end
        let (s3, e3) = get_track_boundaries(&file, 2).unwrap();
        assert_eq!(s3.as_secs(), 120);
        assert!(e3.is_none());
    }

    #[test]
    fn test_htoa_detection() {
        // Gap is 2 seconds (less than 4s threshold) -> None
        let t_short = make_track(1, vec![make_idx(1, 2)]);
        let file_short = CueFile { filename: "s.wav".into(), filetype: crate::models::CueFileType::Wave, tracks: vec![t_short] };
        assert!(get_htoa_boundary(&file_short).is_none());

        // Gap is 5 seconds (>= 4s threshold) -> Some(5)
        let t_long = make_track(1, vec![make_idx(1, 5)]);
        let file_long = CueFile { filename: "l.wav".into(), filetype: crate::models::CueFileType::Wave, tracks: vec![t_long] };
        assert_eq!(get_htoa_boundary(&file_long).unwrap().as_secs(), 5);
    }
}

//! CUE 文件解析器 / CUE Sheet Parser
//!
//! 逐行状态机 + chardetng 编码检测。 / Line-by-line state machine + chardetng encoding detection.
//! 宽容模式：未知指令跳过不报错，时间戳帧号溢出自动 clamp。 / Lenient mode: unknown directives are skipped without error, timestamp frames/seconds overflows are automatically clamped.

use std::path::Path;

use crate::error::{CueError, Result};
use crate::models::*;

// ──────────────────────────────────────────────────────────
//  公共 API / Public API
// ──────────────────────────────────────────────────────────

/// 解析 CUE 文件，自动检测编码，返回强类型 CueSheet / Parses a CUE file, auto-detects encoding, and returns a strongly-typed CueSheet
pub fn parse_cue_file(cue_path: &Path) -> Result<CueSheet> {
    let raw_bytes = std::fs::read(cue_path)
        .map_err(|e| CueError::Io {
            path: cue_path.display().to_string(),
            source: e,
        })?;

    let text = decode_text(&raw_bytes);
    Ok(parse_cue_text_inner(Some(cue_path), &text))
}

/// 从已有文本内容解析 CUE sheet / Parses a CUE sheet from existing text content
///
/// `source_path` 可选，用于设置 `CueSheet.cue_path`（如嵌入式 CUE 可传音频文件路径）
/// `source_path` is optional, used to set `CueSheet.cue_path` (e.g., for embedded CUE, you can pass the audio file path)
pub fn parse_cue_text(source_path: Option<&Path>, text: &str) -> CueSheet {
    parse_cue_text_inner(source_path, text)
}

/// 自动检测编码并解码为 UTF-8 / Auto-detects encoding and decodes to UTF-8
///
/// 支持 UTF-8 BOM、UTF-16 LE/BE、以及 chardetng 自动检测（GBK、Shift-JIS 等 CJK 编码）。
/// Supports UTF-8 BOM, UTF-16 LE/BE, and chardetng auto-detection (GBK, Shift-JIS, and other CJK encodings).
/// 可独立使用，用于在调用 [`parse_cue_text`] 前预处理原始字节。
/// Can be used independently to pre-process raw bytes before calling [`parse_cue_text`].
pub fn decode_text(bytes: &[u8]) -> String {
    // BOM 检测 / BOM detection
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (text, _, _) = encoding_rs::UTF_16LE.decode(
            if bytes.len() >= 2 { &bytes[2..] } else { &[] },
        );
        return text.into_owned();
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (text, _, _) = encoding_rs::UTF_16BE.decode(
            if bytes.len() >= 2 { &bytes[2..] } else { &[] },
        );
        return text.into_owned();
    }

    // chardetng 自动检测 / chardetng auto-detection
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    let encoding = detector.guess(None, true);
    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}

// ──────────────────────────────────────────────────────────
//  逐行状态机解析 / Line-by-line State Machine Parsing
// ──────────────────────────────────────────────────────────

/// 解析 CUE 文本内容 / Parses CUE text content
fn parse_cue_text_inner(cue_path: Option<&Path>, text: &str) -> CueSheet {
    let mut sheet = CueSheet {
        cue_path: cue_path.map(|p| p.to_path_buf()),
        catalog:    None,
        cdtextfile: None,
        performer:  None,
        title:      None,
        songwriter: None,
        remarks:    Vec::new(),
        files:      Vec::new(),
    };

    // 状态：当前 FILE 索引、当前 TRACK 索引 / State: Current FILE index, current TRACK index
    let mut current_file: Option<usize> = None;
    let mut current_track: Option<usize> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }

        // 分割为 (command, rest) / Split into (command, rest)
        let (cmd, rest) = split_first_word(line);
        let cmd_upper = cmd.to_uppercase();

        match cmd_upper.as_str() {
            "REM" => {
                // REM KEY VALUE / REM 键 值
                let (key, value) = split_first_word(rest);
                let value = unquote(value);
                if let (Some(fi), Some(ti)) = (current_file, current_track) {
                    sheet.files[fi].tracks[ti].remarks.push((key.to_uppercase(), value));
                } else {
                    sheet.remarks.push((key.to_uppercase(), value));
                }
            }

            "CATALOG" => {
                sheet.catalog = Some(unquote(rest));
            }

            "PERFORMER" => {
                let val = unquote(rest);
                if let (Some(fi), Some(ti)) = (current_file, current_track) {
                    sheet.files[fi].tracks[ti].performer = Some(val);
                } else {
                    sheet.performer = Some(val);
                }
            }

            "TITLE" => {
                let val = unquote(rest);
                if let (Some(fi), Some(ti)) = (current_file, current_track) {
                    sheet.files[fi].tracks[ti].title = Some(val);
                } else {
                    sheet.title = Some(val);
                }
            }

            "SONGWRITER" => {
                let val = unquote(rest);
                if let (Some(fi), Some(ti)) = (current_file, current_track) {
                    sheet.files[fi].tracks[ti].songwriter = Some(val);
                } else {
                    sheet.songwriter = Some(val);
                }
            }

            "FILE" => {
                // FILE "filename" TYPE / FILE "文件名" 类型
                let (filename, filetype_str) = parse_file_directive(rest);
                let filetype = CueFileType::from_str(&filetype_str);
                sheet.files.push(CueFile {
                    filename,
                    filetype,
                    tracks: Vec::new(),
                });
                current_file = Some(sheet.files.len() - 1);
                current_track = None;
            }

            "TRACK" => {
                // TRACK NN AUDIO / TRACK 轨道号 类型
                let (num_str, type_str) = split_first_word(rest);
                let number = num_str.parse::<u32>().unwrap_or(0);
                let track_type = CueTrackType::from_str(type_str)
                    .unwrap_or(CueTrackType::Audio);

                if let Some(fi) = current_file {
                    sheet.files[fi].tracks.push(CueTrack {
                        number,
                        track_type,
                        title:      None,
                        performer:  None,
                        songwriter: None,
                        isrc:       None,
                        flags:      Vec::new(),
                        remarks:    Vec::new(),
                        pregap:     None,
                        postgap:    None,
                        indices:    Vec::new(),
                    });
                    current_track = Some(sheet.files[fi].tracks.len() - 1);
                }
                // 如果 TRACK 出现在 FILE 之前（不规范），忽略
                // If TRACK appears before FILE (non-standard), ignore it
            }

            "INDEX" => {
                // INDEX NN MM:SS:FF / INDEX 索引号 时间轴
                let (num_str, time_str) = split_first_word(rest);
                let number = num_str.parse::<u32>().unwrap_or(0);
                if let Some(ts) = parse_timestamp(time_str) {
                    if let (Some(fi), Some(ti)) = (current_file, current_track) {
                        sheet.files[fi].tracks[ti].indices.push(CueIndex {
                            number,
                            position: ts,
                        });
                    }
                }
            }

            "PREGAP" => {
                if let Some(ts) = parse_timestamp(rest) {
                    if let (Some(fi), Some(ti)) = (current_file, current_track) {
                        sheet.files[fi].tracks[ti].pregap = Some(ts);
                    }
                }
            }

            "POSTGAP" => {
                if let Some(ts) = parse_timestamp(rest) {
                    if let (Some(fi), Some(ti)) = (current_file, current_track) {
                        sheet.files[fi].tracks[ti].postgap = Some(ts);
                    }
                }
            }

            "ISRC" => {
                if let (Some(fi), Some(ti)) = (current_file, current_track) {
                    sheet.files[fi].tracks[ti].isrc = Some(unquote(rest));
                }
            }

            "FLAGS" => {
                if let (Some(fi), Some(ti)) = (current_file, current_track) {
                    sheet.files[fi].tracks[ti].flags = rest
                        .split_whitespace()
                        .map(|s| s.to_uppercase())
                        .collect();
                }
            }

            "CDTEXTFILE" => {
                sheet.cdtextfile = Some(unquote(rest));
            }

            _ => {
                // 宽容模式：跳过不认识的指令 / Lenient mode: skip unrecognized directives
            }
        }
    }

    sheet
}

// ──────────────────────────────────────────────────────────
//  辅助函数 / Helper Functions
// ──────────────────────────────────────────────────────────

/// 分割第一个空白分隔的单词和剩余部分 / Splits the first whitespace-separated word and the remaining part
fn split_first_word(s: &str) -> (&str, &str) {
    let s = s.trim();
    match s.find(|c: char| c.is_whitespace()) {
        Some(pos) => (&s[..pos], s[pos..].trim_start()),
        None      => (s, ""),
    }
}

/// 剥离引号 / Strips quotes
fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// 解析 FILE 指令：`"filename with spaces" TYPE` 或 `filename TYPE`
/// Parses FILE directive: `"filename with spaces" TYPE` or `filename TYPE`
fn parse_file_directive(s: &str) -> (String, String) {
    let s = s.trim();
    if s.starts_with('"') {
        // 带引号的文件名 / Quoted filename
        if let Some(close_quote) = s[1..].find('"') {
            let filename = s[1..1 + close_quote].to_string();
            let rest = s[1 + close_quote + 1..].trim();
            let filetype = rest.split_whitespace().next().unwrap_or("WAVE").to_string();
            return (filename, filetype);
        }
    }
    // 无引号：最后一个空白分隔的词是 TYPE / Unquoted: the last whitespace-separated word is TYPE
    let parts: Vec<&str> = s.rsplitn(2, char::is_whitespace).collect();
    if parts.len() == 2 {
        (parts[1].to_string(), parts[0].to_string())
    } else {
        (s.to_string(), "WAVE".to_string())
    }
}

/// 解析 MM:SS:FF 时间戳，秒数溢出自动 clamp 到 59，帧号溢出自动 clamp 到 74
/// Parses MM:SS:FF timestamp; seconds overflow automatically clamps to 59, frames overflow automatically clamps to 74
pub fn parse_timestamp(s: &str) -> Option<CueTimestamp> {
    let s = s.trim();
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let minutes = parts[0].parse::<u32>().ok()?;
    let seconds = parts[1].parse::<u32>().ok()?.min(59); // clamp
    let frames  = parts[2].parse::<u32>().ok()?.min(74); // clamp
    Some(CueTimestamp { minutes, seconds, frames })
}

// ──────────────────────────────────────────────────────────
//  测试 / Tests
// ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_timestamp_display() {
        let ts = CueTimestamp { minutes: 3, seconds: 9, frames: 7 };
        assert_eq!(ts.to_string(), "03:09:07");

        // minutes may exceed 99 for very long files; format still works
        // 对于非常长的文件，分钟数可能超过 99；格式化依然有效
        let ts = CueTimestamp { minutes: 123, seconds: 45, frames: 0 };
        assert_eq!(ts.to_string(), "123:45:00");

        // round-trip: Display output can be parsed back
        // 往返测试：Display 输出可以被重新解析回去
        let ts = CueTimestamp { minutes: 7, seconds: 30, frames: 74 };
        assert_eq!(parse_timestamp(&ts.to_string()), Some(ts));
    }

    #[test]
    fn test_timestamp_parsing() {
        let ts = parse_timestamp("04:32:17").unwrap();
        assert_eq!(ts.minutes, 4);
        assert_eq!(ts.seconds, 32);
        assert_eq!(ts.frames, 17);

        // 帧数溢出 clamp / Frames overflow clamp
        let ts = parse_timestamp("00:00:99").unwrap();
        assert_eq!(ts.frames, 74);
    }

    #[test]
    fn test_timestamp_seconds_clamp() {
        let ts = parse_timestamp("00:99:00").unwrap();
        assert_eq!(ts.seconds, 59);
    }

    #[test]
    fn test_timestamp_conversion() {
        // 01:00:00 = 60秒 / 60 seconds
        let ts = CueTimestamp { minutes: 1, seconds: 0, frames: 0 };
        assert_eq!(ts.to_millis(), 60_000);

        // 00:01:00 = 1秒 / 1 second
        let ts = CueTimestamp { minutes: 0, seconds: 1, frames: 0 };
        assert_eq!(ts.to_millis(), 1_000);

        // 00:00:75 帧 = 1秒（理论值，帧号实际最大74）
        // 00:00:75 frames = 1 second (theoretical value, actual frames max at 74)
        let ts = CueTimestamp { minutes: 0, seconds: 0, frames: 75 };
        assert_eq!(ts.to_millis(), 1_000);
    }

    #[test]
    fn test_timestamp_ord() {
        let a = CueTimestamp { minutes: 1, seconds: 0, frames: 0 };
        let b = CueTimestamp { minutes: 0, seconds: 59, frames: 74 };
        assert!(a > b);
    }

    #[test]
    fn test_unquote() {
        assert_eq!(unquote("\"hello world\""), "hello world");
        assert_eq!(unquote("hello"), "hello");
        assert_eq!(unquote("\"\""), "");
    }

    #[test]
    fn test_file_directive() {
        let (name, ft) = parse_file_directive("\"Album.flac\" WAVE");
        assert_eq!(name, "Album.flac");
        assert_eq!(ft, "WAVE");

        let (name, ft) = parse_file_directive("\"My Album.ape\" WAVE");
        assert_eq!(name, "My Album.ape");
        assert_eq!(ft, "WAVE");

        let (name, ft) = parse_file_directive("track01.wav WAVE");
        assert_eq!(name, "track01.wav");
        assert_eq!(ft, "WAVE");
    }

    #[test]
    fn test_parse_simple_cue() {
        let text = r#"
REM GENRE Rock
REM DATE 2000
PERFORMER "Test Artist"
TITLE "Test Album"
FILE "album.flac" WAVE
  TRACK 01 AUDIO
    TITLE "Track One"
    PERFORMER "Artist One"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "Track Two"
    INDEX 00 03:45:10
    INDEX 01 03:47:00
  TRACK 03 AUDIO
    TITLE "Track Three"
    INDEX 01 07:22:30
"#;
        let sheet = parse_cue_text(Some(Path::new("test.cue")), text);
        assert_eq!(sheet.title.as_deref(), Some("Test Album"));
        assert_eq!(sheet.performer.as_deref(), Some("Test Artist"));
        assert_eq!(sheet.files.len(), 1);

        let file = &sheet.files[0];
        assert_eq!(file.filename, "album.flac");
        assert_eq!(file.tracks.len(), 3);

        assert_eq!(file.tracks[0].title.as_deref(), Some("Track One"));
        assert_eq!(file.tracks[0].performer.as_deref(), Some("Artist One"));
        assert_eq!(file.tracks[0].indices[0].number, 1);
        assert_eq!(file.tracks[0].indices[0].position.minutes, 0);

        // TRACK 02 有 INDEX 00 和 INDEX 01
        // TRACK 02 has INDEX 00 and INDEX 01
        assert_eq!(file.tracks[1].indices.len(), 2);
        assert_eq!(file.tracks[1].indices[0].number, 0);
        assert_eq!(file.tracks[1].indices[1].number, 1);

        // REM
        assert!(sheet.remarks.iter().any(|(k, v)| k == "DATE" && v == "2000"));
        assert!(sheet.remarks.iter().any(|(k, v)| k == "GENRE" && v == "Rock"));
    }

    #[test]
    fn test_parse_without_path() {
        let text = r#"
TITLE "No Path Album"
FILE "album.flac" WAVE
  TRACK 01 AUDIO
    INDEX 01 00:00:00
"#;
        let sheet = parse_cue_text(None, text);
        assert!(sheet.cue_path.is_none());
        assert_eq!(sheet.title.as_deref(), Some("No Path Album"));
    }

    #[test]
    fn test_parse_multi_file_cue() {
        let text = r#"
TITLE "Multi File Album"
FILE "track01.wav" WAVE
  TRACK 01 AUDIO
    TITLE "First"
    INDEX 01 00:00:00
FILE "track02.wav" WAVE
  TRACK 02 AUDIO
    TITLE "Second"
    INDEX 01 00:00:00
"#;
        let sheet = parse_cue_text(Some(Path::new("test.cue")), text);
        assert_eq!(sheet.files.len(), 2);
        assert_eq!(sheet.files[0].tracks.len(), 1);
        assert_eq!(sheet.files[1].tracks.len(), 1);
        assert_eq!(sheet.files[0].tracks[0].title.as_deref(), Some("First"));
        assert_eq!(sheet.files[1].tracks[0].title.as_deref(), Some("Second"));
    }

    #[test]
    fn test_parse_track_level_rem() {
        let text = r#"
TITLE "Test Album"
FILE "album.flac" WAVE
  TRACK 01 AUDIO
    TITLE "Song"
    REM COMPOSER "Bach"
    REM LYRICIST "Unknown"
    INDEX 01 00:00:00
"#;
        let sheet = parse_cue_text(Some(Path::new("test.cue")), text);
        let track = &sheet.files[0].tracks[0];
        assert!(track.remarks.iter().any(|(k, v)| k == "COMPOSER" && v == "Bach"));
        assert!(track.remarks.iter().any(|(k, v)| k == "LYRICIST" && v == "Unknown"));
    }

    #[test]
    fn test_parse_isrc_and_flags() {
        let text = r#"
FILE "album.flac" WAVE
  TRACK 01 AUDIO
    ISRC USAT29900609
    FLAGS DCP PRE
    INDEX 01 00:00:00
"#;
        let sheet = parse_cue_text(Some(Path::new("test.cue")), text);
        let track = &sheet.files[0].tracks[0];
        assert_eq!(track.isrc.as_deref(), Some("USAT29900609"));
        assert_eq!(track.flags, vec!["DCP", "PRE"]);
    }

    #[test]
    fn test_parse_pregap_postgap() {
        let text = r#"
FILE "album.flac" WAVE
  TRACK 01 AUDIO
    PREGAP 00:02:00
    INDEX 01 00:00:00
    POSTGAP 00:01:00
"#;
        let sheet = parse_cue_text(Some(Path::new("test.cue")), text);
        let track = &sheet.files[0].tracks[0];
        assert_eq!(track.pregap.unwrap().to_millis(), 2_000);
        assert!(track.postgap.is_some());
    }

    #[test]
    fn test_parse_lenient_unknown_directives() {
        let text = r#"
CDTEXTFILE "disc.cdt"
UNKNOWN_DIRECTIVE some value
FILE "album.flac" WAVE
  TRACK 01 AUDIO
    INDEX 01 00:00:00
"#;
        let sheet = parse_cue_text(Some(Path::new("test.cue")), text);
        assert_eq!(sheet.files.len(), 1);
        assert_eq!(sheet.files[0].tracks.len(), 1);
        assert_eq!(sheet.cdtextfile.as_deref(), Some("disc.cdt"));
    }

    #[test]
    fn test_timestamp_edge_cases() {
        assert!(parse_timestamp("").is_none());
        assert!(parse_timestamp("12:34").is_none());
        assert!(parse_timestamp("abc:00:00").is_none());

        let ts = parse_timestamp("999:59:74").unwrap();
        assert_eq!(ts.minutes, 999);
        assert_eq!(ts.seconds, 59);
        assert_eq!(ts.frames, 74);
    }

    #[test]
    fn test_catalog_parsing() {
        let text = r#"
CATALOG 0123456789012
TITLE "Test"
FILE "album.flac" WAVE
  TRACK 01 AUDIO
    INDEX 01 00:00:00
"#;
        let sheet = parse_cue_text(Some(Path::new("test.cue")), text);
        assert_eq!(sheet.catalog.as_deref(), Some("0123456789012"));
    }

    #[test]
    fn test_songwriter_at_both_levels() {
        let text = r#"
SONGWRITER "Album Writer"
FILE "album.flac" WAVE
  TRACK 01 AUDIO
    SONGWRITER "Track Writer"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    INDEX 01 05:00:00
"#;
        let sheet = parse_cue_text(Some(Path::new("test.cue")), text);
        assert_eq!(sheet.songwriter.as_deref(), Some("Album Writer"));
        assert_eq!(sheet.files[0].tracks[0].songwriter.as_deref(), Some("Track Writer"));
        assert_eq!(sheet.files[0].tracks[1].songwriter, None);
    }

    // ── 性能测试 / Performance Tests ───────────────

    fn generate_cue_text(track_count: u32) -> String {
        let mut s = String::with_capacity(track_count as usize * 120);
        s.push_str("REM GENRE \"Test Genre\"\n");
        s.push_str("REM DATE 2024\n");
        s.push_str("PERFORMER \"Performance Test Artist\"\n");
        s.push_str("TITLE \"Performance Test Album\"\n");
        s.push_str("FILE \"album.flac\" WAVE\n");
        for i in 1..=track_count {
            let mins = (i - 1) * 4;
            s.push_str(&format!("  TRACK {:02} AUDIO\n", i));
            s.push_str(&format!("    TITLE \"Track {:04}\"\n", i));
            s.push_str(&format!("    PERFORMER \"Artist {:04}\"\n", i));
            s.push_str(&format!("    REM COMPOSER \"Composer {:04}\"\n", i));
            if i > 1 {
                s.push_str(&format!("    INDEX 00 {:02}:{:02}:50\n", mins, (i % 60)));
            }
            s.push_str(&format!("    INDEX 01 {:02}:{:02}:00\n", mins, (i % 60)));
        }
        s
    }

    #[test]
    fn test_perf_parse_99_tracks() {
        let text = generate_cue_text(99);
        let start = std::time::Instant::now();
        let sheet = parse_cue_text(Some(Path::new("perf.cue")), &text);
        let elapsed = start.elapsed();

        assert_eq!(sheet.files.len(), 1);
        assert_eq!(sheet.files[0].tracks.len(), 99);
        assert_eq!(sheet.files[0].tracks[0].title.as_deref(), Some("Track 0001"));
        assert_eq!(sheet.files[0].tracks[98].title.as_deref(), Some("Track 0099"));
        assert!(elapsed.as_millis() < 10, "解析 99 轨耗时 {}ms / Parsing 99 tracks took {}ms", elapsed.as_millis(), elapsed.as_millis());
    }

    #[test]
    fn test_perf_parse_1000_cue_sheets() {
        let text = generate_cue_text(12);
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let sheet = parse_cue_text(Some(Path::new("perf.cue")), &text);
            assert_eq!(sheet.files[0].tracks.len(), 12);
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 500, "解析 1000 个 CUE 耗时 {}ms / Parsing 1000 CUEs took {}ms", elapsed.as_millis(), elapsed.as_millis());
    }

    #[test]
    fn test_perf_encoding_detection() {
        let utf8_text = generate_cue_text(20);
        let bytes = utf8_text.as_bytes();
        let start = std::time::Instant::now();
        for _ in 0..500 {
            let _ = decode_text(bytes);
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 200, "500 次编码检测耗时 {}ms / 500 encoding detections took {}ms", elapsed.as_millis(), elapsed.as_millis());
    }
}

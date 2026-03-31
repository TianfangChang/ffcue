//! CUE 数据模型 / CUE Data Models
//!
//! 完整表达 CUE sheet 的层次结构：Sheet → File → Track → Index。
//! Fully represents the hierarchical structure of a CUE sheet: Sheet → File → Track → Index.

use std::path::PathBuf;
use std::time::Duration;

// ──────────────────────────────────────────────────────────
//  CUE Sheet 层次结构 / CUE Sheet Hierarchy
// ──────────────────────────────────────────────────────────

/// CUE 文件解析后的完整结构 / The complete structure of a parsed CUE file
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CueSheet {
    /// CUE 文件自身路径（外部文件解析时为绝对路径，文本解析时为调用方提供的值）
    /// Path to the CUE file itself (absolute path if parsed from a file, or a user-provided path if parsed from text)
    pub cue_path: Option<PathBuf>,
    /// CATALOG（条形码 / EAN） / CATALOG (Barcode / EAN)
    pub catalog: Option<String>,
    /// 顶级 PERFORMER（专辑艺术家） / Top-level PERFORMER (Album Artist)
    pub performer: Option<String>,
    /// 顶级 TITLE（专辑名） / Top-level TITLE (Album Title)
    pub title: Option<String>,
    /// 顶级 SONGWRITER / Top-level SONGWRITER
    pub songwriter: Option<String>,
    /// REM 注释（DATE / GENRE / DISCID / COMMENT 等） / REM remarks (DATE / GENRE / DISCID / COMMENT etc.)
    pub remarks: Vec<(String, String)>,
    /// FILE 块列表（一个 CUE 可引用多个文件） / List of FILE blocks (a CUE sheet can reference multiple files)
    pub files: Vec<CueFile>,
}

/// 一个 FILE 块 / A single FILE block
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CueFile {
    /// FILE 指令中的文件名（原始字符串，可能是相对路径）
    /// The filename from the FILE directive (raw string, potentially a relative path)
    pub filename: String,
    /// 文件类型 / File type
    pub filetype: CueFileType,
    /// 该文件内的 TRACK 列表 / List of TRACKs within this file
    pub tracks: Vec<CueTrack>,
}

/// FILE 指令的文件类型 / File type specified in the FILE directive
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CueFileType {
    Wave,
    Mp3,
    Aiff,
    Flac,
    Binary,
    Motorola,
    Unknown(String),
}

impl CueFileType {
    /// 从字符串解析文件类型 / Parse file type from a string
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "WAVE" | "WAV" => Self::Wave,
            "MP3"          => Self::Mp3,
            "AIFF" | "AIF" => Self::Aiff,
            "FLAC"         => Self::Flac,
            "BINARY"       => Self::Binary,
            "MOTOROLA"     => Self::Motorola,
            _              => Self::Unknown(s.to_string()),
        }
    }
}

/// 一个 TRACK 块 / A single TRACK block
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CueTrack {
    /// 轨道号（CUE 中的 TRACK NN） / Track number (TRACK NN in CUE)
    pub number: u32,
    /// 数据类型 / Data type
    pub track_type: CueTrackType,
    /// TITLE / 标题
    pub title: Option<String>,
    /// PERFORMER / 艺术家
    pub performer: Option<String>,
    /// SONGWRITER / 作曲者
    pub songwriter: Option<String>,
    /// ISRC / 国际标准录音代码
    pub isrc: Option<String>,
    /// FLAGS（DCP / 4CH / PRE / SCMS） / FLAGS (DCP / 4CH / PRE / SCMS)
    pub flags: Vec<String>,
    /// REM 注释 / REM remarks
    pub remarks: Vec<(String, String)>,
    /// PREGAP（虚拟间隔，不存在于音频数据中） / PREGAP (virtual gap, not present in the audio data)
    pub pregap: Option<CueTimestamp>,
    /// POSTGAP / 轨道后间隙
    pub postgap: Option<CueTimestamp>,
    /// INDEX 列表 / List of INDEXes
    pub indices: Vec<CueIndex>,
}

/// TRACK 数据类型 / TRACK data type
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CueTrackType {
    Audio,
    Cdg,
    Mode1_2048,
    Mode1_2352,
    Mode2_2336,
    Mode2_2352,
    Cdi2336,
    Cdi2352,
}

impl CueTrackType {
    /// 从字符串解析轨道类型 / Parse track type from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "AUDIO"      => Some(Self::Audio),
            "CDG"        => Some(Self::Cdg),
            "MODE1/2048" => Some(Self::Mode1_2048),
            "MODE1/2352" => Some(Self::Mode1_2352),
            "MODE2/2336" => Some(Self::Mode2_2336),
            "MODE2/2352" => Some(Self::Mode2_2352),
            "CDI/2336"   => Some(Self::Cdi2336),
            "CDI/2352"   => Some(Self::Cdi2352),
            _            => None,
        }
    }
}

/// INDEX NN MM:SS:FF
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CueIndex {
    /// 索引号（00 = pregap 起始, 01 = 播放起始） / Index number (00 = pregap start, 01 = playback start)
    pub number: u32,
    /// 时间位置 / Time position
    pub position: CueTimestamp,
}

/// MM:SS:FF 时间戳（FF = 帧，1帧 = 1/75秒） / MM:SS:FF Timestamp (FF = frames, 1 frame = 1/75 second)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CueTimestamp {
    pub minutes: u32,
    pub seconds: u32,
    /// 0–74，每帧 = 1/75 秒 ≈ 13.33ms / 0-74, each frame = 1/75 second ≈ 13.33ms
    pub frames: u32,
}

impl PartialOrd for CueTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CueTimestamp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.total_frames().cmp(&other.total_frames())
    }
}

impl std::fmt::Display for CueTimestamp {
    /// Formats the timestamp as `MM:SS:FF` (zero-padded to 2 digits each),
    /// matching the CUE sheet text representation.
    /// 将时间戳格式化为 `MM:SS:FF`（每部分补齐两位零），匹配 CUE sheet 中的文本表示。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.minutes, self.seconds, self.frames)
    }
}

impl CueTimestamp {
    /// 总帧数（内部计算用） / Total frames (for internal calculations)
    pub fn total_frames(&self) -> u64 {
        self.minutes as u64 * 60 * 75
            + self.seconds as u64 * 75
            + self.frames as u64
    }

    /// 转换为 Duration（精度到 1/75 秒） / Convert to Duration (precision to 1/75 second)
    pub fn to_duration(&self) -> Duration {
        let total = self.total_frames();
        // total_frames * (1_000_000_000 / 75) 纳秒
        // total_frames * (1_000_000_000 / 75) nanoseconds
        Duration::from_nanos(total * 1_000_000_000 / 75)
    }

    /// 转换为毫秒 / Convert to milliseconds
    pub fn to_millis(&self) -> u64 {
        self.total_frames() * 1000 / 75
    }
}

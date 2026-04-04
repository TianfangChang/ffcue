//! # ffcue
//!
//! CUE sheet parser with automatic CJK encoding detection and fuzzy file path resolution.
//!
//! CUE sheet 解析器，内置 CJK 编码自动检测和模糊文件路径解析。
//!
//! ## Features / 特性
//!
//! - **Encoding detection / 编码检测** — automatically detects and decodes
//!   UTF-8, UTF-16 LE/BE, GBK, Shift-JIS, Big5, and other encodings via `chardetng`
//! - **Lenient parsing / 宽容解析** — unknown directives are silently skipped;
//!   out-of-range seconds (>59) and frames (>74) are clamped
//! - **Strong typing / 强类型** — track numbers are `u32`, file types and track types
//!   are enums, timestamps support `Ord` comparison via `total_frames()`
//! - **Fuzzy path resolution / 模糊路径解析** (feature `resolver`, on by default) —
//!   4-level strategy: direct match → case-insensitive → extension swap → unique file inference
//! - **Optional serde support / 可选序列化** — enable the `serde` feature for
//!   `Serialize`/`Deserialize` on all model types
//!
//! ## Quick Start / 快速开始
//!
//! ```rust
//! use ffcue::parser::{parse_cue_file, parse_cue_text};
//!
//! // Parse from file (auto encoding detection)
//! // 从文件解析（自动编码检测）
//! // let sheet = parse_cue_file(Path::new("album.cue")).unwrap();
//!
//! // Parse from text / 从文本解析
//! let sheet = parse_cue_text(None, r#"
//! PERFORMER "Artist"
//! TITLE "Album"
//! FILE "album.flac" WAVE
//!   TRACK 01 AUDIO
//!     TITLE "Song One"
//!     INDEX 01 00:00:00
//!   TRACK 02 AUDIO
//!     TITLE "Song Two"
//!     INDEX 01 05:30:00
//! "#);
//!
//! assert_eq!(sheet.title.as_deref(), Some("Album"));
//! assert_eq!(sheet.files[0].tracks.len(), 2);
//! ```
//!
//! ## Resolve file paths / 解析文件路径
//!
//! ```rust,no_run
//! use std::path::Path;
//! use ffcue::parser::parse_cue_file;
//! use ffcue::resolver::resolve_all_files;
//!
//! let sheet = parse_cue_file(Path::new("album.cue")).unwrap();
//! let resolved = resolve_all_files(&sheet);
//! for (file_idx, audio_path) in &resolved {
//!     println!("FILE[{}] → {}", file_idx, audio_path.display());
//! }
//! ```

pub mod error;
pub mod models;
pub mod parser;
pub mod scanner;

#[cfg(feature = "resolver")]
pub mod resolver;

// Re-export core types at crate root for convenience
// 在 crate 根重新导出核心类型
pub use error::{CueError, Result};
pub use models::{CueSheet, CueFile, CueFileType, CueTrack, CueTrackType, CueIndex, CueTimestamp};
pub use parser::{parse_cue_file, parse_cue_text, decode_text};
pub use scanner::{get_track_boundaries, get_htoa_boundary, HTOA_MIN_DURATION_SECS};

#[cfg(feature = "resolver")]
pub use resolver::{resolve_audio_path, resolve_all_files, resolve_all_files_in};

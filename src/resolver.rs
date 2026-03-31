//! CUE file path resolution with fuzzy matching
//! CUE 文件路径解析与模糊匹配
//!
//! Resolves FILE directives in CUE sheets to actual audio file paths,
//! handling case mismatches, extension changes, and single-file inference.
//!
//! 将 CUE FILE 指令中的（可能是相对的、大小写错误的、扩展名不匹配的）
//! 文件名解析为实际存在的音频文件绝对路径。

use std::path::{Path, PathBuf};

use crate::models::{CueFile, CueSheet};

/// Known audio file extensions (lowercase).
/// 已知的音频文件扩展名（小写）。
const AUDIO_EXTENSIONS: &[&str] = &[
    "flac", "ape", "wav", "wv", "m4a", "mp3",
    "aiff", "aif", "ogg", "opus", "dsf", "dff",
    "mpc", "aac", "tak",
];

/// Resolve a FILE directive's filename to an absolute path.
/// 将 CUE FILE 指令中的文件名解析为绝对路径。
///
/// Uses a 4-level lookup strategy (decreasing priority):
/// 采用 4 级查找策略（优先级递减）：
///
/// 1. **Direct match** — join relative to `cue_dir`, check existence
///    直接拼接，完全匹配
/// 2. **Case-insensitive** — scan directory entries ignoring case
///    大小写不敏感匹配（遍历目录）
/// 3. **Extension swap** — same stem, different audio extension (e.g. `.ape` → `.flac`)
///    同名不同扩展名
/// 4. **Unique file inference** — if `single_file_cue` is true and the directory
///    contains exactly one audio file, use it
///    唯一音频文件推断（目录下仅一个音频文件 + CUE 仅一个 FILE 块）
pub fn resolve_audio_path(cue_dir: &Path, file_entry: &CueFile, single_file_cue: bool) -> Option<PathBuf> {
    let raw = &file_entry.filename;

    // Handle backslash path separators (common in Windows CUE files)
    // 处理反斜杠路径分隔符（Windows CUE 文件常见）
    let normalized = raw.replace('\\', "/");
    let raw_path = Path::new(&normalized);

    // Strategy 1: direct join / 策略 1：直接拼接
    let direct = cue_dir.join(raw_path);
    if direct.exists() {
        return Some(canonicalize_safe(&direct));
    }

    // Read directory entries (shared by strategies 2–4)
    // 读取目录内容（后续策略共用）
    let entries: Vec<PathBuf> = match std::fs::read_dir(cue_dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => return None,
    };

    let target_name = raw_path.file_name()?.to_str()?.to_lowercase();

    // Strategy 2: case-insensitive / 策略 2：大小写不敏感匹配
    for entry in &entries {
        if let Some(name) = entry.file_name().and_then(|n| n.to_str()) {
            if name.to_lowercase() == target_name && entry.is_file() {
                return Some(canonicalize_safe(entry));
            }
        }
    }

    // Strategy 3: same stem, different audio extension / 策略 3：同名不同扩展名
    let stem = raw_path.file_stem()?.to_str()?.to_lowercase();
    for entry in &entries {
        if !entry.is_file() { continue; }
        if let (Some(s), Some(ext)) = (
            entry.file_stem().and_then(|n| n.to_str()),
            entry.extension().and_then(|n| n.to_str()),
        ) {
            if s.to_lowercase() == stem
                && AUDIO_EXTENSIONS.contains(&ext.to_lowercase().as_str())
            {
                return Some(canonicalize_safe(entry));
            }
        }
    }

    // Strategy 4: unique audio file inference / 策略 4：唯一音频文件推断
    if single_file_cue {
        let audio_files: Vec<&PathBuf> = entries.iter()
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
                        .unwrap_or(false)
            })
            .collect();
        if audio_files.len() == 1 {
            return Some(canonicalize_safe(audio_files[0]));
        }
    }

    None
}

/// Resolve all FILE blocks in a CueSheet to absolute paths.
/// 为 CueSheet 中的所有 FILE 块解析绝对路径。
///
/// The CUE directory is derived from `sheet.cue_path`; returns an empty vec if
/// `cue_path` is `None`. To supply the directory explicitly, use [`resolve_all_files_in`].
///
/// Returns `Vec<(file_index, resolved_path)>`. Unresolvable entries are omitted.
/// 返回 `Vec<(file_index, resolved_path)>`，无法解析的 FILE 块不在列表中。
pub fn resolve_all_files(sheet: &CueSheet) -> Vec<(usize, PathBuf)> {
    let cue_dir = match &sheet.cue_path {
        Some(p) => p.parent().unwrap_or(Path::new(".")),
        None => return Vec::new(),
    };
    resolve_all_files_in(sheet, cue_dir)
}

/// Resolve all FILE blocks in a CueSheet to absolute paths, using `cue_dir` as the
/// base directory for relative FILE references.
/// 为 CueSheet 中的所有 FILE 块解析绝对路径，使用显式提供的 `cue_dir` 作为基准目录。
///
/// Useful when the sheet was parsed from text (i.e. `cue_path` is `None`) but the
/// caller knows which directory to resolve against.
/// 适用于 CUE 是从文本解析（即 `cue_path` 为 `None`）但调用方知道应相对于哪个目录解析的情况。
///
/// Returns `Vec<(file_index, resolved_path)>`. Unresolvable entries are omitted.
/// 返回 `Vec<(file_index, resolved_path)>`，合并无法解析的出项。
pub fn resolve_all_files_in(sheet: &CueSheet, cue_dir: &Path) -> Vec<(usize, PathBuf)> {
    let single_file = sheet.files.len() == 1;
    sheet.files.iter().enumerate()
        .filter_map(|(i, f)| {
            resolve_audio_path(cue_dir, f, single_file).map(|p| (i, p))
        })
        .collect()
}

/// Safe canonicalize: returns original path on failure.
/// 安全的 canonicalize：失败时返回原路径。
///
/// On Windows, `std::fs::canonicalize` prepends `\\?\` (extended-length path prefix).
/// This prefix is stripped so that returned paths compare equal to paths produced by
/// other APIs (e.g. `walkdir`, `Path::new`) that do not add it.
fn canonicalize_safe(path: &Path) -> PathBuf {
    match std::fs::canonicalize(path) {
        Ok(p) => strip_unc_prefix(p),
        Err(_) => path.to_path_buf(),
    }
}

/// Strip the Windows `\\?\` extended-length path prefix when present.
/// 去除 Windows `\\?\` 扩展长度前缀（若存在）。
#[cfg(windows)]
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(stripped);
    }
    path
}

#[cfg(not(windows))]
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
    path
}

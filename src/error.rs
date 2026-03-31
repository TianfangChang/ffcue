//! Error types. / 错误类型。

use std::io;

/// ffcue error. / ffcue 错误。
#[derive(Debug, thiserror::Error)]
pub enum CueError {
    /// I/O error reading a CUE file. / 读取 CUE 文件时发生 I/O 错误。
    #[error("failed to read CUE file '{path}': {source}")]
    Io {
        path: String,
        source: io::Error,
    },

    /// Parse error (reserved for future strict-mode use; not triggered in lenient mode).
    /// 解析错误（留作未来严格模式使用；在宽容模式下不会触发）。
    #[error("CUE parse error: {0}")]
    Parse(String),
}

/// Result alias. / Result 别名。
pub type Result<T> = std::result::Result<T, CueError>;

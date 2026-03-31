# ffcue

CUE sheet parser with automatic CJK encoding detection and fuzzy file path resolution.

CUE sheet 解析器，内置 CJK 编码自动检测和模糊文件路径解析。

## Why ffcue? / 为什么选 ffcue？

Existing Rust CUE parsers (`rcue`, `cuna`, `cue`) all assume UTF-8 input. In practice, CUE sheets for Chinese and Japanese music are often encoded in GBK, Shift-JIS, or Big5 — causing garbled metadata or parse failures. ffcue solves this with built-in encoding detection powered by `chardetng`.

现有 Rust CUE 解析器（`rcue`、`cuna`、`cue`）均假定输入为 UTF-8。但实际中，中日文音乐的 CUE 文件通常使用 GBK、Shift-JIS 或 Big5 编码，导致乱码或解析失败。ffcue 通过内置 `chardetng` 编码检测解决了这一问题。

| Feature | rcue | cuna | cue (libcue) | ffcue |
|---------|------|------|--------------|-------|
| Pure Rust | ✅ | ✅ | ❌ (C FFI) | ✅ |
| License | MIT | MIT | GPL-2.0 | MIT / Apache-2.0 |
| CJK encoding detection | ❌ | ❌ | ❌ | ✅ |
| UTF-16 BOM | ❌ | ❌ | ❌ | ✅ |
| Fuzzy path resolution | ❌ | ❌ | ❌ | ✅ |
| Strong typing (enums) | ❌ | partial | ✅ | ✅ |
| Integer timestamp math | ❌ (f64) | ✅ | ✅ (sectors) | ✅ |
| Lenient parsing | ✅ | ✅ | ✅ | ✅ |
| serde support | ❌ | ❌ | ❌ | opt-in |

## Install / 安装

```toml
[dependencies]
ffcue = "0.1"
```

### Feature flags

| Flag | Default | Description |
|------|---------|-------------|
| `resolver` | ✅ | Fuzzy file path resolution (requires filesystem access) / 模糊文件路径解析 |
| `serde` | ❌ | `Serialize`/`Deserialize` for all model types / 所有模型类型的序列化支持 |

## Usage / 使用

### Parse a CUE file / 解析 CUE 文件

```rust
use std::path::Path;
use ffcue::parser::parse_cue_file;

let sheet = parse_cue_file(Path::new("album.cue")).unwrap();

println!("Album: {}", sheet.title.unwrap_or_default());
println!("Artist: {}", sheet.performer.unwrap_or_default());

for file in &sheet.files {
    for track in &file.tracks {
        let start = track.indices.iter()
            .find(|i| i.number == 1)
            .map(|i| i.position.to_duration());
        println!(
            "  Track {:02} - {} (starts at {:?})",
            track.number,
            track.title.as_deref().unwrap_or("?"),
            start,
        );
    }
}
```

### Parse from text / 从文本解析

```rust
use ffcue::parser::parse_cue_text;

let cue_text = r#"
PERFORMER "Artist"
TITLE "Album"
FILE "album.flac" WAVE
  TRACK 01 AUDIO
    TITLE "Song One"
    INDEX 01 00:00:00
"#;

let sheet = parse_cue_text(None, cue_text);
assert_eq!(sheet.title.as_deref(), Some("Album"));
```

### Decode raw bytes (manual encoding detection) / 手动编码检测

```rust
use ffcue::parser::decode_text;

let raw_bytes = std::fs::read("album.cue").unwrap();
let utf8_text = decode_text(&raw_bytes);
// Now parse the decoded text / 然后解析解码后的文本
let sheet = ffcue::parser::parse_cue_text(None, &utf8_text);
```

### Resolve FILE paths / 解析 FILE 路径

```rust,no_run
use std::path::Path;
use ffcue::parser::parse_cue_file;
use ffcue::resolver::{resolve_all_files, resolve_audio_path};

let sheet = parse_cue_file(Path::new("/music/album/disc.cue")).unwrap();

// Resolve all FILE blocks at once / 一次解析所有 FILE 块
let resolved = resolve_all_files(&sheet);
for (idx, path) in &resolved {
    println!("FILE[{}] → {}", idx, path.display());
}

// Or resolve a single FILE entry / 或单独解析一个 FILE 条目
let cue_dir = Path::new("/music/album");
if let Some(path) = resolve_audio_path(cue_dir, &sheet.files[0], sheet.files.len() == 1) {
    println!("Found: {}", path.display());
}
```

The resolver uses a 4-level strategy / 解析器使用 4 级策略：

1. **Direct match / 直接匹配** — `cue_dir/filename` exists
2. **Case-insensitive / 大小写不敏感** — scan directory ignoring case
3. **Extension swap / 换扩展名** — same stem, different audio extension (e.g. `.ape` → `.flac` after transcoding)
4. **Unique file inference / 唯一文件推断** — single-FILE CUE + directory has exactly one audio file

### Timestamps / 时间戳

```rust
use ffcue::CueTimestamp;

let ts = CueTimestamp { minutes: 3, seconds: 45, frames: 37 };

// Integer arithmetic, no floating point / 整数运算，无浮点数
assert_eq!(ts.total_frames(), 3 * 60 * 75 + 45 * 75 + 37);
assert_eq!(ts.to_millis(), 225493);

// Duration conversion / Duration 转换
let dur = ts.to_duration();
println!("{:?}", dur); // 225.493333...s

// Ord comparison based on total_frames / 基于 total_frames 的排序比较
let earlier = CueTimestamp { minutes: 3, seconds: 45, frames: 0 };
assert!(earlier < ts);
```

## Data Model / 数据模型

```
CueSheet
├── cue_path: Option<PathBuf>      // source file path / 源文件路径
├── catalog: Option<String>        // EAN/barcode
├── performer: Option<String>      // album artist / 专辑艺术家
├── title: Option<String>          // album title / 专辑名
├── songwriter: Option<String>
├── remarks: Vec<(String, String)> // REM key-value pairs
└── files: Vec<CueFile>
    ├── filename: String           // raw FILE path / 原始 FILE 路径
    ├── filetype: CueFileType      // Wave | Mp3 | Aiff | Flac | Binary | Motorola | Unknown
    └── tracks: Vec<CueTrack>
        ├── number: u32            // track number (1–99)
        ├── track_type: CueTrackType  // Audio | Cdg | Mode1_2048 | ...
        ├── title, performer, songwriter: Option<String>
        ├── isrc: Option<String>
        ├── flags: Vec<String>     // DCP, 4CH, PRE, SCMS
        ├── remarks: Vec<(String, String)>
        ├── pregap, postgap: Option<CueTimestamp>
        └── indices: Vec<CueIndex>
            ├── number: u32        // 00 = pregap start, 01 = track start
            └── position: CueTimestamp { minutes, seconds, frames }
```

## Functional Completeness / 功能完备性

Below is a comparison of supported directives and features across existing Rust CUE parsers:

| Directive or Feature / 指令或特性 | rcue | cuna | cue (libcue) | ffcue |
| :--- | :---: | :---: | :---: | :---: |
| **CATALOG** | ✅ | ✅ (13-digit) | ✅ | ✅ |
| **PERFORMER** (sheet/track) | ✅ | ✅ | ✅ | ✅ |
| **TITLE** (sheet/track) | ✅ | ✅ | ✅ | ✅ |
| **SONGWRITER** | ✅ | ✅ | ✅ | ✅ |
| **FILE** (Multi-file / 多文件) | ✅ | ✅ | ✅ | ✅ |
| **TRACK** | ✅ | ✅ | ✅ | ✅ |
| **INDEX** | ✅ | ✅ | ✅ | ✅ |
| **PREGAP / POSTGAP** | ✅ | ✅ | ✅ | ✅ |
| **ISRC** | ✅ | ✅ | ✅ | ✅ |
| **FLAGS** | ✅ | ✅ | ✅ | ✅ |
| **REM** (Key-value / 键值对) | ✅ | ✅ | ✅ | ✅ |
| **CDTEXTFILE** | ✅ | ✅ | ✅ | Ignored / 忽略 |
| **CJK Encoding Detection** | ❌ | ❌ | ❌ | **✅ (Auto)** |
| **UTF-16 LE/BE** | ❌ | ❌ | ❌ | **✅** |
| **Fuzzy Path Resolution** | ❌ | ❌ | ❌ | **✅** |
| **serde Support** | ❌ | ❌ | ❌ | **opt-in** |

Note: `ffcue` prioritizes robustness in real-world CJK environments, ensuring that metadata is preserved even when encoding varies across different sources.

## 致谢 / Acknowledgements

This project was designed and implemented with reference to the following open-source projects (listed in alphabetical order):

- rcue — https://github.com/gyng/rcue
- cuna — https://github.com/snylonue/cuna
- cue (libcue-based) — https://github.com/lipnitsk/libcue

These projects provided valuable insights into CUE sheet parsing workflows, data structure design, and edge-case handling. Parts of this implementation were informed by comparative reading of their source code, with adaptations and refactoring applied to align with the goals of this project.

We would like to express our sincere appreciation to the authors and contributors of these projects.

Additional thanks to Claude for assistance with coding, problem-solving, and implementation discussions during development.

本项目在设计与实现过程中参考并受益于以下开源项目（按字母顺序排列）：

- rcue — https://github.com/gyng/rcue
- cuna — https://github.com/snylonue/cuna
- cue（基于 libcue）— https://github.com/lipnitsk/libcue

这些项目在 CUE sheet 解析流程、数据结构组织以及边界情况处理等方面提供了重要参考与启发。部分实现思路在对其源码阅读与对比分析后进行了适配与重构，以符合本项目的设计目标。

在此对上述项目的作者与所有贡献者表示诚挚感谢。

此外，本项目在开发过程中借助了 Claude 进行辅助编码、问题分析与实现细节讨论，在此一并致谢。


## License / 许可证

MIT / Apache-2.0

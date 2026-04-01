# ffcue

CUE sheet parser with automatic CJK encoding detection and fuzzy file path resolution.

CUE sheet 解析器，内置 CJK 编码自动检测和模糊文件路径解析。

## Why ffcue? / 为什么选 ffcue？

Many existing CUE parsers are optimized for UTF-8 input. In real-world Chinese and Japanese music archives, however, CUE sheets are often encoded in GBK, Shift-JIS, Big5, or UTF-16, which can lead to garbled metadata or parsing failures.

许多现有 CUE 解析器主要面向 UTF-8 输入设计；但在实际的中日文音乐归档中，CUE 文件常见 GBK、Shift-JIS、Big5 或 UTF-16 编码，容易造成元数据乱码或解析失败。

`ffcue` is designed for these real-world archives. It provides built-in encoding detection, robust text decoding, and optional fuzzy file path resolution so that imperfect but common CUE collections remain usable.

`ffcue` 面向这类真实归档场景设计，提供内置编码检测、稳健文本解码，以及可选的模糊文件路径解析，使存在历史遗留问题的 CUE 资源仍然可用。

## Install / 安装

```toml
[dependencies]
ffcue = "0.1"
```

### Feature flags

| Flag | Default | Description |
|------|---------|-------------|
| `resolver` | ✅ | Fuzzy file path resolution (requires filesystem access) / 模糊文件路径解析 |
| `serde` | ❌ | `Serialize`/`Deserialize` for model types / 模型类型的序列化支持 |

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

### Decode raw bytes / 手动解码原始字节

The decoder (`src/parser.rs::decode_text`) uses a 4-level fallback strategy / 解码器（`src/parser.rs::decode_text`）包含 4 级回退策略：
1. **UTF-8 BOM**
2. **UTF-16 LE BOM**
3. **UTF-16 BE BOM**
4. **chardetng** auto-detection for CJK/unknown encodings / 针对 CJK/未知编码的自动推断

```rust
use ffcue::parser::decode_text;

let raw_bytes = std::fs::read("album.cue").unwrap();
let utf8_text = decode_text(&raw_bytes);

let sheet = ffcue::parser::parse_cue_text(None, &utf8_text);
```

### Resolve FILE paths / 解析 FILE 路径

```rust,no_run
use std::path::Path;
use ffcue::parser::parse_cue_file;
use ffcue::resolver::{resolve_all_files, resolve_audio_path};

let sheet = parse_cue_file(Path::new("/music/album/disc.cue")).unwrap();

// Resolve all FILE blocks
let resolved = resolve_all_files(&sheet);
for (idx, path) in &resolved {
    println!("FILE[{}] -> {}", idx, path.display());
}

// Resolve a single FILE entry
let cue_dir = Path::new("/music/album");
if let Some(path) = resolve_audio_path(cue_dir, &sheet.files, sheet.files.len() == 1) {
    println!("Found: {}", path.display());
}
```

The resolver uses a 4-level strategy (see `src/resolver.rs::resolve_audio_path`) / 路径解析包含 4 级推断策略（详情见 `src/resolver.rs::resolve_audio_path`）：

1. **Direct match / 直接匹配** — `cue_dir/filename` exists  
2. **Case-insensitive / 大小写不敏感** — scan directory ignoring case  
3. **Extension swap / 换扩展名** — same stem, different audio extension  
4. **Unique file inference / 唯一文件推断** — single-FILE CUE + directory has exactly one audio file  

### Timestamps / 时间戳

```rust
use ffcue::CueTimestamp;

let ts = CueTimestamp { minutes: 3, seconds: 45, frames: 37 };

assert_eq!(ts.total_frames(), 3 * 60 * 75 + 45 * 75 + 37);
assert_eq!(ts.to_millis(), 225493);

let dur = ts.to_duration();
println!("{:?}", dur);

let earlier = CueTimestamp { minutes: 3, seconds: 45, frames: 0 };
assert!(earlier < ts);
```

### Serialize and Deserialize / 序列化与反序列化

Enable the `serde` feature first / 先启用 `serde` 特性：

```toml
[dependencies]
ffcue = { version = "0.1", features = ["serde"] }
```

Then serialize parsed models / 然后即可序列化解析结果：

```rust,ignore
use ffcue::parser::parse_cue_text;
use serde_json;

let cue_text = "TITLE \"Example\"";
let sheet = parse_cue_text(None, cue_text);

let json_str = serde_json::to_string_pretty(&sheet).unwrap();
println!("{}", json_str);
```

## Data Model / 数据模型

```text
CueSheet
├── cue_path: Option<PathBuf>
├── catalog: Option<String>
├── performer: Option<String>
├── title: Option<String>
├── songwriter: Option<String>
├── remarks: Vec<(String, String)>
└── files: Vec<CueFile>
    ├── filename: String
    ├── filetype: CueFileType
    └── tracks: Vec<CueTrack>
        ├── number: u32
        ├── track_type: CueTrackType
        ├── title, performer, songwriter: Option<String>
        ├── isrc: Option<String>
        ├── flags: Vec<String>
        ├── remarks: Vec<(String, String)>
        ├── pregap, postgap: Option<CueTimestamp>
        └── indices: Vec<CueIndex>
            ├── number: u32
            └── position: CueTimestamp
```

## Comparison Notes / 对比说明

The table below is intentionally conservative. It summarizes public documentation and observed project positioning, and should not be read as a line-by-line formal compliance matrix.

下表采用保守写法，仅总结公开文档与项目定位，不应视为逐条标准兼容性认证结果。

| Project | Publicly observable position |
|---------|------------------------------|
| `rcue` | A simple Rust CUE reader. Public docs do not advertise automatic CJK encoding detection or UTF-16 decoding. |
| `cuna` | MIT-licensed Rust parser; public package description explicitly mentions UTF-8 and UTF-8 with BOM support. |
| `cue` | Rust bindings around `libcue`, not a standalone pure-Rust parser. |
| `ffcue` | Focuses on real-world archive robustness: encoding detection, decoding, and optional path resolution. |

### At-a-glance feature goals / 目标特性概览

| Feature | rcue | cuna | cue (libcue bindings) | ffcue |
|---------|------|------|-----------------------|-------|
| License | MIT | MIT | GPL-2.0 | MIT / Apache-2.0 |
| Automatic CJK encoding detection | No public support statement | No public support statement | No public support statement | ✅ |
| UTF-16 decoding | No public support statement | UTF-8 BOM documented; UTF-16 not claimed | No public support statement | ✅ |
| Fuzzy path resolution | ❌ | ❌ | ❌ | ✅ |
| Strong typed model | Limited / lightweight | Partial | Higher-level typed wrapper over `libcue` | ✅ |
| Integer timestamp math | Limited | ✅ | ✅ | ✅ |
| Lenient parsing | Generally permissive | Generally permissive | Backed by `libcue` behavior | ✅ |
| `serde` support | ❌ | ❌ | ❌ | opt-in |

> Note / 说明：for other projects, cells are intentionally phrased conservatively unless the capability is explicitly documented. If you need a strict directive-by-directive compatibility matrix, verify each item against source code or tests first.

## Compatibility Scope / 兼容范围

`ffcue` is designed to handle common CUE directives used in real album images, including metadata fields, `FILE` blocks, track definitions, indices, gaps, flags, and remarks.

`ffcue` 面向真实专辑镜像场景，支持常见的 CUE 指令，包括元数据字段、`FILE` 块、轨道定义、索引、间隙、标志以及备注。

When documenting support for other parsers, prefer source links or test cases over broad checkmark tables.

在描述其他解析器支持情况时，建议优先引用源码或测试，而不是直接给出大而全的打勾表。

## Acknowledgements / 致谢

This project was designed and implemented with reference to the following open-source projects:

- rcue — https://github.com/gyng/rcue
- cuna — https://github.com/snylonue/cuna
- cue / libcue — https://github.com/lipnitsk/libcue

These projects were useful references for parsing workflows, model design, and edge-case handling.

本项目在解析流程、数据结构设计与边界情况处理方面参考了上述项目。

## License / 许可证

MIT / Apache-2.0
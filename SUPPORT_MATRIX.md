# CUE Parser Support Matrix (Verified by Source Code)

This document provides a detailed, instruction-by-instruction compatibility matrix for `ffcue` compared to other popular Rust CUE parsers (`rcue`, `cuna`, `cue`/`libcue`). 

*Note: This matrix focuses on actual parser behavior, fallback strategies, and encoding tolerance based on source code implementations and test suites.*

## 1. Top-Level Directives (Metadata)

| Directive | `ffcue` | `rcue` | `cuna` | `cue` (`libcue`) | Notes |
|-----------|---------|--------|--------|------------------|-------|
| `CATALOG` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | 13-digit EAN/UPC identifier. |
| `CDTEXTFILE` | ✅ Supported | ❌ Ignored | ❌ Ignored | ✅ Supported | Points to a CD-Text file. `ffcue` captures it via `parser::parse_cue_text`. |
| `PERFORMER` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Sets top-level performer. |
| `SONGWRITER` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Sets top-level songwriter. |
| `TITLE` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Sets top-level album title. |
| `REM` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Parses arbitrary key-value metadata (e.g., `REM DATE 2000`, `REM GENRE Pop`). |

## 2. File & Track Formatting

| Directive | `ffcue` | `rcue` | `cuna` | `cue` (`libcue`) | Notes |
|-----------|---------|--------|--------|------------------|-------|
| `FILE` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | `ffcue` supports subsequent path resolution (`src/resolver.rs`), extracting both filename and type. |
| `TRACK` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Numeric IDs and track type (AUDIO, MODE1/2352, etc). |

## 3. Track-Level Directives

| Directive | `ffcue` | `rcue` | `cuna` | `cue` (`libcue`) | Notes |
|-----------|---------|--------|--------|------------------|-------|
| `FLAGS` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Track flags (e.g., `DCP`, `PRE`, `4CH`). |
| `ISRC` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Track-level standard recording code. |
| `PERFORMER` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Track-level performer. |
| `SONGWRITER` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Track-level songwriter. |
| `TITLE` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Track-level title. |
| `PREGAP` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Gap length preceding the track. |
| `POSTGAP` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Gap length succeeding the track. |
| `INDEX` | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Parses index numbers (00, 01, etc.) and timestamps (MM:SS:FF). |
| `REM` | ✅ Supported | ⚠️ Partially | ⚠️ Partially | ✅ Supported | `ffcue` associates track-specific `REM` blocks with the exact track. |

## 4. TimeStamp Processing & Tolerance

| Capability | `ffcue` | `rcue` | `cuna` | `cue` (`libcue`) | Notes |
|------------|---------|--------|--------|------------------|-------|
| Frames `>74` | ✅ Clamp to `74` | ❌ Fails | ❌ Fails | ✅ Supported | `ffcue` (`src/parser.rs::parse_timestamp`) gracefully handles bad CD rips. |
| Seconds `>59` | ✅ Clamp to `59` | ❌ Fails | ❌ Fails | ✅ Supported | Clamps out-of-spec timestamp strings without terminating parser. |
| Time Conversions | ✅ Built-in | ❌ Basic | ✅ Built-in | ✅ Built-in | Millis & frames calculation built into `CueTimestamp` (`src/models.rs`). |

## 5. Decoding & Encoding Detection (`decode_text`)

| Encoding | `ffcue` | `rcue` | `cuna` | `cue` (`libcue`) | Notes |
|----------|---------|--------|--------|------------------|-------|
| UTF-8 (No BOM) | ✅ Supported | ✅ Supported | ✅ Supported | ✅ Supported | Standard fallback. |
| UTF-8 (BOM) | ✅ Stripped | ⚠️ OS/Impl | ✅ Stripped | ⚠️ Dependent | `ffcue` explicitly strips `EF BB BF`. |
| UTF-16 (LE/BE) | ✅ Auto-decode | ❌ Fails | ❌ Fails | ❌ Fails | Native support via `FF FE` and `FE FF` BOMs without user configuration. |
| Shift-JIS / GBK / Big5 | ✅ `chardetng` | ❌ Fails | ❌ Fails | ❌ Fails | `ffcue` fallback automatically detects and decodes CJK historical archives. |

## 6. File Path Resolution (`resolver.rs`)

| Capability | `ffcue` | `rcue` | `cuna` | `cue` (`libcue`) | Notes |
|------------|---------|--------|--------|------------------|-------|
| Fuzzy Matching | ✅ Supported | ❌ Built-in | ❌ Built-in | ❌ Built-in | Disk scanning fallback matching stem/case/extension. |
| Path Normalization | ✅ Supported | ⚠️ Limited | ⚠️ Limited | ⚠️ Limited | Overcomes Windows `\` paths directly within CUE blocks. |

## Conclusion

While most CUE parsers effectively parse properly formatted ASCII/UTF-8 CUE sheets, `ffcue` heavily differentiates itself via **graceful degradation** and **archival robustness**. Rather than strictly rejecting malformed CUEs (such as out-of-range bounds, Windows separators, or Shift-JIS encodings), `ffcue` assumes real-world music archival scenarios and automatically clamps, corrects, and iteratively matches extensions and names to ensure the metadata stream is always extracted successfully.

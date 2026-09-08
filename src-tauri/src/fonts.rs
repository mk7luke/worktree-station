//! Enumerating installed monospace fonts.
//!
//! Measuring text in a canvas — the usual web trick — reports faces as missing
//! in WKWebView even when they are installed, so the terminal font picker would
//! hide exactly the Nerd Fonts it exists to offer. This reads the family name
//! out of each font file instead, which is unambiguous and works the same on
//! every platform.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Directories the OS loads fonts from, most specific (per-user) first.
fn font_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            out.push(home.join("Library/Fonts"));
        }
        out.push(PathBuf::from("/Library/Fonts"));
        out.push(PathBuf::from("/System/Library/Fonts"));
        out.push(PathBuf::from("/System/Library/Fonts/Supplemental"));
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            out.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
        }
        if let Some(win) = std::env::var_os("WINDIR") {
            out.push(PathBuf::from(win).join("Fonts"));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(home) = dirs::home_dir() {
            out.push(home.join(".fonts"));
            out.push(home.join(".local/share/fonts"));
        }
        out.push(PathBuf::from("/usr/share/fonts"));
        out.push(PathBuf::from("/usr/local/share/fonts"));
    }

    out.into_iter().filter(|d| d.is_dir()).collect()
}

/// How deep to walk a font directory. macOS and Windows keep font files
/// directly in the directory, but Linux nests them by format and family —
/// `/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf` is already three
/// levels down, and some distributions go deeper still.
const MAX_FONT_DEPTH: usize = 6;

fn is_font_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("ttf") | Some("otf") | Some("ttc") | Some("otc")
    )
}

fn be_u16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]))
}

fn be_u32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *b.get(at)?,
        *b.get(at + 1)?,
        *b.get(at + 2)?,
        *b.get(at + 3)?,
    ]))
}

fn read_at(file: &mut File, offset: u64, len: usize) -> Option<Vec<u8>> {
    // A corrupt table can claim an absurd length; refuse to allocate for it.
    if len > 4 * 1024 * 1024 {
        return None;
    }
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf).ok()?;
    Some(buf)
}

struct TableRecord {
    offset: u32,
    length: u32,
}

/// Locate a table within one font of the file, by its four-byte tag.
fn find_table(file: &mut File, font_start: u64, tag: &[u8; 4]) -> Option<TableRecord> {
    let header = read_at(file, font_start, 12)?;
    let num_tables = be_u16(&header, 4)? as usize;
    let directory = read_at(file, font_start + 12, num_tables.checked_mul(16)?)?;
    for i in 0..num_tables {
        let base = i * 16;
        if directory.get(base..base + 4)? == tag {
            return Some(TableRecord {
                offset: be_u32(&directory, base + 8)?,
                length: be_u32(&directory, base + 12)?,
            });
        }
    }
    None
}

/// `post` table, offset 12: non-zero means the face is fixed-pitch.
fn is_monospaced(file: &mut File, font_start: u64) -> bool {
    let Some(post) = find_table(file, font_start, b"post") else {
        return false;
    };
    let Some(bytes) = read_at(file, post.offset as u64, 16) else {
        return false;
    };
    be_u32(&bytes, 12).unwrap_or(0) != 0
}

/// The human-readable family name, preferring the typographic family (name ID
/// 16) so "FiraCode Nerd Font" wins over "FiraCode Nerd Font Light".
fn family_name(file: &mut File, font_start: u64) -> Option<String> {
    let table = find_table(file, font_start, b"name")?;
    let bytes = read_at(file, table.offset as u64, table.length as usize)?;

    let count = be_u16(&bytes, 2)? as usize;
    let storage = be_u16(&bytes, 4)? as usize;

    let mut fallback: Option<String> = None;
    for i in 0..count {
        let rec = 6 + i * 12;
        let platform = be_u16(&bytes, rec)?;
        let encoding = be_u16(&bytes, rec + 2)?;
        let name_id = be_u16(&bytes, rec + 6)?;
        if name_id != 1 && name_id != 16 {
            continue;
        }
        let len = be_u16(&bytes, rec + 8)? as usize;
        let off = be_u16(&bytes, rec + 10)? as usize;
        let raw = bytes.get(storage + off..storage + off + len)?;

        let decoded = match (platform, encoding) {
            // Windows platform: UTF-16BE.
            (3, _) => {
                let units: Vec<u16> = raw
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
                    .collect();
                String::from_utf16(&units).ok()?
            }
            // Macintosh Roman: ASCII for every name we care about.
            (1, 0) => raw.iter().map(|&b| b as char).collect(),
            _ => continue,
        };
        let decoded = decoded.trim().to_owned();
        if decoded.is_empty() {
            continue;
        }
        if name_id == 16 {
            return Some(decoded);
        }
        fallback.get_or_insert(decoded);
    }
    fallback
}

/// Every font packed into one file: a plain font has one, a `.ttc` collection
/// lists several.
fn font_offsets(file: &mut File) -> Vec<u64> {
    let Some(head) = read_at(file, 0, 12) else {
        return vec![];
    };
    if &head[0..4] != b"ttcf" {
        return vec![0];
    }
    let Some(num_fonts) = be_u32(&head, 8) else {
        return vec![];
    };
    let num_fonts = num_fonts.min(64) as usize;
    let Some(table) = read_at(file, 12, num_fonts * 4) else {
        return vec![];
    };
    (0..num_fonts)
        .filter_map(|i| be_u32(&table, i * 4).map(u64::from))
        .collect()
}

/// Family names of every fixed-pitch font installed on this machine, sorted.
pub fn monospace_families() -> Vec<String> {
    let mut families: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for dir in font_dirs() {
        let entries = walkdir::WalkDir::new(&dir)
            .max_depth(MAX_FONT_DEPTH)
            .into_iter()
            .filter_map(Result::ok);

        for entry in entries {
            let path = entry.path();
            if !entry.file_type().is_file() || !is_font_file(path) {
                continue;
            }
            let Ok(mut file) = File::open(path) else {
                continue;
            };
            for start in font_offsets(&mut file) {
                if !is_monospaced(&mut file, start) {
                    continue;
                }
                if let Some(name) = family_name(&mut file, start) {
                    // A leading dot marks a system-private face the user is
                    // not meant to pick.
                    if name.starts_with('.') {
                        continue;
                    }
                    if seen.insert(name.to_lowercase()) {
                        families.push(name);
                    }
                }
            }
        }
    }

    families.sort_by_key(|f| f.to_lowercase());
    families
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walk without a depth limit, so the test can tell "this machine has no
    /// fonts" apart from "the walker never reached them".
    fn font_files_exist_anywhere() -> bool {
        font_dirs().iter().any(|dir| {
            walkdir::WalkDir::new(dir)
                .into_iter()
                .filter_map(Result::ok)
                .any(|e| e.file_type().is_file() && is_font_file(e.path()))
        })
    }

    #[test]
    fn finds_every_installed_monospace_family() {
        let families = monospace_families();

        // If any font file is reachable at all, the walker must have found
        // families. This is what catches a depth limit set too shallow for the
        // way a platform nests its font directories.
        if font_files_exist_anywhere() {
            assert!(
                !families.is_empty(),
                "font files exist on this machine but none were enumerated — \
                 MAX_FONT_DEPTH is probably too shallow for this platform's layout"
            );
        }

        // Family names, not file names.
        assert!(families
            .iter()
            .all(|f| !f.ends_with(".ttf") && !f.ends_with(".otf")));
        // Sorted and deduplicated.
        let mut sorted = families.clone();
        sorted.sort_by_key(|f| f.to_lowercase());
        assert_eq!(families, sorted, "families are not sorted");
        let mut deduped = families.clone();
        deduped.dedup();
        assert_eq!(families.len(), deduped.len(), "families contain duplicates");

        eprintln!("found {} families: {:?}", families.len(), families);
    }
}

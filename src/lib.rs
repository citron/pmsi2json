use anyhow::{Context, Result, anyhow, bail};
use encoding_rs::WINDOWS_1252;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct ConversionResult {
    pub input: String,
    pub files: Vec<FileResult>,
}

#[derive(Debug, Serialize)]
pub struct FileResult {
    pub path: String,
    pub encoding: String,
    pub format: FileFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
    pub record_count: usize,
    pub records: Vec<Record>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileFormat {
    Delimited,
    WhitespaceSeparated,
    FixedWidth,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Record {
    Fields(BTreeMap<String, String>),
    Structured(StructuredRecord),
}

#[derive(Debug, Serialize)]
pub struct StructuredRecord {
    pub line_number: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_type: Option<String>,
    pub raw: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub fields: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_length: Option<usize>,
}

pub fn convert_path(input: &Path) -> Result<ConversionResult> {
    let files = collect_grp_files(input)?;
    let parsed_files = files
        .iter()
        .map(|path| parse_file(path))
        .collect::<Result<Vec<_>>>()?;

    Ok(ConversionResult {
        input: input.display().to_string(),
        files: parsed_files,
    })
}

pub fn write_json(result: &ConversionResult, output: Option<&Path>, pretty: bool) -> Result<()> {
    let json = if pretty {
        serde_json::to_string_pretty(result)?
    } else {
        serde_json::to_string(result)?
    };

    if let Some(path) = output {
        fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    } else {
        println!("{json}");
    }

    Ok(())
}

fn collect_grp_files(input: &Path) -> Result<Vec<PathBuf>> {
    if input.is_file() {
        return Ok(vec![input.to_path_buf()]);
    }

    if !input.is_dir() {
        bail!("{} is neither a file nor a directory", input.display());
    }

    let mut files = fs::read_dir(input)
        .with_context(|| format!("failed to read {}", input.display()))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && has_grp_extension(path))
        .collect::<Vec<_>>();

    files.sort();

    if files.is_empty() {
        bail!("no .grp files found in {}", input.display());
    }

    Ok(files)
}

fn has_grp_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("grp"))
}

fn parse_file(path: &Path) -> Result<FileResult> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let decoded = decode_bytes(&bytes);
    let lines = decoded
        .text
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let clean = line.trim_end_matches('\r');
            (!clean.trim().is_empty()).then(|| IndexedLine {
                line_number: index + 1,
                raw: clean.to_string(),
            })
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return Err(anyhow!("{} does not contain any records", path.display()));
    }

    let parsed = parse_lines(&lines);

    Ok(FileResult {
        path: path.display().to_string(),
        encoding: decoded.encoding.to_string(),
        format: parsed.format,
        delimiter: parsed.delimiter.map(|delimiter| delimiter.to_string()),
        record_count: parsed.records.len(),
        records: parsed.records,
    })
}

fn parse_lines(lines: &[IndexedLine]) -> ParsedFile {
    if let Some(delimiter) = infer_delimiter(lines) {
        return parse_delimited(lines, delimiter);
    }

    if looks_like_whitespace_separated(lines) {
        return parse_whitespace_separated(lines);
    }

    parse_fixed_width(lines)
}

fn parse_delimited(lines: &[IndexedLine], delimiter: char) -> ParsedFile {
    let split_rows = lines
        .iter()
        .map(|line| (line, split_and_trim(&line.raw, delimiter)))
        .collect::<Vec<_>>();
    let header_row = infer_header_row(&split_rows);

    let records = split_rows
        .into_iter()
        .enumerate()
        .filter_map(|(index, (line, fields))| {
            if header_row.is_some() && index == 0 {
                return None;
            }

            Some(match &header_row {
                Some(headers) => Record::Fields(build_field_map(headers, &fields)),
                None => Record::Structured(StructuredRecord {
                    line_number: line.line_number,
                    record_type: infer_record_type(&line.raw),
                    raw: line.raw.clone(),
                    fields,
                    line_length: None,
                }),
            })
        })
        .collect();

    ParsedFile {
        format: FileFormat::Delimited,
        delimiter: Some(delimiter),
        records,
    }
}

fn parse_whitespace_separated(lines: &[IndexedLine]) -> ParsedFile {
    let records = lines
        .iter()
        .map(|line| {
            Record::Structured(StructuredRecord {
                line_number: line.line_number,
                record_type: infer_record_type(&line.raw),
                raw: line.raw.clone(),
                fields: split_on_multi_space(&line.raw),
                line_length: None,
            })
        })
        .collect();

    ParsedFile {
        format: FileFormat::WhitespaceSeparated,
        delimiter: None,
        records,
    }
}

fn parse_fixed_width(lines: &[IndexedLine]) -> ParsedFile {
    let records = lines
        .iter()
        .map(|line| {
            Record::Structured(StructuredRecord {
                line_number: line.line_number,
                record_type: infer_record_type(&line.raw),
                raw: line.raw.clone(),
                fields: Vec::new(),
                line_length: Some(line.raw.chars().count()),
            })
        })
        .collect();

    ParsedFile {
        format: FileFormat::FixedWidth,
        delimiter: None,
        records,
    }
}

fn decode_bytes(bytes: &[u8]) -> DecodedContent {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return DecodedContent {
            encoding: "utf-8",
            text: text.to_string(),
        };
    }

    let (text, _, _) = WINDOWS_1252.decode(bytes);
    DecodedContent {
        encoding: "windows-1252",
        text: text.into_owned(),
    }
}

fn infer_delimiter(lines: &[IndexedLine]) -> Option<char> {
    ['|', ';', '\t', ',']
        .into_iter()
        .filter_map(|delimiter| {
            let counts = lines
                .iter()
                .take(50)
                .map(|line| line.raw.matches(delimiter).count())
                .filter(|count| *count > 0)
                .collect::<Vec<_>>();

            if counts.len() < 2 {
                return None;
            }

            let mut frequencies = HashMap::new();
            for count in counts.iter().copied() {
                *frequencies.entry(count).or_insert(0usize) += 1;
            }

            let (most_common_count, most_common_frequency) = frequencies
                .into_iter()
                .max_by_key(|(count, frequency)| (*frequency, *count))?;

            (most_common_frequency >= 2).then_some((
                delimiter,
                most_common_frequency,
                counts.len(),
                most_common_count,
            ))
        })
        .max_by_key(|(_, mode_frequency, support, field_count)| {
            (*mode_frequency, *support, *field_count)
        })
        .map(|(delimiter, _, _, _)| delimiter)
}

fn looks_like_whitespace_separated(lines: &[IndexedLine]) -> bool {
    let field_counts = lines
        .iter()
        .take(50)
        .map(|line| split_on_multi_space(&line.raw).len())
        .filter(|count| *count > 1)
        .collect::<Vec<_>>();

    if field_counts.len() < 2 {
        return false;
    }

    let mut frequencies = HashMap::new();
    for count in field_counts {
        *frequencies.entry(count).or_insert(0usize) += 1;
    }

    frequencies.values().copied().max().unwrap_or_default() >= 2
}

fn split_and_trim(line: &str, delimiter: char) -> Vec<String> {
    line.split(delimiter)
        .map(|value| value.trim().to_string())
        .collect()
}

fn split_on_multi_space(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut spaces = 0usize;

    for ch in line.chars() {
        if ch == ' ' {
            spaces += 1;
            if spaces == 1 {
                current.push(ch);
            }
            continue;
        }

        if spaces > 1 {
            let value = current.trim();
            if !value.is_empty() {
                fields.push(value.to_string());
            }
            current.clear();
        }

        if spaces == 1 {
            current.push(' ');
        }

        spaces = 0;
        current.push(ch);
    }

    if spaces > 1 {
        let value = current.trim();
        if !value.is_empty() {
            fields.push(value.to_string());
        }
        current.clear();
    } else if spaces == 1 {
        current.push(' ');
    }

    let tail = current.trim();
    if !tail.is_empty() {
        fields.push(tail.to_string());
    }

    fields
}

fn infer_header_row(rows: &[(&IndexedLine, Vec<String>)]) -> Option<Vec<String>> {
    let (_, first_row) = rows.first()?;
    if first_row.len() < 2 {
        return None;
    }

    if !rows
        .iter()
        .skip(1)
        .take(10)
        .all(|(_, row)| row.len() == first_row.len())
    {
        return None;
    }

    let headers = first_row
        .iter()
        .map(|value| value.trim().to_string())
        .collect::<Vec<_>>();

    let unique_headers = headers.iter().collect::<HashSet<_>>().len() == headers.len();
    let looks_like_header = headers.iter().all(|header| {
        !header.is_empty()
            && header.chars().any(|ch| ch.is_ascii_alphabetic())
            && !header.chars().all(|ch| ch.is_ascii_digit())
    });

    (unique_headers && looks_like_header).then_some(headers)
}

fn build_field_map(headers: &[String], values: &[String]) -> BTreeMap<String, String> {
    headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            let value = values.get(index).cloned().unwrap_or_default();
            (header.clone(), value)
        })
        .collect()
}

fn infer_record_type(line: &str) -> Option<String> {
    let token = line
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric())
        .take(4)
        .collect::<String>();

    (!token.is_empty()).then_some(token)
}

struct DecodedContent {
    encoding: &'static str,
    text: String,
}

struct IndexedLine {
    line_number: usize,
    raw: String,
}

struct ParsedFile {
    format: FileFormat,
    delimiter: Option<char>,
    records: Vec<Record>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parses_delimited_file_with_header() {
        let lines = vec![
            IndexedLine {
                line_number: 1,
                raw: "finess;rss;ghm".to_string(),
            },
            IndexedLine {
                line_number: 2,
                raw: "010000000;123456789;28Z".to_string(),
            },
        ];

        let parsed = parse_lines(&lines);
        assert!(matches!(parsed.format, FileFormat::Delimited));
        assert_eq!(parsed.delimiter, Some(';'));

        let Record::Fields(fields) = &parsed.records[0] else {
            panic!("expected fields record");
        };

        assert_eq!(fields.get("finess"), Some(&"010000000".to_string()));
        assert_eq!(fields.get("ghm"), Some(&"28Z".to_string()));
    }

    #[test]
    fn parses_whitespace_separated_records() {
        let lines = vec![
            IndexedLine {
                line_number: 1,
                raw: "RSS001  010000000  28Z".to_string(),
            },
            IndexedLine {
                line_number: 2,
                raw: "RSS002  010000001  14C".to_string(),
            },
        ];

        let parsed = parse_lines(&lines);
        assert!(matches!(parsed.format, FileFormat::WhitespaceSeparated));

        let Record::Structured(record) = &parsed.records[0] else {
            panic!("expected structured record");
        };

        assert_eq!(record.fields, vec!["RSS001", "010000000", "28Z"]);
    }

    #[test]
    fn falls_back_to_fixed_width_records() {
        let lines = vec![
            IndexedLine {
                line_number: 1,
                raw: "01000000012345678928Z".to_string(),
            },
            IndexedLine {
                line_number: 2,
                raw: "01000000112345678014C".to_string(),
            },
        ];

        let parsed = parse_lines(&lines);
        assert!(matches!(parsed.format, FileFormat::FixedWidth));

        let Record::Structured(record) = &parsed.records[0] else {
            panic!("expected structured record");
        };

        assert_eq!(record.line_length, Some(21));
        assert!(record.fields.is_empty());
    }

    #[test]
    fn collects_only_grp_files_from_a_directory() {
        let dir = tempdir().expect("tempdir");
        let grp_path = dir.path().join("sample.grp");
        let txt_path = dir.path().join("sample.txt");

        fs::write(&grp_path, "finess;rss\n1;2\n").expect("write grp");
        fs::write(&txt_path, "ignore me").expect("write txt");

        let result = convert_path(dir.path()).expect("convert directory");
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, grp_path.display().to_string());
    }
}

use crate::ffmpeg::{self, FfmpegPaths};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Cue {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub sign: bool,
    pub italic: bool,
    pub top: bool,
}

type Cache = Mutex<HashMap<(String, u32), Arc<Vec<Cue>>>>;
static CACHE: OnceLock<Cache> = OnceLock::new();

pub fn cues(paths: &FfmpegPaths, file: &Path, ordinal: u32) -> Result<Arc<Vec<Cue>>, String> {
    let key = (file.to_string_lossy().to_string(), ordinal);
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().unwrap().get(&key) {
        return Ok(hit.clone());
    }
    let out = ffmpeg::command(&paths.ffmpeg)
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(file)
        .args(["-map", &format!("0:s:{ordinal}"), "-c:s", "ass", "-f", "ass", "-"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let parsed = Arc::new(parse_ass(&String::from_utf8_lossy(&out.stdout)));
    let mut guard = cache.lock().unwrap();
    if guard.len() > 8 {
        guard.clear();
    }
    guard.insert(key, parsed.clone());
    Ok(parsed)
}

fn parse_time(t: &str) -> Option<i64> {
    let mut parts = t.trim().split(':');
    let h: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let s: f64 = parts.next()?.parse().ok()?;
    Some(h * 3_600_000 + m * 60_000 + (s * 1000.0).round() as i64)
}

fn clean_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0u32;
    for c in raw.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.replace("\\N", "\n").replace("\\n", "\n").replace("\\h", "\u{a0}").trim().to_string()
}

fn looks_like_sign(style: &str, raw_text: &str) -> bool {
    let style = style.to_ascii_lowercase();
    let style_is_sign = ["sign", "song", "kara", "title", "note", "insert", "op", "ed", "lyrics", "credit"]
        .iter()
        .any(|k| style.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| w == *k) || style.starts_with(k));
    let text = raw_text.to_ascii_lowercase();
    let positioned = text.contains("\\pos(") || text.contains("\\move(") || text.contains("\\org(");
    let karaoke = text.contains("\\k") || text.contains("\\kf") || text.contains("\\ko");
    let drawing = text.contains("\\p1") || text.contains("\\p2") || text.contains("\\p4");
    style_is_sign || positioned || karaoke || drawing
}

pub fn parse_ass(ass: &str) -> Vec<Cue> {
    let mut fields: Vec<String> = Vec::new();
    let mut in_events = false;
    let mut cues = Vec::new();
    for line in ass.lines() {
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.starts_with('[') {
            in_events = line.eq_ignore_ascii_case("[events]");
            continue;
        }
        if !in_events {
            continue;
        }
        if let Some(rest) = line.strip_prefix("Format:") {
            fields = rest.split(',').map(|f| f.trim().to_ascii_lowercase()).collect();
            continue;
        }
        let Some(rest) = line.strip_prefix("Dialogue:") else { continue };
        if fields.is_empty() {
            continue;
        }
        let values: Vec<&str> = rest.splitn(fields.len(), ',').collect();
        if values.len() != fields.len() {
            continue;
        }
        let get = |name: &str| fields.iter().position(|f| f == name).map(|i| values[i].trim());
        let (Some(start), Some(end), Some(raw)) = (get("start"), get("end"), fields.iter().position(|f| f == "text"))
        else {
            continue;
        };
        let raw_text = values[raw];
        let (Some(start_ms), Some(end_ms)) = (parse_time(start), parse_time(end)) else { continue };
        let text = clean_text(raw_text);
        if text.is_empty() || end_ms <= start_ms {
            continue;
        }
        let style = get("style").unwrap_or("").to_ascii_lowercase();
        let lower = raw_text.to_ascii_lowercase();
        let sign = looks_like_sign(&style, raw_text);
        let italic = style.contains("ital") || lower.contains("\\i1");
        let top = style == "top" || ["\\an7", "\\an8", "\\an9"].iter().any(|t| lower.contains(t));
        cues.push(Cue { start_ms, end_ms, text, sign, italic, top });
    }
    cues.sort_by_key(|c| (c.start_ms, c.end_ms));
    cues.dedup_by(|a, b| a.start_ms == b.start_ms && a.end_ms == b.end_ms && a.text == b.text);
    cues
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[Script Info]\nTitle: x\n\n[Events]\n\
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
Dialogue: 0,0:00:01.50,0:00:03.00,Default,,0,0,0,,Olá, {\\i1}Subaru{\\i0}!\\NTudo bem?\n\
Dialogue: 0,0:00:01.50,0:00:03.00,Default,,0,0,0,,Olá, {\\i1}Subaru{\\i0}!\\NTudo bem?\n\
Dialogue: 0,0:00:02.00,0:00:04.00,Sign,,0,0,0,,{\\pos(320,50)}Mercado\n\
Dialogue: 0,0:01:00.00,0:01:02.25,Main,,0,0,0,,{\\an8}Fala em cima\n";

    #[test]
    fn parses_dialogue_and_flags_signs() {
        let cues = parse_ass(SAMPLE);
        assert_eq!(cues.len(), 3, "duplicada em outra camada sai");
        assert_eq!(cues[0].text, "Olá, Subaru!\nTudo bem?");
        assert_eq!((cues[0].start_ms, cues[0].end_ms), (1500, 3000));
        assert!(!cues[0].sign && cues[0].italic, "\\i1 no meio marca itálico");
        assert!(cues[1].sign, "estilo Sign + \\pos = letreiro");
        assert_eq!(cues[2].start_ms, 60_000);
        assert_eq!(cues[2].end_ms, 62_250);
        assert!(!cues[2].sign, "\\an8 (fala em cima) continua sendo diálogo");
        assert!(cues[2].top);
    }
}

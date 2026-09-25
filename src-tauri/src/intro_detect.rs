//! Detecção local de abertura/encerramento, pra completar o AniSkip (que é
//! colaborativo e tem muito buraco). Duas fontes, nessa ordem:
//!
//! 1. Capítulos do arquivo ("Opening", "Ending", "Credits"… — comuns em
//!    release de Blu-ray).
//! 2. Áudio: a abertura e o encerramento tocam a mesma música em todo
//!    episódio da temporada. Tira uma impressão digital do áudio (estilo
//!    Haitsma–Kalker: 32 bits por quadro, do sinal da diferença de energia
//!    entre bandas de frequência vizinhas) do começo e do fim de cada
//!    episódio e procura o trecho em comum com os episódios vizinhos — a
//!    posição muda de episódio pra episódio (depende da cena de abertura),
//!    por isso não dá pra copiar o tempo de um vizinho.
//!
//! Roda em segundo plano, 1 episódio por vez, e grava em `detected_segments`.

use crate::{db, ffmpeg, state::AppState};
use rustfft::{num_complex::Complex, FftPlanner};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager};

const SAMPLE_RATE: u32 = 5512;
const FRAME: usize = 2048;
const HOP: usize = 512;
const BANDS: usize = 33;
const BAND_LOW_HZ: f32 = 300.0;
const BAND_HIGH_HZ: f32 = 2000.0;

/// Janela analisada no começo (abertura) e no fim (encerramento).
const HEAD_SECS: f64 = 8.0 * 60.0;
const TAIL_SECS: f64 = 7.0 * 60.0;
/// Tamanho aceito pra abertura/encerramento (em geral ~90s). Abaixo de 30s
/// costuma ser vinheta de título/eyecatch repetida, não a abertura.
const MIN_SEGMENT_SECS: f64 = 30.0;
const MAX_SEGMENT_SECS: f64 = 150.0;
/// Capítulo "Credits"/"Ending" costuma ir até o fim do arquivo (com a prévia).
const MAX_CHAPTER_SECS: f64 = 180.0;
/// Quantos episódios vizinhos comparar com cada episódio.
const REFERENCES: usize = 2;

fn frame_secs() -> f64 {
    HOP as f64 / SAMPLE_RATE as f64
}

/// Impressão digital de um trecho de áudio: 1 hash de 32 bits por quadro
/// (~93ms) e se o quadro tem som (silêncio casa com qualquer coisa).
pub struct Fingerprint {
    hashes: Vec<u32>,
    loud: Vec<bool>,
}

fn band_edges() -> [usize; BANDS + 1] {
    let mut edges = [0usize; BANDS + 1];
    let ratio = (BAND_HIGH_HZ / BAND_LOW_HZ).powf(1.0 / BANDS as f32);
    for (i, edge) in edges.iter_mut().enumerate() {
        let hz = BAND_LOW_HZ * ratio.powi(i as i32);
        *edge = (hz * FRAME as f32 / SAMPLE_RATE as f32).round() as usize;
    }
    edges
}

pub fn fingerprint(samples: &[f32]) -> Fingerprint {
    if samples.len() < FRAME * 2 {
        return Fingerprint { hashes: Vec::new(), loud: Vec::new() };
    }
    let fft = FftPlanner::<f32>::new().plan_fft_forward(FRAME);
    let window: Vec<f32> = (0..FRAME)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (FRAME - 1) as f32).cos())
        .collect();
    let edges = band_edges();

    let frames = (samples.len() - FRAME) / HOP + 1;
    let mut energies: Vec<[f32; BANDS]> = Vec::with_capacity(frames);
    let mut rms: Vec<f32> = Vec::with_capacity(frames);
    let mut buf = vec![Complex::new(0.0f32, 0.0); FRAME];
    for f in 0..frames {
        let chunk = &samples[f * HOP..f * HOP + FRAME];
        rms.push((chunk.iter().map(|s| s * s).sum::<f32>() / FRAME as f32).sqrt());
        for (b, (s, w)) in buf.iter_mut().zip(chunk.iter().zip(&window)) {
            *b = Complex::new(s * w, 0.0);
        }
        fft.process(&mut buf);
        let mut bands = [0f32; BANDS];
        for (m, band) in bands.iter_mut().enumerate() {
            *band = buf[edges[m]..edges[m + 1].max(edges[m] + 1)].iter().map(|c| c.norm_sqr()).sum();
        }
        energies.push(bands);
    }

    let mut hashes = Vec::with_capacity(frames);
    let mut loud = Vec::with_capacity(frames);
    for n in 0..frames {
        let mut h = 0u32;
        if n > 0 {
            for m in 0..BANDS - 1 {
                let now = energies[n][m] - energies[n][m + 1];
                let before = energies[n - 1][m] - energies[n - 1][m + 1];
                if now - before > 0.0 {
                    h |= 1 << m;
                }
            }
        }
        hashes.push(h);
        // ~ -46 dBFS: abaixo disso é silêncio/quase silêncio.
        loud.push(n > 0 && rms[n] > 0.005);
    }
    Fingerprint { hashes, loud }
}

/// Trecho em comum mais longo entre `a` e `b`, como faixa de quadros em `a`.
pub fn find_common(a: &Fingerprint, b: &Fingerprint) -> Option<(usize, usize)> {
    if a.hashes.is_empty() || b.hashes.is_empty() {
        return None;
    }
    let mut index: HashMap<u32, Vec<usize>> = HashMap::new();
    for (j, (&h, &loud)) in b.hashes.iter().zip(&b.loud).enumerate() {
        if loud {
            index.entry(h).or_default().push(j);
        }
    }

    // Vota no deslocamento (i - j) de hashes iguais ou a 1 bit de diferença.
    let mut votes: HashMap<i64, u32> = HashMap::new();
    for (i, (&h, &loud)) in a.hashes.iter().zip(&a.loud).enumerate() {
        if !loud {
            continue;
        }
        for variant in std::iter::once(h).chain((0..32).map(|k| h ^ (1 << k))) {
            if let Some(js) = index.get(&variant) {
                for &j in js {
                    *votes.entry(i as i64 - j as i64).or_default() += 1;
                }
            }
        }
    }
    let mut offsets: Vec<(i64, u32)> = votes.into_iter().filter(|&(_, v)| v >= 10).collect();
    offsets.sort_by(|x, y| y.1.cmp(&x.1));

    let mut best: Option<(usize, usize)> = None;
    for &(d, _) in offsets.iter().take(5) {
        if let Some((s, e)) = longest_run(a, b, d) {
            if best.is_none_or(|(bs, be)| e - s > be - bs) {
                best = Some((s, e));
            }
        }
    }
    best
}

/// Maior sequência de quadros parecidos com `b` deslocado de `d` quadros,
/// tolerando falhas curtas (~1s).
fn longest_run(a: &Fingerprint, b: &Fingerprint, d: i64) -> Option<(usize, usize)> {
    const WINDOW: i64 = 4;
    const MAX_AVG_BITS: f32 = 10.0;
    const MAX_GAP: usize = 12;

    let n = a.hashes.len();
    // Distância por quadro (None = sem som / fora de `b`), aceitando ±1
    // quadro de desalinhamento.
    let dist: Vec<Option<u32>> = (0..n)
        .map(|i| {
            if !a.loud[i] {
                return None;
            }
            (-1..=1)
                .filter_map(|dd| {
                    let j = i as i64 - d + dd;
                    (j >= 0 && (j as usize) < b.hashes.len() && b.loud[j as usize])
                        .then(|| (a.hashes[i] ^ b.hashes[j as usize]).count_ones())
                })
                .min()
        })
        .collect();

    let good: Vec<bool> = (0..n as i64)
        .map(|i| {
            let window: Vec<u32> = (i - WINDOW..=i + WINDOW)
                .filter(|&k| k >= 0 && k < n as i64)
                .filter_map(|k| dist[k as usize])
                .collect();
            window.len() >= 3 && (window.iter().sum::<u32>() as f32 / window.len() as f32) <= MAX_AVG_BITS
        })
        .collect();

    let mut best: Option<(usize, usize)> = None;
    let mut start: Option<usize> = None;
    let mut last_good = 0usize;
    let mut count = 0usize;
    for (i, &g) in good.iter().enumerate() {
        if g {
            if start.is_none() {
                start = Some(i);
                count = 0;
            }
            last_good = i;
            count += 1;
        } else if let Some(s) = start {
            if i - last_good > MAX_GAP {
                keep_best(&mut best, s, last_good, count);
                start = None;
            }
        }
    }
    if let Some(s) = start {
        keep_best(&mut best, s, last_good, count);
    }
    best
}

fn keep_best(best: &mut Option<(usize, usize)>, s: usize, e: usize, good_frames: usize) {
    let len = e + 1 - s;
    // Maioria dos quadros tem que casar de verdade, não só "passar" nas falhas.
    if good_frames * 2 < len {
        return;
    }
    if best.is_none_or(|(bs, be)| len > be + 1 - bs) {
        *best = Some((s, e));
    }
}

/// Faixa de quadros → ms absolutos, se tiver tamanho de abertura/encerramento.
fn to_segment(range: (usize, usize), window_start_secs: f64) -> Option<(i64, i64)> {
    let start = window_start_secs + range.0 as f64 * frame_secs();
    let end = window_start_secs + (range.1 + 1) as f64 * frame_secs() + FRAME as f64 / SAMPLE_RATE as f64;
    let len = end - start;
    (MIN_SEGMENT_SECS..=MAX_SEGMENT_SECS)
        .contains(&len)
        .then(|| ((start * 1000.0).round() as i64, (end * 1000.0).round() as i64))
}

// ---------------------------------------------------------------------------
// Arquivo: decodificação do áudio, capítulos.

fn decode(ffmpeg_exe: &Path, file: &Path, start: f64, len: f64) -> Result<Vec<f32>, String> {
    let out = ffmpeg::command(ffmpeg_exe)
        .args(["-v", "error", "-nostdin", "-ss", &format!("{start:.3}"), "-t", &format!("{len:.3}"), "-i"])
        .arg(file)
        .args(["-map", "0:a:0", "-ac", "1", "-ar", &SAMPLE_RATE.to_string(), "-f", "s16le", "-"])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(out
        .stdout
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect())
}

#[derive(serde::Deserialize)]
struct ChaptersProbe {
    #[serde(default)]
    chapters: Vec<Chapter>,
}

#[derive(serde::Deserialize)]
struct Chapter {
    start_time: String,
    end_time: String,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Default, Clone, Copy)]
struct Found {
    intro: Option<(i64, i64)>,
    ending: Option<(i64, i64)>,
}

fn chapter_segments(ffprobe: &Path, file: &Path) -> Found {
    let Ok(out) = ffmpeg::command(ffprobe)
        .args(["-v", "error", "-show_chapters", "-of", "json"])
        .arg(file)
        .output()
    else {
        return Found::default();
    };
    let Ok(probe) = serde_json::from_slice::<ChaptersProbe>(&out.stdout) else {
        return Found::default();
    };
    let intro_re = regex::Regex::new(r"(?i)^\s*(op|opening|intro|abertura)\b").unwrap();
    let ending_re = regex::Regex::new(r"(?i)^\s*(ed|ending|credits|outro|encerramento)\b").unwrap();
    let mut found = Found::default();
    for c in probe.chapters {
        let title = c.tags.get("title").map(String::as_str).unwrap_or("");
        let (Ok(start), Ok(end)) = (c.start_time.parse::<f64>(), c.end_time.parse::<f64>()) else {
            continue;
        };
        // Marcador da Crunchyroll vira capítulo "Intro" até em episódio sem
        // abertura (só a vinheta do título, 2–12s) — fora do tamanho de uma
        // abertura/encerramento, ignora e deixa pro áudio.
        if !(MIN_SEGMENT_SECS..=MAX_CHAPTER_SECS).contains(&(end - start)) {
            continue;
        }
        let range = ((start * 1000.0).round() as i64, (end * 1000.0).round() as i64);
        if found.intro.is_none() && intro_re.is_match(title) {
            found.intro = Some(range);
        } else if found.ending.is_none() && ending_re.is_match(title) {
            found.ending = Some(range);
        }
    }
    found
}

struct EpisodeAudio {
    head: Fingerprint,
    tail: Fingerprint,
    tail_start_secs: f64,
}

fn analyze_audio(paths: &ffmpeg::FfmpegPaths, file: &Path) -> Result<EpisodeAudio, String> {
    let duration = crate::media_file::probe(&paths.ffprobe, file)?
        .duration_secs()
        .ok_or_else(|| "no duration".to_string())?;
    let head = fingerprint(&decode(&paths.ffmpeg, file, 0.0, HEAD_SECS.min(duration))?);
    let tail_start_secs = (duration - TAIL_SECS).max(0.0);
    let tail = fingerprint(&decode(&paths.ffmpeg, file, tail_start_secs, duration - tail_start_secs)?);
    Ok(EpisodeAudio { head, tail, tail_start_secs })
}

/// Maior trecho em comum com os vizinhos, já no tamanho aceito.
fn best_against(
    target: &Fingerprint,
    others: &[&Fingerprint],
    window_start_secs: f64,
) -> Option<(i64, i64)> {
    others
        .iter()
        .filter_map(|other| find_common(target, other))
        .filter_map(|range| to_segment(range, window_start_secs))
        .max_by_key(|(s, e)| e - s)
}

// ---------------------------------------------------------------------------
// Fila em segundo plano.

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Analisa em segundo plano os episódios baixados que ainda não foram
/// analisados (ou trocaram de arquivo). Não faz nada se já estiver rodando.
pub fn spawn_pending(app: &AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        run(&app).await;
        RUNNING.store(false, Ordering::SeqCst);
    });
}

struct Candidate {
    number: i64,
    path: PathBuf,
}

async fn run(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(settings) = db::settings::get_all(&state.db).await else { return };
    if settings.get("detect_segments").map(String::as_str) == Some("0") {
        return;
    }
    let Ok(episodes) = db::episodes::list_available(&state.db).await else { return };
    let Ok(analyzed) = db::skip_segments::detected_paths(&state.db).await else { return };

    let mut by_watch: HashMap<i64, Vec<Candidate>> = HashMap::new();
    for ep in episodes {
        let (Some(number), Some(path)) = (ep.episode_number, ep.item_path.or(ep.save_path)) else {
            continue;
        };
        let path = PathBuf::from(path);
        if path.is_file() {
            by_watch.entry(ep.watch_id).or_default().push(Candidate { number, path });
        }
    }
    // Precisa de pelo menos 2 episódios pra comparar; pendente = sem análise
    // ou analisado com outro arquivo.
    let work: Vec<(i64, Vec<Candidate>, HashSet<i64>)> = by_watch
        .into_iter()
        .filter(|(_, eps)| eps.len() >= 2)
        .filter_map(|(watch_id, mut eps)| {
            eps.sort_by_key(|e| e.number);
            let pending: HashSet<i64> = eps
                .iter()
                .filter(|e| analyzed.get(&(watch_id, e.number)).map(String::as_str) != e.path.to_str())
                .map(|e| e.number)
                .collect();
            (!pending.is_empty()).then_some((watch_id, eps, pending))
        })
        .collect();
    if work.is_empty() {
        return;
    }

    let paths = match ffmpeg::ensure_installed(app, &state.http).await {
        Ok(p) => p,
        Err(e) => {
            state.activity.error(tr!("Detecção de abertura: {e}", "Intro detection: {e}"));
            return;
        }
    };

    for (watch_id, eps, pending) in work {
        let title = db::watches::get(&state.db, watch_id).await.map(|w| w.title).unwrap_or_default();
        let mut audio: HashMap<i64, Option<EpisodeAudio>> = HashMap::new();
        let mut found_count = 0usize;

        for target in eps.iter().filter(|e| pending.contains(&e.number)) {
            let chapters = {
                let (ffprobe, file) = (paths.ffprobe.clone(), target.path.clone());
                tauri::async_runtime::spawn_blocking(move || chapter_segments(&ffprobe, &file))
                    .await
                    .unwrap_or_default()
            };
            let mut found = chapters;

            if found.intro.is_none() || found.ending.is_none() {
                // Vizinhos mais próximos em número de episódio.
                let mut refs: Vec<&Candidate> = eps.iter().filter(|e| e.number != target.number).collect();
                refs.sort_by_key(|e| (e.number - target.number).abs());
                refs.truncate(REFERENCES);

                for c in std::iter::once(target).chain(refs.iter().copied()) {
                    if !audio.contains_key(&c.number) {
                        let (p, file) = (paths.clone(), c.path.clone());
                        let result = tauri::async_runtime::spawn_blocking(move || analyze_audio(&p, &file))
                            .await
                            .ok()
                            .and_then(Result::ok);
                        audio.insert(c.number, result);
                    }
                }

                if let Some(Some(me)) = audio.get(&target.number) {
                    let others: Vec<&EpisodeAudio> =
                        refs.iter().filter_map(|r| audio.get(&r.number).and_then(Option::as_ref)).collect();
                    if found.intro.is_none() {
                        let heads: Vec<&Fingerprint> = others.iter().map(|o| &o.head).collect();
                        found.intro = best_against(&me.head, &heads, 0.0);
                    }
                    if found.ending.is_none() {
                        let tails: Vec<&Fingerprint> = others.iter().map(|o| &o.tail).collect();
                        found.ending = best_against(&me.tail, &tails, me.tail_start_secs);
                    }
                }
            }

            let from_chapters = chapters.intro.is_some() || chapters.ending.is_some();
            let from_audio = found.intro != chapters.intro || found.ending != chapters.ending;
            let source = match (from_chapters, from_audio) {
                (true, true) => "chapters+audio",
                (true, false) => "chapters",
                (false, true) => "audio",
                (false, false) => "none",
            };
            if found.intro.is_some() || found.ending.is_some() {
                found_count += 1;
            }
            let _ = db::skip_segments::upsert_detected(
                &state.db,
                watch_id,
                target.number,
                found.intro,
                found.ending,
                source,
                &target.path.to_string_lossy(),
            )
            .await;
        }

        if found_count > 0 {
            state.activity.info(tr!(
                "Abertura/encerramento detectados em {found_count} episódio(s): {title}",
                "Opening/ending detected in {found_count} episode(s): {title}"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ruído pseudo-aleatório determinístico (sem crate de rand).
    fn noise(seed: u32, len: usize) -> Vec<f32> {
        let mut x = seed.wrapping_mul(2654435761).max(1);
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x as f32 / u32::MAX as f32 - 0.5) * 0.6
            })
            .collect()
    }

    fn secs(s: f64) -> usize {
        (s * SAMPLE_RATE as f64) as usize
    }

    #[test]
    fn finds_shared_song_at_different_positions() {
        // "Música" de 90s em comum, começando em 40s num episódio e 95s no outro.
        let song = noise(7, secs(90.0));
        let mut a = noise(1, secs(40.0));
        a.extend(&song);
        a.extend(noise(2, secs(60.0)));
        let mut b = noise(3, secs(95.0));
        b.extend(&song);
        b.extend(noise(4, secs(30.0)));

        let (fa, fb) = (fingerprint(&a), fingerprint(&b));
        let range = find_common(&fa, &fb).expect("should find the shared song");
        let (start_ms, end_ms) = to_segment(range, 0.0).expect("song length is in range");
        assert!((start_ms - 40_000).abs() < 1_500, "start {start_ms}");
        assert!((end_ms - 130_000).abs() < 1_500, "end {end_ms}");
    }

    /// Manual, com arquivos de verdade:
    /// TORII_FFMPEG_DIR=... TORII_EPISODES="a.mkv|b.mkv|c.mkv" cargo test real_episodes -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_episodes() {
        let dir = PathBuf::from(std::env::var("TORII_FFMPEG_DIR").unwrap());
        let paths = ffmpeg::FfmpegPaths { ffmpeg: dir.join("ffmpeg.exe"), ffprobe: dir.join("ffprobe.exe") };
        let files: Vec<PathBuf> = std::env::var("TORII_EPISODES").unwrap().split('|').map(PathBuf::from).collect();
        let audio: Vec<EpisodeAudio> = files.iter().map(|f| analyze_audio(&paths, f).unwrap()).collect();
        let fmt = |s: Option<(i64, i64)>| {
            s.map(|(a, b)| format!("{}:{:02}-{}:{:02}", a / 60000, a / 1000 % 60, b / 60000, b / 1000 % 60))
                .unwrap_or_else(|| "-".into())
        };
        for (i, me) in audio.iter().enumerate() {
            let others: Vec<&EpisodeAudio> = audio.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, a)| a).collect();
            let heads: Vec<&Fingerprint> = others.iter().map(|o| &o.head).collect();
            let tails: Vec<&Fingerprint> = others.iter().map(|o| &o.tail).collect();
            println!(
                "{}: intro {} ending {}",
                files[i].file_name().unwrap().to_string_lossy(),
                fmt(best_against(&me.head, &heads, 0.0)),
                fmt(best_against(&me.tail, &tails, me.tail_start_secs)),
            );
        }
    }

    #[test]
    fn unrelated_audio_has_no_segment() {
        let (fa, fb) = (fingerprint(&noise(11, secs(180.0))), fingerprint(&noise(12, secs(180.0))));
        let segment = find_common(&fa, &fb).and_then(|r| to_segment(r, 0.0));
        assert!(segment.is_none());
    }
}

//! Discord Rich Presence: mostra no perfil o que está tocando no player do
//! Torii ("Assistindo <anime>", episódio, capa e tempo restante).
//!
//! O cliente IPC do Discord é síncrono (named pipe) e o Discord pode nem
//! estar aberto — por isso roda numa thread própria que recebe o estado do
//! player por canal, reconecta sozinha e só manda atualização quando algo
//! muda de verdade (o Discord aceita poucas por minuto).

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// Application ID do app "Torii" no Discord Developer Portal — é o nome que
/// aparece no "Assistindo Torii" e no card do perfil.
const CLIENT_ID: &str = "1554307267120078869";

/// Espera entre tentativas de conectar (Discord fechado).
const RECONNECT_EVERY: Duration = Duration::from_secs(20);

/// Diferença de posição acima disso = seek; reenvia pra corrigir o tempo restante.
const SEEK_TOLERANCE_MS: i64 = 3_000;

#[derive(Clone, PartialEq)]
pub struct Meta {
    pub title: String,
    pub episode: String,
    pub cover_url: Option<String>,
    pub anilist_id: Option<i64>,
}

enum Msg {
    Enabled(bool),
    Open(Meta),
    Playback { playing: bool, position_ms: i64, duration_ms: i64 },
    Clear,
}

pub struct Presence {
    tx: Sender<Msg>,
}

impl Presence {
    pub fn start() -> Self {
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || run(rx))
            .expect("thread do Discord");
        Self { tx }
    }

    pub fn set_enabled(&self, enabled: bool) {
        let _ = self.tx.send(Msg::Enabled(enabled));
    }

    /// Episódio novo aberto no player.
    pub fn open(&self, meta: Meta) {
        let _ = self.tx.send(Msg::Open(meta));
    }

    /// Estado atual do player (chamado a cada snapshot — barato, a thread filtra).
    pub fn playback(&self, playing: bool, position_ms: i64, duration_ms: i64) {
        let _ = self.tx.send(Msg::Playback { playing, position_ms, duration_ms });
    }

    /// Player parou/fechou.
    pub fn clear(&self) {
        let _ = self.tx.send(Msg::Clear);
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// O que está no Discord agora: meta + tocando/pausado + início "virtual"
/// do episódio (agora − posição), que só muda com seek.
#[derive(Clone, PartialEq)]
struct Shown {
    meta: Meta,
    playing: bool,
    anchor_ms: i64,
}

fn run(rx: std::sync::mpsc::Receiver<Msg>) {
    if CLIENT_ID == "0" {
        // Sem Application ID configurado: não tenta conectar.
        while rx.recv().is_ok() {}
        return;
    }
    let mut client: Option<DiscordIpcClient> = None;
    let mut last_attempt: Option<Instant> = None;
    let mut enabled = true;
    let mut meta: Option<Meta> = None;
    let mut playback: Option<(bool, i64, i64)> = None;
    // `None` = nada mostrado (ou limpo).
    let mut shown: Option<Shown> = None;
    let mut dirty = false;

    loop {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Msg::Enabled(on)) => {
                enabled = on;
                dirty = true;
            }
            Ok(Msg::Open(m)) => {
                meta = Some(m);
                playback = None;
                dirty = true;
            }
            Ok(Msg::Playback { playing, position_ms, duration_ms }) => {
                playback = Some((playing, position_ms, duration_ms));
            }
            Ok(Msg::Clear) => {
                meta = None;
                playback = None;
                dirty = true;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let wanted = match (enabled, &meta, playback) {
            (true, Some(m), Some((playing, position, duration))) if duration > 0 => Some(Shown {
                meta: m.clone(),
                playing,
                anchor_ms: now_ms() - position,
            }),
            _ => None,
        };
        let changed = match (&shown, &wanted) {
            (None, None) => false,
            (Some(a), Some(b)) => {
                a.meta != b.meta || a.playing != b.playing || (b.playing && (a.anchor_ms - b.anchor_ms).abs() > SEEK_TOLERANCE_MS)
            }
            _ => true,
        };
        if !changed && !dirty {
            continue;
        }
        if wanted.is_none() && shown.is_none() {
            // Nada pra mostrar nem pra limpar — não precisa nem conectar.
            dirty = false;
            continue;
        }

        if client.is_none() {
            if last_attempt.is_some_and(|t| t.elapsed() < RECONNECT_EVERY) {
                continue;
            }
            last_attempt = Some(Instant::now());
            let mut c = DiscordIpcClient::new(CLIENT_ID);
            if let Err(e) = c.connect() {
                eprintln!("[discord] sem conexão: {e}");
                continue;
            }
            eprintln!("[discord] conectado");
            client = Some(c);
        }
        let c = client.as_mut().unwrap();
        let duration = playback.map(|p| p.2).unwrap_or(0);
        let result = match &wanted {
            Some(s) => c.set_activity(activity(s, duration)),
            None => c.clear_activity(),
        }
        // Resposta do Discord: lida sempre (senão o pipe enche) e loga
        // payload recusado — o envio em si "dá certo" mesmo quando recusa.
        .and_then(|()| c.recv())
        .map(|(_, reply)| {
            if reply["evt"] == "ERROR" {
                eprintln!("[discord] recusado: {}", reply["data"]);
            }
        });
        match result {
            Ok(()) => {
                shown = wanted;
                dirty = false;
            }
            Err(e) => {
                eprintln!("[discord] erro ao enviar: {e}");
                // Discord fechou: reconecta depois e reenvia.
                let _ = c.close();
                client = None;
                dirty = true;
            }
        }
    }
    if let Some(mut c) = client {
        let _ = c.clear_activity();
        let _ = c.close();
    }
}

fn activity(s: &Shown, duration_ms: i64) -> Activity<'_> {
    let state = if s.playing {
        s.meta.episode.clone()
    } else {
        tr!("{} · Pausado", "{} · Paused", s.meta.episode)
    };
    let mut a = Activity::new()
        .activity_type(ActivityType::Watching)
        // Lista de membros mostra "Assistindo <anime>" em vez de "Assistindo Torii".
        .status_display_type(StatusDisplayType::Details)
        .details(s.meta.title.as_str())
        .state(state);
    let mut assets = Assets::new().large_text(s.meta.title.as_str());
    if let Some(cover) = &s.meta.cover_url {
        assets = assets.large_image(cover.as_str());
    }
    a = a.assets(assets);
    if s.playing {
        a = a.timestamps(Timestamps::new().start(s.anchor_ms).end(s.anchor_ms + duration_ms));
    }
    if let Some(id) = s.meta.anilist_id {
        a = a.buttons(vec![Button::new(
            tr!("Ver no AniList", "View on AniList"),
            format!("https://anilist.co/anime/{id}"),
        )]);
    }
    a
}

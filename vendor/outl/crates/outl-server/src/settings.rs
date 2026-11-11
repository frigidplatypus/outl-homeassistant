//! Flat settings wire shape + conversions, mirroring the desktop's
//! `settings.rs`. The frontend's `Settings` interface expects this exact
//! shape; internally it maps to / from `outl_config::Config` so the on-disk
//! `config.toml` stays the single source of truth the TUI and CLI also read.
//!
//! The desktop's copy lives in a Tauri crate this server does not depend on,
//! so it is replicated here. The field set and the parse/render helpers are
//! kept byte-for-byte compatible with the desktop so a workspace configured
//! in the desktop app behaves identically in the addon.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use outl_config::{
    BacklinksOrder, Config, DiagramsMode, DisplayCfg, EditorCfg, SyncConfig, SyncTransportKind,
    ThemeCfg, WorkspaceCfg,
};

fn default_diagrams() -> String {
    "pele".to_string()
}

fn parse_diagrams(s: &str) -> DiagramsMode {
    match s {
        "off" => DiagramsMode::Off,
        _ => DiagramsMode::Pele,
    }
}

fn diagrams_str(mode: DiagramsMode) -> String {
    match mode {
        DiagramsMode::Pele => "pele",
        DiagramsMode::Off => "off",
    }
    .to_string()
}

fn parse_transport(s: &str) -> SyncTransportKind {
    match s {
        "file" => SyncTransportKind::File,
        _ => SyncTransportKind::Iroh,
    }
}

fn transport_str(t: SyncTransportKind) -> String {
    match t {
        SyncTransportKind::File => "file",
        SyncTransportKind::Iroh => "iroh",
    }
    .to_string()
}

fn parse_backlinks_order(s: &str) -> BacklinksOrder {
    match s {
        "oldest" => BacklinksOrder::Oldest,
        _ => BacklinksOrder::Newest,
    }
}

fn backlinks_order_str(o: BacklinksOrder) -> String {
    match o {
        BacklinksOrder::Newest => "newest",
        BacklinksOrder::Oldest => "oldest",
    }
    .to_string()
}

fn parse_theme_mode(s: &str) -> outl_config::ThemeMode {
    match s {
        "light" => outl_config::ThemeMode::Light,
        "dark" => outl_config::ThemeMode::Dark,
        _ => outl_config::ThemeMode::Auto,
    }
}

fn theme_mode_str(m: outl_config::ThemeMode) -> String {
    match m {
        outl_config::ThemeMode::Light => "light",
        outl_config::ThemeMode::Dark => "dark",
        outl_config::ThemeMode::Auto => "auto",
    }
    .to_string()
}

/// Flat shape the frontend's `Settings` interface expects.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub last_workspace: Option<PathBuf>,
    pub vim_mode: bool,
    pub theme: String,
    pub theme_dark: String,
    pub theme_mode: String,
    pub font_size: u32,
    pub sync_transport: String,
    pub backlinks_order: String,
    pub reminders_enabled: bool,
    pub reminders_quiet_hours: String,
    #[serde(default = "default_diagrams")]
    pub diagrams: String,
}

impl From<Config> for Settings {
    fn from(c: Config) -> Self {
        let theme_dark = c.theme.dark().to_string();
        let theme_mode = theme_mode_str(c.theme.mode);
        Self {
            last_workspace: c.workspace.last,
            vim_mode: c.editor.vim_mode,
            theme: c.theme.preset,
            theme_dark,
            theme_mode,
            font_size: c.editor.font_size,
            sync_transport: transport_str(c.sync.transport),
            backlinks_order: backlinks_order_str(c.display.backlinks_order),
            reminders_enabled: c.reminders.enabled,
            reminders_quiet_hours: c.reminders.quiet_hours.unwrap_or_default(),
            diagrams: diagrams_str(c.editor.diagrams),
        }
    }
}

impl From<Settings> for Config {
    fn from(s: Settings) -> Self {
        Self {
            // Display-only flag: the on-disk config is the source of truth
            // (the server's DTO does not track it).
            managed: outl_config::managed(),
            workspace: WorkspaceCfg {
                last: s.last_workspace,
            },
            theme: ThemeCfg {
                preset: s.theme,
                preset_dark: Some(s.theme_dark),
                mode: parse_theme_mode(&s.theme_mode),
            },
            editor: EditorCfg {
                vim_mode: s.vim_mode,
                font_size: s.font_size,
                diagrams: parse_diagrams(&s.diagrams),
            },
            calendar: outl_config::CalendarCfg::default(),
            sync: SyncConfig {
                transport: parse_transport(&s.sync_transport),
                relay_url: None,
            },
            tui: outl_config::TuiCfg::default(),
            snapshot: outl_config::SnapshotCfg::default(),
            storage: outl_config::StorageCfg::default(),
            display: DisplayCfg {
                backlinks_order: parse_backlinks_order(&s.backlinks_order),
            },
            assets: outl_config::AssetsCfg::default(),
            backup: outl_config::BackupCfg::default(),
            reminders: outl_config::RemindersCfg {
                enabled: s.reminders_enabled,
                quiet_hours: Some(s.reminders_quiet_hours).filter(|q| !q.trim().is_empty()),
            },
            telemetry: outl_config::TelemetryCfg::default(),
        }
    }
}

/// Load `config.toml` and project to the flat wire shape.
pub fn load() -> Settings {
    outl_config::load().into()
}

/// Overwrite the sections the flat `Settings` does not model with the
/// on-disk values, so a settings write only touches the fields it owns.
fn restore_unmodeled_sections(cfg: &mut Config, on_disk: &Config) {
    cfg.sync.relay_url = on_disk.sync.relay_url.clone();
    cfg.calendar = on_disk.calendar.clone();
    cfg.tui = on_disk.tui.clone();
    cfg.snapshot = on_disk.snapshot.clone();
    cfg.storage = on_disk.storage.clone();
    cfg.display = on_disk.display.clone();
    cfg.assets = on_disk.assets.clone();
    cfg.backup = on_disk.backup.clone();
    cfg.telemetry = on_disk.telemetry.clone();
}

/// Persist `settings`, restoring the unmodeled sections from disk first.
pub fn save(settings: &Settings) -> anyhow::Result<()> {
    let mut cfg: Config = settings.clone().into();
    let on_disk = outl_config::load();
    restore_unmodeled_sections(&mut cfg, &on_disk);
    Ok(outl_config::save(&cfg)?)
}

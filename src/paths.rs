use std::path::PathBuf;

const APP: &str = "orbis";
const LEGACY: &str = "polynia";

fn prefer_app_dir(app: PathBuf, legacy: PathBuf) -> PathBuf {
    if app.exists() || !legacy.exists() {
        app
    } else {
        legacy
    }
}

/// User config directory (`~/.config/orbis`, falling back to `polynia`).
pub fn config_dir() -> Option<PathBuf> {
    let base = dirs::config_dir()?;
    Some(prefer_app_dir(base.join(APP), base.join(LEGACY)))
}

/// User data directory (`~/.local/share/orbis`, falling back to `polynia`).
pub fn data_dir() -> Option<PathBuf> {
    let base = dirs::data_dir()?;
    Some(prefer_app_dir(base.join(APP), base.join(LEGACY)))
}

pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|p| p.join("config.toml"))
}

pub fn mcp_config_file() -> Option<PathBuf> {
    config_dir().map(|p| p.join("mcp.json"))
}

pub fn sessions_dir() -> Option<PathBuf> {
    data_dir().map(|p| p.join("sessions"))
}

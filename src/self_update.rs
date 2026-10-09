use anyhow::{Context, Result};
use reqwest::Client;
use semver::Version;
use tokio::fs;

const REPO: &str = "your-org/orbis"; // Update this to actual repo
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn check_for_update() -> Result<Option<Version>> {
    let client = Client::new();
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");

    let response = client
        .get(&url)
        .header("User-Agent", "orbis")
        .send()
        .await
        .context("Failed to fetch latest release")?;

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch release: {}", response.status());
    }

    let json: serde_json::Value = response.json().await?;
    let tag_name = json["tag_name"]
        .as_str()
        .context("No tag_name in release")?;

    let latest_version = Version::parse(tag_name.trim_start_matches('v'))?;
    let current_version = Version::parse(CURRENT_VERSION)?;

    Ok(if latest_version > current_version {
        Some(latest_version)
    } else {
        None
    })
}

pub async fn self_update() -> Result<()> {
    println!("Checking for updates...");

    if let Some(latest) = check_for_update().await? {
        println!("Update available: v{} -> v{}", CURRENT_VERSION, latest);
        download_and_install(latest.clone()).await?;
        println!("Successfully updated to v{}", latest);
    } else {
        println!("Already at latest version (v{})", CURRENT_VERSION);
    }

    Ok(())
}

async fn download_and_install(version: Version) -> Result<()> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let asset_name = match (os, arch) {
        ("linux", "x86_64") => format!("orbis-v{}-x86_64-unknown-linux-gnu", version),
        ("linux", "aarch64") => format!("orbis-v{}-aarch64-unknown-linux-gnu", version),
        ("macos", "x86_64") => format!("orbis-v{}-x86_64-apple-darwin", version),
        ("macos", "aarch64") => format!("orbis-v{}-aarch64-apple-darwin", version),
        ("windows", "x86_64") => format!("orbis-v{}-x86_64-pc-windows-msvc.exe", version),
        _ => anyhow::bail!("Unsupported platform: {}-{}", os, arch),
    };

    let client = Client::new();
    let download_url = format!(
        "https://github.com/{REPO}/releases/download/v{}/{}",
        version, asset_name
    );

    println!("Downloading {}...", asset_name);
    let response = client
        .get(&download_url)
        .header("User-Agent", "orbis")
        .send()
        .await
        .context("Failed to download release")?;

    if !response.status().is_success() {
        anyhow::bail!("Download failed: {}", response.status());
    }

    let bytes = response.bytes().await?;

    let current_exe = std::env::current_exe().context("Failed to get current exe path")?;
    let backup_path = current_exe.with_extension("bak");

    // Backup current binary
    fs::copy(&current_exe, &backup_path)
        .await
        .context("Failed to backup current binary")?;

    // Write new binary
    fs::write(&current_exe, &bytes)
        .await
        .context("Failed to write new binary")?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&current_exe).await?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&current_exe, perms).await?;
    }

    // Clean up backup
    let _ = fs::remove_file(&backup_path).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        let v1 = Version::parse("0.1.0").unwrap();
        let v2 = Version::parse("0.2.0").unwrap();
        assert!(v2 > v1);

        let v3 = Version::parse("1.0.0").unwrap();
        assert!(v3 > v2);
    }
}

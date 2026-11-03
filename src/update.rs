//! Güncelleme kontrolü: açılışta GitHub'daki son yayınlanmış sürüme bakılır.
//! Taslak sürümler `releases/latest`'te görünmez, yani yayınlanana kadar
//! kullanıcıya duyurulmaz. Hata olursa sessizce geçilir; yalnızca log'a yazılır.

use crate::log::logln;
use serde::Deserialize;

const REPO: &str = "dgmosdev/drive";

pub struct Release {
    pub version: String,
    /// Bu platformun paketini doğrudan indiren bağlantı.
    pub download_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

/// Bu platform için release.yml'deki paket adı.
fn asset_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "DGMOS-Drive-macOS.dmg"
    } else if cfg!(target_os = "windows") {
        "DGMOS-Drive-Windows.zip"
    } else {
        "DGMOS-Drive-Linux-x86_64.tar.gz"
    }
}

/// Yeni bir sürüm varsa döner.
pub async fn check() -> Option<Release> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("dgmos-drive/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let res = client
        .get(format!("https://api.github.com/repos/{REPO}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .inspect_err(|e| logln!("güncelleme kontrolü: {e}"))
        .ok()?;
    if !res.status().is_success() {
        logln!("güncelleme kontrolü: {}", res.status());
        return None;
    }
    let release: GithubRelease = res.json().await.ok()?;
    let version = release.tag_name.trim_start_matches('v').to_owned();
    if !is_newer(&version, env!("CARGO_PKG_VERSION")) {
        return None;
    }
    Some(Release {
        download_url: format!(
            "https://github.com/{REPO}/releases/download/{}/{}",
            release.tag_name,
            asset_name()
        ),
        version,
    })
}

/// `1.10.0` > `1.9.2` gibi sayısal karşılaştırma; ön ek/son ek (`-beta`) yok sayılır.
fn is_newer(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.split(['.', '-'])
            .take(3)
            .map(|n| n.parse().unwrap_or(0))
            .collect()
    };
    parse(candidate) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.10.0", "1.9.2"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("0.9.0", "1.0.0"));
    }
}

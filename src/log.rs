//! Teknik hata kaydı. Uygulama Finder/Dock'tan açıldığında stderr görünmediği
//! için ayrıntılar bir dosyaya da yazılır; kullanıcı menüden açıp destek
//! ekibine iletebilir. Anahtarlar asla yazılmaz.
//!
//! macOS: ~/Library/Logs/DGMOS Drive/drive.log
//! Diğer: <yerel veri klasörü>/DGMOS Drive/drive.log

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 1024 * 1024;

macro_rules! logln {
    ($($arg:tt)*) => { $crate::log::write(format!($($arg)*)) };
}
pub(crate) use logln;

pub fn path() -> Option<PathBuf> {
    let dir = if cfg!(target_os = "macos") {
        dirs::home_dir()?.join("Library/Logs/DGMOS Drive")
    } else {
        dirs::data_local_dir()?.join("DGMOS Drive")
    };
    Some(dir.join("drive.log"))
}

pub fn write(message: String) {
    eprintln!("{message}");
    let Some(path) = path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Basit döndürme: 1 MB'ı aşınca baştan başla.
    let truncate = std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(!truncate)
        .write(true)
        .truncate(truncate)
        .open(&path);
    if let Ok(mut file) = file {
        let _ = writeln!(file, "{} {message}", utc_now());
    }
}

/// "2026-11-02 13:05:09Z" — ek bağımlılık olmadan UTC biçimleme.
fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant'ın civil_from_days algoritması
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

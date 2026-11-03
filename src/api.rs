//! DGMOS API istemcisi.
//!
//! Kimlik doğrulama: frontend'deki "API Anahtarı Güvenlik Standartları (v2.0)"
//! ile aynı — `X-Access-Key`, `X-Timestamp` ve Secret Key ile üretilen
//! HMAC-SHA256 `X-Signature`. İmza metni: `METHOD\nPATH\nTIMESTAMP\nBODY`.
//!
//! Yükleme akışı Dropzone ile aynı:
//!   1. POST /modules/generate-presigned-upload-url-multi
//!   2. Her dosya için presigned URL'e PUT (S3, `x-amz-tagging: status=temp`)
//!   3. POST /modules/confirm-file-upload-multi
//!
//! Hatalar `ApiError` olarak döner; `Display` çıktısı doğrudan son kullanıcıya
//! gösterilecek dildedir. Teknik ayrıntı yalnızca stderr'e yazılır.

use crate::log::logln;
use std::fmt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::TryStreamExt;
use hmac::{Hmac, KeyInit, Mac};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use tokio_util::io::ReaderStream;

#[derive(Debug, Clone)]
pub enum ApiError {
    /// Sunucuya ulaşılamadı (internet yok, zaman aşımı, DNS).
    Network,
    /// Anahtarlar kabul edilmedi. `server` sunucunun ham mesajıdır (destek için gösterilir).
    Unauthorized { reason: AuthReason, server: String },
    /// Dosya bu bilgisayarda okunamadı.
    FileRead,
    /// Dosya depolamaya gönderilemedi.
    Storage,
    /// DGMOS dosyayı kabul etmedi (moderasyon vb.); mesaj sunucudan gelir.
    Rejected(String),
    /// Beklenmeyen sunucu yanıtı.
    Server,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network => write!(
                f,
                "İnternete bağlanılamadı. Bağlantınızı kontrol edip tekrar deneyin."
            ),
            Self::Unauthorized { reason, .. } => write!(f, "{}", reason.message()),
            Self::FileRead => write!(f, "Dosya açılamadı. Taşınmış ya da silinmiş olabilir."),
            Self::Storage => write!(f, "Dosya gönderilemedi. Tekrar deneyin."),
            Self::Rejected(message) => write!(f, "{message}"),
            Self::Server => write!(
                f,
                "DGMOS şu anda yanıt veremiyor. Biraz sonra tekrar deneyin."
            ),
        }
    }
}

impl std::error::Error for ApiError {}

impl ApiError {
    pub fn unauthorized(server: impl Into<String>) -> Self {
        let server = server.into();
        Self::Unauthorized {
            reason: AuthReason::classify(&server),
            server,
        }
    }

    /// Kullanıcıya küçük puntoyla gösterilebilecek ham sunucu mesajı.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Unauthorized { server, .. } if !server.is_empty() => Some(server),
            _ => None,
        }
    }
}

/// Backend'in API anahtarı hataları (frontend API Anahtarları → Hata Kodları
/// sekmesindeki liste) ve oturum hataları.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuthReason {
    /// "Invalid HMAC signature" / "Missing mandatory X-Signature"
    Signature,
    /// "Request expired" — saat 5 dakikadan fazla kaymış.
    Expired,
    /// "route restriction: not allowed" — anahtarın yükleme izni yok.
    Route,
    /// "origin not in allowed list" — Public (web sitesi) anahtarı kullanılmış.
    Origin,
    /// IP kısıtı.
    Ip,
    /// Anahtar iptal edilmiş, pasif ya da süresi dolmuş.
    Revoked,
    /// Erişim anahtarı tanınmadı.
    UnknownKey,
    /// "Oturum gerekli" / "you are not logged in" — endpoint anahtarı hiç kabul etmedi.
    NotAccepted,
    Other,
}

impl AuthReason {
    fn classify(server: &str) -> Self {
        let m = server.to_lowercase();
        let has = |s: &str| m.contains(s);
        if has("expired") && !has("key") {
            Self::Expired
        } else if has("hmac") || has("signature") {
            Self::Signature
        } else if has("route") {
            Self::Route
        } else if has("origin") {
            Self::Origin
        } else if has("ip not") || has("ip address") || has("allowed_ips") || has("ip restriction")
        {
            Self::Ip
        } else if has("revoked") || has("inactive") || has("disabled") || has("expired") {
            Self::Revoked
        } else if has("oturum") || has("not logged in") || has("no_login") {
            Self::NotAccepted
        } else if has("key") && (has("invalid") || has("not found") || has("unknown")) {
            Self::UnknownKey
        } else {
            Self::Other
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Signature => {
                "Gizli anahtar eşleşmedi. Anahtarı eksiksiz kopyalayıp tekrar deneyin."
            }
            Self::Expired => {
                "Bilgisayarınızın saati yanlış görünüyor. Tarih ve saati otomatik ayarlayıp tekrar deneyin."
            }
            Self::Route => {
                "Bu anahtarın dosya yükleme izni yok. DGMOS'ta anahtarın izinlerine dosya yüklemeyi ekleyin."
            }
            Self::Origin => {
                "Bu bir web sitesi (Public) anahtarı. Sunucu (Secret) tipinde bir anahtar kullanın."
            }
            Self::Ip => {
                "Bu anahtar yalnızca belirli IP adreslerinden kullanılabilir. Anahtarın IP kısıtını kontrol edin."
            }
            Self::Revoked => {
                "Bu anahtar iptal edilmiş ya da süresi dolmuş. DGMOS'ta yeni bir anahtar oluşturun."
            }
            Self::UnknownKey => "Erişim anahtarı bulunamadı. Doğru kopyaladığınızdan emin olun.",
            Self::NotAccepted => {
                "DGMOS bu anahtarla girişi kabul etmedi. Anahtarın aktif olduğundan emin olun; \
                 sorun sürerse aşağıdaki sunucu yanıtını destek ekibine iletin."
            }
            Self::Other => "Hesap bağlantınız doğrulanamadı. Anahtarlarınızı kontrol edin.",
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        logln!("ağ hatası: {e}");
        if e.is_decode() || e.is_status() {
            Self::Server
        } else {
            Self::Network
        }
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;

#[derive(Clone)]
pub struct Credentials {
    pub base_url: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Serialize)]
pub struct PresignFile {
    pub file_name: String,
    pub file_size: u64,
    pub content_type: String,
    pub is_public: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PresignItem {
    pub file_name: String,
    pub file_id: i64,
    pub upload_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RejectedFile {
    pub filename: Option<String>,
    pub message: Option<String>,
    pub reason: Option<String>,
}

/// Onay adımının sonucu. Backend moderasyonu reddederse 400/422 ve
/// `rejected_files` döner; bu bir ağ hatası değil, dosya bazında sonuçtur.
pub enum ConfirmOutcome {
    Ok,
    Rejected {
        message: String,
        rejected: Vec<RejectedFile>,
    },
}

pub struct DgmosClient {
    http: Client,
    creds: Credentials,
}

impl DgmosClient {
    pub fn new(creds: Credentials) -> Result<Self> {
        let http = Client::builder()
            .user_agent(concat!("dgmos-drive/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(std::time::Duration::from_secs(15))
            .build()?;
        Ok(Self { http, creds })
    }

    fn endpoint(&self, module: &str) -> Result<Url> {
        let base = self.creds.base_url.trim_end_matches('/');
        Url::parse(&format!("{base}/modules/{module}")).map_err(|e| {
            logln!("geçersiz sunucu adresi {base}: {e}");
            ApiError::Network
        })
    }

    fn sign(&self, method: &str, path: &str, timestamp: &str, body: &str) -> String {
        let payload = format!("{method}\n{path}\n{timestamp}\n{body}");
        let mut mac = Hmac::<Sha256>::new_from_slice(self.creds.secret_key.as_bytes())
            .expect("HMAC her uzunlukta anahtar kabul eder");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// İmzalı JSON POST. İmzalanan gövde ile gönderilen gövde birebir aynı
    /// byte'lar olmalı, bu yüzden JSON bir kez serialize edilip ikisinde de kullanılır.
    /// 401/403 burada `Unauthorized`'a çevrilir.
    async fn post_signed(
        &self,
        module: &str,
        body: &impl Serialize,
    ) -> Result<(StatusCode, Value)> {
        let url = self.endpoint(module)?;
        let body = serde_json::to_string(body).map_err(|_| ApiError::Server)?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
            .to_string();
        let signature = self.sign("POST", url.path(), &timestamp, &body);

        let res = self
            .http
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .header("X-Access-Key", &self.creds.access_key)
            .header("X-Timestamp", &timestamp)
            .header("X-Signature", signature)
            .body(body)
            .send()
            .await?;

        let status = res.status();
        let text = res.text().await?;
        let json = serde_json::from_str(&text).unwrap_or(Value::String(text));
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            let message = server_message(&json);
            logln!("{module}: {status} {message}");
            return Err(ApiError::unauthorized(message));
        }
        Ok((status, json))
    }

    /// Anahtarın yükleme endpoint'ine erişebildiğini boş bir presign isteğiyle
    /// dener; dosya kaydı oluşmaz. 401/403 dışındaki her yanıt (boş liste için
    /// 400/422 dahil) kimlik doğrulamanın geçtiği anlamına gelir.
    pub async fn check_auth(&self) -> Result<()> {
        let (status, json) = self
            .post_signed(
                "generate-presigned-upload-url-multi",
                &serde_json::json!({ "files": [] }),
            )
            .await?;
        if status.is_server_error() {
            logln!("check_auth: {status} {}", server_message(&json));
            return Err(ApiError::Server);
        }
        Ok(())
    }

    pub async fn presign_multi(&self, files: Vec<PresignFile>) -> Result<Vec<PresignItem>> {
        let (status, json) = self
            .post_signed(
                "generate-presigned-upload-url-multi",
                &serde_json::json!({ "files": files }),
            )
            .await?;
        if !status.is_success() {
            logln!("presign: {status} {}", server_message(&json));
            return Err(ApiError::Server);
        }
        // Frontend ile aynı: yanıt `result` altında ya da doğrudan kökte olabilir.
        let data = json.get("result").unwrap_or(&json);
        data.get("results")
            .and_then(|r| serde_json::from_value(r.clone()).ok())
            .ok_or_else(|| {
                logln!("presign: beklenmeyen yanıt {json}");
                ApiError::Server
            })
    }

    /// Dropzone ile aynı: tek dosya bırakıldığında tekil rota ve `channel_code: "s3"`.
    pub async fn presign_single(&self, file: PresignFile) -> Result<PresignItem> {
        let (status, json) = self
            .post_signed(
                "generate-presigned-upload-url",
                &serde_json::json!({
                    "file_name": file.file_name,
                    "file_size": file.file_size,
                    "content_type": file.content_type,
                    "channel_code": "s3",
                }),
            )
            .await?;
        if !status.is_success() {
            logln!("presign: {status} {}", server_message(&json));
            return Err(ApiError::Server);
        }
        let data = json.get("result").unwrap_or(&json);
        let file_id = data.get("file_id").and_then(Value::as_i64);
        let upload_url = data.get("upload_url").and_then(Value::as_str);
        match (file_id, upload_url) {
            (Some(file_id), Some(upload_url)) => Ok(PresignItem {
                file_name: file.file_name,
                file_id,
                upload_url: upload_url.to_owned(),
            }),
            _ => {
                logln!("presign: beklenmeyen yanıt {json}");
                Err(ApiError::Server)
            }
        }
    }

    pub async fn confirm_single(&self, file_id: i64) -> Result<ConfirmOutcome> {
        let (status, json) = self
            .post_signed(
                "confirm-file-upload",
                &serde_json::json!({ "file_id": file_id }),
            )
            .await?;
        Self::confirm_outcome(status, json)
    }

    pub async fn confirm_multi(&self, file_ids: &[i64]) -> Result<ConfirmOutcome> {
        let (status, json) = self
            .post_signed(
                "confirm-file-upload-multi",
                &serde_json::json!({ "file_ids": file_ids }),
            )
            .await?;
        Self::confirm_outcome(status, json)
    }

    /// Onay yanıtını yorumlar; tekil ve toplu rota aynı biçimde döner.
    fn confirm_outcome(status: StatusCode, json: Value) -> Result<ConfirmOutcome> {
        let failed = json
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|s| s.eq_ignore_ascii_case("failed"));

        if status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY || failed
        {
            let rejected = json
                .get("rejected_files")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            return Ok(ConfirmOutcome::Rejected {
                message: server_message(&json),
                rejected,
            });
        }
        if !status.is_success() {
            logln!("confirm: {status} {}", server_message(&json));
            return Err(ApiError::Server);
        }
        Ok(ConfirmOutcome::Ok)
    }

    /// Dosyayı presigned URL'e akış halinde yükler. S3 chunked yüklemeyi kabul
    /// etmediği için Content-Length açıkça verilir.
    pub async fn put_to_s3(
        &self,
        upload_url: &str,
        path: &Path,
        size: u64,
        content_type: &str,
        mut on_progress: impl FnMut(u64) + Send + 'static,
    ) -> Result<()> {
        let file = tokio::fs::File::open(path).await.map_err(|e| {
            logln!("{}: {e}", path.display());
            ApiError::FileRead
        })?;
        let mut sent = 0u64;
        let stream = ReaderStream::new(file).inspect_ok(move |chunk| {
            sent += chunk.len() as u64;
            on_progress(sent);
        });

        let res = self
            .http
            .put(upload_url)
            .header(CONTENT_TYPE, content_type)
            .header(CONTENT_LENGTH, size)
            .header("x-amz-tagging", "status=temp")
            .body(reqwest::Body::wrap_stream(stream))
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            logln!(
                "S3 PUT {status}: {}",
                text.chars().take(300).collect::<String>()
            );
            return Err(ApiError::Storage);
        }
        Ok(())
    }
}

fn server_message(json: &Value) -> String {
    json.get("message")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| json.to_string().chars().take(300).collect())
}

pub fn content_type_for(path: &Path) -> String {
    mime_guess::from_path(path)
        .first_raw()
        .unwrap_or("application/octet-stream")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_node_reference() {
        // crypto.createHmac("sha256", "secret").update("POST\n/api/modules/x\n1700000000\n{}").digest("hex")
        let client = DgmosClient::new(Credentials {
            base_url: "https://sandbox.dgmos.com/api".into(),
            access_key: "ak".into(),
            secret_key: "secret".into(),
        })
        .unwrap();
        let sig = client.sign("POST", "/api/modules/x", "1700000000", "{}");
        assert_eq!(
            sig,
            "940f8ddebb891ffa4eb9a14a7011d457c0dd17a078afc339d7e1999b197ddac6"
        );
    }

    #[test]
    fn classifies_documented_auth_errors() {
        // frontend: API Anahtarları → Entegrasyon Rehberi → Hata Kodları
        let cases = [
            ("Missing mandatory X-Signature", AuthReason::Signature),
            ("Invalid HMAC signature", AuthReason::Signature),
            ("Request expired", AuthReason::Expired),
            ("origin not in allowed list", AuthReason::Origin),
            ("route restriction: not allowed", AuthReason::Route),
            ("Oturum gerekli", AuthReason::NotAccepted),
            ("You are not logged in!", AuthReason::NotAccepted),
            (r#"{"status":"no_login"}"#, AuthReason::NotAccepted),
            ("api key revoked", AuthReason::Revoked),
            ("invalid access key", AuthReason::UnknownKey),
        ];
        for (server, expected) in cases {
            assert_eq!(AuthReason::classify(server), expected, "{server}");
        }
    }

    #[test]
    fn endpoint_path_includes_api_prefix() {
        let client = DgmosClient::new(Credentials {
            base_url: "https://sandbox.dgmos.com/api/".into(),
            access_key: String::new(),
            secret_key: String::new(),
        })
        .unwrap();
        let url = client.endpoint("confirm-file-upload-multi").unwrap();
        assert_eq!(url.path(), "/api/modules/confirm-file-upload-multi");
    }
}

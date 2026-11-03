//! Toplu yükleme orkestrasyonu. Arayüzden bağımsızdır; ilerlemeyi `Event`
//! olarak kanala yollar.
//!
//! Dropzone'daki akıştan iki bilinçli farkı var:
//! - Presign yanıtı dosya adına göre bir kuyrukla eşlenir, böylece aynı adlı
//!   iki dosya aynı upload_url'e yazılmaz.
//! - Bir dosyanın S3 yüklemesi başarısız olursa yalnızca o dosya hata alır;
//!   başarılı olanlar yine onaylanır.

use crate::log::logln;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use tokio::sync::Semaphore;

use crate::api::{
    ApiError, ConfirmOutcome, Credentials, DgmosClient, PresignFile, PresignItem, content_type_for,
};

#[derive(Clone)]
pub struct Job {
    pub row: usize,
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
}

pub enum Event {
    Progress { row: usize, sent: u64 },
    Confirming { row: usize },
    Done { row: usize },
    Failed { row: usize, error: ApiError },
    BatchFinished,
    AuthChecked(Result<(), ApiError>),
    UpdateAvailable(crate::update::Release),
}

pub struct BatchOptions {
    pub creds: Credentials,
    pub is_public: bool,
    pub concurrency: usize,
}

pub async fn check_auth(creds: Credentials, tx: Sender<Event>, repaint: impl Fn()) {
    let result = async { DgmosClient::new(creds)?.check_auth().await }.await;
    let _ = tx.send(Event::AuthChecked(result));
    repaint();
}

pub async fn run_batch(
    jobs: Vec<Job>,
    opts: BatchOptions,
    tx: Sender<Event>,
    repaint: impl Fn() + Send + Sync + 'static,
) {
    let repaint = Arc::new(repaint);
    let send = {
        let tx = tx.clone();
        let repaint = repaint.clone();
        move |e: Event| {
            let _ = tx.send(e);
            repaint();
        }
    };

    let fail_all = |jobs: &[Job], error: ApiError| {
        for job in jobs {
            send(Event::Failed {
                row: job.row,
                error: error.clone(),
            });
        }
    };

    let client = match DgmosClient::new(opts.creds) {
        Ok(c) => Arc::new(c),
        Err(e) => {
            fail_all(&jobs, e);
            send(Event::BatchFinished);
            return;
        }
    };

    let mut request: Vec<PresignFile> = jobs
        .iter()
        .map(|j| PresignFile {
            file_name: j.name.clone(),
            file_size: j.size,
            content_type: content_type_for(&j.path),
            is_public: opts.is_public,
        })
        .collect();

    // Dropzone ile aynı: tek dosya tekil rotalardan, birden fazlası toplu rotalardan gider.
    let single = request.len() == 1;
    let presigned = if single {
        client
            .presign_single(request.remove(0))
            .await
            .map(|item| vec![item])
    } else {
        client.presign_multi(request).await
    };
    let presigned = match presigned {
        Ok(items) => items,
        Err(e) => {
            fail_all(&jobs, e);
            send(Event::BatchFinished);
            return;
        }
    };

    let mut by_name: HashMap<String, VecDeque<PresignItem>> = HashMap::new();
    for item in presigned {
        by_name
            .entry(item.file_name.clone())
            .or_default()
            .push_back(item);
    }

    let semaphore = Arc::new(Semaphore::new(opts.concurrency.max(1)));
    let mut handles = Vec::new();

    for job in jobs {
        let Some(item) = by_name.get_mut(&job.name).and_then(VecDeque::pop_front) else {
            logln!("presign yanıtında dosya yok: {}", job.name);
            send(Event::Failed {
                row: job.row,
                error: ApiError::Server,
            });
            continue;
        };

        let client = client.clone();
        let semaphore = semaphore.clone();
        let tx = tx.clone();
        let repaint = repaint.clone();

        handles.push(tokio::spawn(async move {
            let _permit = semaphore.acquire_owned().await.ok();
            let row = job.row;
            let progress = move |sent| {
                let _ = tx.send(Event::Progress { row, sent });
                repaint();
            };
            let content_type = content_type_for(&job.path);
            let result = client
                .put_to_s3(
                    &item.upload_url,
                    &job.path,
                    job.size,
                    &content_type,
                    progress,
                )
                .await;
            (job, item.file_id, result)
        }));
    }

    // (satır, file_id, dosya adı) — yalnızca S3'e başarıyla ulaşanlar
    let mut uploaded: Vec<(usize, i64, String)> = Vec::new();
    for handle in handles {
        match handle.await {
            Ok((job, file_id, Ok(()))) => {
                send(Event::Confirming { row: job.row });
                uploaded.push((job.row, file_id, job.name));
            }
            Ok((job, _, Err(error))) => send(Event::Failed {
                row: job.row,
                error,
            }),
            Err(e) => logln!("Yükleme görevi çöktü: {e}"),
        }
    }

    if uploaded.is_empty() {
        send(Event::BatchFinished);
        return;
    }

    let ids: Vec<i64> = uploaded.iter().map(|(_, id, _)| *id).collect();
    let confirmed = if single {
        client.confirm_single(ids[0]).await
    } else {
        client.confirm_multi(&ids).await
    };
    match confirmed {
        Ok(ConfirmOutcome::Ok) => {
            for (row, _, _) in uploaded {
                send(Event::Done { row });
            }
        }
        Ok(ConfirmOutcome::Rejected { message, rejected }) => {
            // Backend hangi dosyaları reddettiğini söylemediyse tüm parti reddedilmiş sayılır.
            let rejected_by_name: HashMap<String, String> = rejected
                .into_iter()
                .filter_map(|r| {
                    let text = r.message.or(r.reason).unwrap_or_else(|| message.clone());
                    r.filename.map(|f| (f, text))
                })
                .collect();
            for (row, _, name) in uploaded {
                if rejected_by_name.is_empty() {
                    send(Event::Failed {
                        row,
                        error: ApiError::Rejected(message.clone()),
                    });
                } else if let Some(reason) = rejected_by_name.get(&name) {
                    send(Event::Failed {
                        row,
                        error: ApiError::Rejected(reason.clone()),
                    });
                } else {
                    send(Event::Done { row });
                }
            }
        }
        Err(error) => {
            for (row, _, _) in uploaded {
                send(Event::Failed {
                    row,
                    error: error.clone(),
                });
            }
        }
    }

    send(Event::BatchFinished);
}

/// Bırakılan yolları dosya listesine açar; klasörler özyinelemeli gezilir,
/// gizli dosyalar (.DS_Store vb.) atlanır.
pub fn expand_paths(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack: Vec<PathBuf> = paths.into_iter().collect();
    stack.reverse();
    while let Some(path) = stack.pop() {
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'));
        if hidden {
            continue;
        }
        if path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&path) {
                let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
                children.sort();
                children.reverse();
                stack.extend(children);
            }
        } else if path.is_file() {
            out.push(path);
        }
    }
    out
}

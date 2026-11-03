#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod config;
mod log;
mod theme;
mod upload;

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{
    self, Align, Id, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, UiBuilder, Vec2,
};

use api::{ApiError, Credentials};
use config::Settings;
use log::logln;
use theme::{DANGER, Logos, PRIMARY, Palette, SUCCESS, Tone};
use upload::{BatchOptions, Event, Job};

/// Pencere başlığı, Dock ve menü çubuğunda görünen ad (macOS'ta paketteki
/// `CFBundleName` ile aynı olmalı; bkz. scripts/bundle-macos.sh).
const APP_NAME: &str = "DGMOS Drive";
const HEADER_HEIGHT: f32 = 60.0;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_icon(theme::app_icon())
            .with_inner_size([500.0, 660.0])
            .with_min_inner_size([420.0, 540.0])
            .with_drag_and_drop(true)
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            let logos = theme::install(&cc.egui_ctx);
            Ok(Box::new(DriveApp::new(logos)))
        }),
    )
}

#[derive(PartialEq)]
enum Screen {
    Connect,
    Main,
}

enum RowStatus {
    Queued,
    Uploading,
    Confirming,
    Done,
    Failed(ApiError),
}

struct Row {
    path: PathBuf,
    name: String,
    size: u64,
    sent: u64,
    status: RowStatus,
}

#[derive(Default)]
struct ConnectForm {
    access_key: String,
    secret_key: String,
    base_url: String,
    show_help: bool,
    show_advanced: bool,
    checking: bool,
    error: Option<String>,
    /// Sunucunun ham yanıtı; destek için hatanın altında küçük gösterilir.
    error_detail: Option<String>,
}

struct DriveApp {
    logos: Logos,
    runtime: tokio::runtime::Runtime,
    tx: Sender<Event>,
    rx: Receiver<Event>,
    settings: Settings,
    screen: Screen,
    form: ConnectForm,
    rows: Vec<Row>,
    active_batches: usize,
    session_expired: bool,
}

impl DriveApp {
    fn new(logos: Logos) -> Self {
        let (tx, rx) = channel();
        let settings = Settings::load();
        let connected = config::load_secret(&settings.access_key).is_some();
        let form = ConnectForm {
            access_key: settings.access_key.clone(),
            base_url: settings.base_url.clone(),
            ..Default::default()
        };
        Self {
            logos,
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            tx,
            rx,
            settings,
            screen: if connected {
                Screen::Main
            } else {
                Screen::Connect
            },
            form,
            rows: Vec::new(),
            active_batches: 0,
            session_expired: false,
        }
    }

    // ------------------------------------------------------------ durum

    fn credentials(&self) -> Option<Credentials> {
        let secret_key = config::load_secret(&self.settings.access_key)?;
        Some(Credentials {
            base_url: self.settings.base_url.clone(),
            access_key: self.settings.access_key.clone(),
            secret_key,
        })
    }

    fn enqueue(&mut self, ctx: &egui::Context, paths: Vec<PathBuf>) {
        let mut rows = Vec::new();
        for path in upload::expand_paths(paths) {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            rows.push(self.rows.len());
            self.rows.push(Row {
                path,
                name,
                size,
                sent: 0,
                status: RowStatus::Queued,
            });
        }
        self.start(ctx, rows);
    }

    fn retry(&mut self, ctx: &egui::Context, row: usize) {
        let r = &mut self.rows[row];
        r.sent = 0;
        r.status = RowStatus::Queued;
        self.start(ctx, vec![row]);
    }

    fn start(&mut self, ctx: &egui::Context, rows: Vec<usize>) {
        if rows.is_empty() {
            return;
        }
        let Some(creds) = self.credentials() else {
            self.session_expired = true;
            for &row in &rows {
                self.rows[row].status = RowStatus::Failed(ApiError::unauthorized(""));
            }
            return;
        };
        let jobs = rows
            .into_iter()
            .map(|row| {
                let r = &self.rows[row];
                Job {
                    row,
                    path: r.path.clone(),
                    name: r.name.clone(),
                    size: r.size,
                }
            })
            .collect();
        self.active_batches += 1;
        let opts = BatchOptions {
            creds,
            is_public: self.settings.is_public,
            concurrency: self.settings.concurrency,
        };
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.runtime
            .spawn(upload::run_batch(jobs, opts, tx, move || {
                ctx.request_repaint()
            }));
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Progress { row, sent } => {
                    let r = &mut self.rows[row];
                    r.sent = sent;
                    r.status = RowStatus::Uploading;
                }
                Event::Confirming { row } => {
                    let r = &mut self.rows[row];
                    r.sent = r.size;
                    r.status = RowStatus::Confirming;
                }
                Event::Done { row } => self.rows[row].status = RowStatus::Done,
                Event::Failed { row, error } => {
                    if matches!(error, ApiError::Unauthorized { .. }) {
                        self.session_expired = true;
                    }
                    self.rows[row].status = RowStatus::Failed(error);
                }
                Event::BatchFinished => self.active_batches = self.active_batches.saturating_sub(1),
                Event::AuthChecked(result) => self.finish_connect(result),
            }
        }
    }

    fn submit_connect(&mut self, ctx: &egui::Context) {
        let access_key = self.form.access_key.trim().to_owned();
        let secret_key = self.form.secret_key.trim().to_owned();
        if access_key.is_empty() || secret_key.is_empty() {
            self.form.error = Some("İki anahtarı da girin.".into());
            return;
        }
        let base_url = match self.form.base_url.trim() {
            "" => config::DEFAULT_BASE_URL.to_owned(),
            url => url.to_owned(),
        };
        self.form.error = None;
        self.form.error_detail = None;
        self.form.checking = true;
        let creds = Credentials {
            base_url,
            access_key,
            secret_key,
        };
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.runtime
            .spawn(upload::check_auth(creds, tx, move || ctx.request_repaint()));
    }

    fn finish_connect(&mut self, result: Result<(), ApiError>) {
        self.form.checking = false;
        if let Err(e) = result {
            self.form.error = Some(e.to_string());
            self.form.error_detail = e.detail().map(str::to_owned);
            return;
        }
        let access_key = self.form.access_key.trim().to_owned();
        if let Err(e) = config::save_secret(&access_key, self.form.secret_key.trim()) {
            logln!("{e:#}");
            self.form.error = Some("Anahtar bu bilgisayara kaydedilemedi. Tekrar deneyin.".into());
            return;
        }
        self.settings.access_key = access_key;
        if !self.form.base_url.trim().is_empty() {
            self.settings.base_url = self.form.base_url.trim().to_owned();
        }
        if let Err(e) = self.settings.save() {
            logln!("{e:#}");
        }
        self.form.secret_key.clear();
        self.session_expired = false;
        self.screen = Screen::Main;
    }

    fn disconnect(&mut self) {
        config::delete_secret(&self.settings.access_key);
        self.settings.access_key.clear();
        if let Err(e) = self.settings.save() {
            logln!("{e:#}");
        }
        self.form = ConnectForm {
            base_url: self.settings.base_url.clone(),
            ..Default::default()
        };
        if self.active_batches == 0 {
            self.rows.clear();
        }
        self.session_expired = false;
        self.screen = Screen::Connect;
    }

    fn reconnect(&mut self) {
        self.form = ConnectForm {
            access_key: self.settings.access_key.clone(),
            base_url: self.settings.base_url.clone(),
            ..Default::default()
        };
        self.screen = Screen::Connect;
    }

    fn web(&self, path: &str) -> String {
        format!("{}{path}", self.settings.web_url.trim_end_matches('/'))
    }

    // ------------------------------------------------------------ üst bar

    fn header_ui(&mut self, ui: &mut Ui, p: &Palette, header: Rect) {
        // Logo pencerenin tam ortasında; sağdaki kontroller onu kaydırmasın diye
        // akıştan bağımsız çizilir.
        theme::wordmark(ui, &self.logos, p, header.center(), 18.0);
        if self.screen != Screen::Main {
            return;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let menu = theme::btn_icon(ui, p, "⋯");
            egui::Popup::menu(&menu)
                .align(egui::RectAlign::BOTTOM_END)
                .width(220.0)
                .show(|ui| {
                    ui.add_space(4.0);
                    if menu_item(ui, p, "DGMOS'ta dosyalarımı aç", p.gray_800) {
                        open_in_browser(&self.web("/apps/file"));
                    }
                    if menu_item(ui, p, "Hata kaydını göster", p.gray_800) {
                        reveal_log();
                    }
                    if menu_item(ui, p, "Hesap bağlantısını kes", DANGER) {
                        self.disconnect();
                    }
                    ui.add_space(4.0);
                });
            ui.add_space(4.0);
            if self.active_batches > 0 {
                theme::badge(ui, p, "Yükleniyor", Tone::Primary);
            } else if self.session_expired {
                theme::badge(ui, p, "Bağlantı yok", Tone::Danger);
            } else {
                theme::badge(ui, p, "Bağlı", Tone::Success);
            }
        });
    }

    // ------------------------------------------------------------ bağlan

    fn connect_ui(&mut self, ui: &mut Ui, p: &Palette) {
        let card_width = ui.available_width().min(420.0);
        let inner = card_width - 64.0;
        ui.add_space(((ui.available_height() - 520.0) / 2.0).max(12.0));

        ui.vertical_centered(|ui| {
            ui.set_max_width(card_width);
            theme::card(p, 32).show(ui, |ui| {
                ui.set_width(inner);
                ui.vertical_centered(|ui| {
                    theme::mark_symbol(ui, &self.logos, p, 56.0);
                    ui.add_space(12.0);
                    ui.label(RichText::new("DGMOS'a bağlanın").font(theme::bold(22.0)).color(p.gray_900));
                    ui.label(
                        RichText::new("Dosyalarınızı sürükleyip bırakarak yüklemek için hesabınızı bağlayın.")
                            .font(theme::medium(13.5))
                            .color(p.gray_500),
                    );
                });
                ui.add_space(22.0);

                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    theme::form_label(ui, p, "Erişim anahtarı");
                    theme::form_control(ui, &mut self.form.access_key, "dg_ak_…", false, inner);
                    ui.add_space(8.0);
                    theme::form_label(ui, p, "Gizli anahtar");
                    let secret = theme::form_control(ui, &mut self.form.secret_key, "Gizli anahtarınız", true, inner);
                    let enter = secret.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                    if let Some(error) = &self.form.error {
                        ui.add_space(6.0);
                        alert(ui, p, Tone::Danger, None, error, inner);
                        if let Some(detail) = &self.form.error_detail {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("Sunucu yanıtı: {detail}"))
                                        .font(theme::medium(12.0))
                                        .color(p.gray_500),
                                )
                                .wrap()
                                .selectable(true),
                            );
                        }
                        if theme::link(ui, "Hata kaydını göster", p.gray_600, 12.0).clicked() {
                            reveal_log();
                        }
                    }
                    ui.add_space(14.0);

                    let label = if self.form.checking { "Bağlanıyor…" } else { "Bağlan" };
                    let clicked = theme::btn_primary(ui, label, Some(inner), !self.form.checking).clicked();
                    if (clicked || enter) && !self.form.checking {
                        let ctx = ui.ctx().clone();
                        self.submit_connect(&ctx);
                    }
                });

                ui.add_space(14.0);
                ui.vertical_centered(|ui| {
                    if theme::link(ui, "Anahtarlarımı nereden bulurum?", PRIMARY, 13.5).clicked() {
                        self.form.show_help = !self.form.show_help;
                    }
                });

                if self.form.show_help {
                    ui.add_space(10.0);
                    let mut open = false;
                    notice(ui, p, inner, |ui| {
                        for (n, step) in [
                            "DGMOS'ta Servisler → API Anahtarları sayfasını açın.",
                            "Yeni anahtar oluşturun; tür olarak Sunucu (Secret) seçin ve dosya yükleme iznini işaretleyin.",
                            "Gösterilen erişim anahtarını ve gizli anahtarı buraya yapıştırın.",
                        ]
                        .iter()
                        .enumerate()
                        {
                            ui.horizontal_top(|ui| {
                                ui.label(RichText::new(format!("{}.", n + 1)).font(theme::bold(13.0)).color(PRIMARY));
                                ui.add(
                                    egui::Label::new(RichText::new(*step).font(theme::medium(13.0)).color(p.gray_700))
                                        .wrap(),
                                );
                            });
                        }
                        ui.add_space(4.0);
                        open = theme::btn_light_primary(ui, p, "API Anahtarları sayfasını aç", None).clicked();
                    });
                    if open {
                        open_in_browser(&self.web("/apps/services/api-keys"));
                    }
                }

                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if theme::link(ui, "Gelişmiş", p.gray_500, 12.5).clicked() {
                        self.form.show_advanced = !self.form.show_advanced;
                    }
                });
                if self.form.show_advanced {
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        theme::form_label(ui, p, "Sunucu adresi");
                        theme::form_control(ui, &mut self.form.base_url, config::DEFAULT_BASE_URL, false, inner);
                    });
                }
            });
        });
    }

    // ------------------------------------------------------------ ana ekran

    fn main_ui(&mut self, ui: &mut Ui, p: &Palette, hovering_files: bool) {
        let width = ui.available_width();
        if self.session_expired {
            let mut reconnect = false;
            alert(
                ui,
                p,
                Tone::Danger,
                Some("Hesap bağlantınız sona erdi"),
                "Yüklemeye devam etmek için yeniden bağlanın.",
                width,
            );
            ui.horizontal(|ui| {
                reconnect = theme::btn_primary(ui, "Yeniden bağlan", None, true).clicked();
            });
            if reconnect {
                self.reconnect();
                return;
            }
            ui.add_space(12.0);
        }

        let has_rows = !self.rows.is_empty();
        let zone_height = if has_rows {
            150.0
        } else {
            (ui.available_height() - 110.0).max(220.0)
        };
        let mut pick = false;
        theme::card(p, 24).show(ui, |ui| {
            ui.set_width(width - 48.0);
            card_title(
                ui,
                p,
                "Dosya Yükle",
                "Dosyalarınız DGMOS hesabınıza yüklenir",
            );
            ui.add_space(14.0);
            pick = self.dropzone(ui, p, zone_height, hovering_files);
        });
        if pick
            && let Some(paths) = rfd::FileDialog::new()
                .set_title("Yüklenecek dosyaları seçin")
                .pick_files()
        {
            let ctx = ui.ctx().clone();
            self.enqueue(&ctx, paths);
        }

        if has_rows {
            ui.add_space(16.0);
            self.uploads_card(ui, p, width);
        }
    }

    /// Metronic dropzone. Tıklanırsa `true` döner.
    fn dropzone(&self, ui: &mut Ui, p: &Palette, height: f32, hovering_files: bool) -> bool {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        let t = ui
            .ctx()
            .animate_bool_with_time(Id::unique("dropzone-files"), hovering_files, 0.15);
        let h = ui.ctx().animate_bool_with_time(
            Id::unique("dropzone-pointer"),
            response.hovered(),
            0.12,
        );
        theme::paint_dropzone(ui, p, rect, t.max(h * 0.5));

        let c = rect.center();
        let compact = height < 190.0;
        let icon_y = if compact { c.y - 26.0 } else { c.y - 36.0 };
        theme::paint_file_up(
            ui,
            Pos2::new(c.x, icon_y),
            if compact { 40.0 } else { 54.0 },
            PRIMARY,
        );

        let title = if hovering_files {
            "Bırakın, hemen yükleyelim"
        } else {
            "Dosyaları buraya bırakın ya da tıklayıp seçin"
        };
        let title_y = if compact { c.y + 14.0 } else { c.y + 20.0 };
        ui.painter().text(
            Pos2::new(c.x, title_y),
            egui::Align2::CENTER_CENTER,
            title,
            theme::bold(15.0),
            p.gray_900,
        );
        ui.painter().text(
            Pos2::new(c.x, title_y + 22.0),
            egui::Align2::CENTER_CENTER,
            "Birden fazla dosya ya da klasör seçebilirsiniz",
            theme::medium(12.5),
            p.gray_500,
        );
        response.clicked()
    }

    fn uploads_card(&mut self, ui: &mut Ui, p: &Palette, width: f32) {
        let total = self.rows.len();
        let done = self
            .rows
            .iter()
            .filter(|r| matches!(r.status, RowStatus::Done))
            .count();
        let failed = self
            .rows
            .iter()
            .filter(|r| matches!(r.status, RowStatus::Failed(_)))
            .count();
        let (summary, tone) = if self.active_batches > 0 {
            (format!("{done}/{total} tamamlandı"), Tone::Primary)
        } else if failed == 0 {
            ("Hepsi yüklendi".to_owned(), Tone::Success)
        } else {
            (format!("{failed} dosya yüklenemedi"), Tone::Danger)
        };

        let mut retry = None;
        let mut clear = false;
        theme::card(p, 24).show(ui, |ui| {
            ui.set_width(width - 48.0);
            ui.horizontal(|ui| {
                card_title(ui, p, "Yüklemeler", &format!("{total} dosya"));
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    if self.active_batches == 0
                        && theme::link(ui, "Temizle", p.gray_500, 13.0).clicked()
                    {
                        clear = true;
                    }
                    theme::badge(ui, p, &summary, tone);
                });
            });
            ui.add_space(10.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .max_height((ui.available_height() - 4.0).max(120.0))
                .show(ui, |ui| {
                    let count = self.rows.len();
                    for (i, (index, row)) in self.rows.iter().enumerate().rev().enumerate() {
                        if upload_row(ui, p, row) {
                            retry = Some(index);
                        }
                        if i + 1 < count {
                            dashed_separator(ui, p);
                        }
                    }
                });
        });

        if clear {
            self.rows.clear();
        }
        if let Some(row) = retry {
            let ctx = ui.ctx().clone();
            self.retry(&ctx, row);
        }
    }
}

// ---------------------------------------------------------------- parçalar

/// Metronic `card-title`: `card-label fw-bold` + `text-muted fw-semibold fs-7`.
fn card_title(ui: &mut Ui, p: &Palette, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(
            RichText::new(title)
                .font(theme::bold(16.5))
                .color(p.gray_900),
        );
        ui.label(
            RichText::new(subtitle)
                .font(theme::medium(12.5))
                .color(p.gray_500),
        );
    });
}

/// Metronic `alert bg-light-* border-dashed`.
fn alert(ui: &mut Ui, p: &Palette, tone: Tone, title: Option<&str>, text: &str, width: f32) {
    let (fg, bg) = p.tone(tone);
    egui::Frame::new()
        .fill(bg)
        .corner_radius(theme::RADIUS)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(width - 28.0);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                if let Some(title) = title {
                    ui.label(RichText::new(title).font(theme::bold(14.0)).color(fg));
                }
                ui.add(
                    egui::Label::new(
                        RichText::new(text)
                            .font(theme::medium(13.0))
                            .color(if title.is_some() { p.gray_700 } else { fg }),
                    )
                    .wrap(),
                );
            });
        });
}

/// API Anahtarları sayfasındaki bilgi kutusu: `bg-light-primary border-primary border-dashed`.
fn notice(ui: &mut Ui, p: &Palette, width: f32, add: impl FnOnce(&mut Ui)) {
    let (_, bg) = p.tone(Tone::Primary);
    let inner = egui::Frame::new()
        .fill(bg)
        .corner_radius(theme::RADIUS)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(width - 32.0);
            ui.with_layout(Layout::top_down(Align::Min), add);
        });
    let r = inner.response.rect.shrink(0.5);
    let path = [
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    ui.painter().extend(egui::epaint::Shape::dashed_line(
        &path,
        Stroke::new(1.0, PRIMARY.gamma_multiply(0.6)),
        5.0,
        4.0,
    ));
}

fn dashed_separator(ui: &mut Ui, p: &Palette) {
    let r = ui.available_rect_before_wrap();
    let y = ui.cursor().top() + 1.0;
    ui.painter().extend(egui::epaint::Shape::dashed_line(
        &[Pos2::new(r.left(), y), Pos2::new(r.right() - 8.0, y)],
        Stroke::new(1.0, p.border.gamma_multiply(if p.dark { 1.0 } else { 1.6 })),
        4.0,
        4.0,
    ));
    ui.add_space(3.0);
}

fn menu_item(ui: &mut Ui, p: &Palette, text: &str, color: egui::Color32) -> bool {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 36.0), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, theme::RADIUS, p.tone(Tone::Primary).1);
    }
    let fg = if response.hovered() && color != DANGER {
        PRIMARY
    } else {
        color
    };
    ui.painter().text(
        Pos2::new(rect.left() + 12.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        theme::medium(13.5),
        fg,
    );
    let clicked = response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked();
    if clicked {
        ui.close();
    }
    clicked
}

/// Tek bir yükleme satırı. "Tekrar dene"ye basılırsa `true` döner.
fn upload_row(ui: &mut Ui, p: &Palette, row: &Row) -> bool {
    let mut retry = false;
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        theme::file_symbol(ui, p, &row.name);
        ui.add_space(6.0);

        let right_width = 120.0;
        ui.allocate_ui_with_layout(
            Vec2::new((ui.available_width() - right_width).max(80.0), 40.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.add(
                    egui::Label::new(
                        RichText::new(&row.name)
                            .font(theme::bold(14.0))
                            .color(p.gray_800),
                    )
                    .truncate(),
                );
                let text = match &row.status {
                    RowStatus::Queued => human_size(row.size),
                    RowStatus::Uploading => {
                        format!("{} / {}", human_size(row.sent), human_size(row.size))
                    }
                    RowStatus::Confirming => "Son kontroller yapılıyor…".to_owned(),
                    RowStatus::Done => human_size(row.size),
                    RowStatus::Failed(e) => e.to_string(),
                };
                let color = if matches!(row.status, RowStatus::Failed(_)) {
                    DANGER
                } else {
                    p.gray_500
                };
                ui.add(
                    egui::Label::new(RichText::new(text).font(theme::medium(12.5)).color(color))
                        .wrap(),
                );
            },
        );

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            match &row.status {
                RowStatus::Done => {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                    ui.painter()
                        .rect_filled(rect, theme::RADIUS, p.tone(Tone::Success).1);
                    theme::paint_check(ui, rect.center(), 16.0, SUCCESS);
                }
                RowStatus::Failed(e) => {
                    if is_retryable(e) {
                        retry = theme::link(ui, "Tekrar dene", PRIMARY, 13.0).clicked();
                    } else {
                        theme::badge(ui, p, "Reddedildi", Tone::Danger);
                    }
                }
                RowStatus::Uploading => {
                    theme::badge(ui, p, &format!("%{}", percent(row)), Tone::Primary)
                }
                RowStatus::Confirming => theme::badge(ui, p, "Kontrol", Tone::Primary),
                RowStatus::Queued => theme::badge(ui, p, "Sırada", Tone::Gray),
            }
        });
    });

    if matches!(row.status, RowStatus::Uploading | RowStatus::Confirming) {
        ui.add_space(4.0);
        let full = ui.available_rect_before_wrap();
        let bar = Rect::from_min_size(
            Pos2::new(full.left() + 54.0, ui.cursor().top()),
            Vec2::new(full.width() - 62.0, 6.0),
        );
        theme::progress(ui, p, bar, percent(row) as f32 / 100.0);
        ui.add_space(8.0);
    }
    ui.add_space(10.0);
    retry
}

fn is_retryable(e: &ApiError) -> bool {
    matches!(
        e,
        ApiError::Network
            | ApiError::Storage
            | ApiError::Server
            | ApiError::FileRead
            | ApiError::Unauthorized { .. }
    )
}

fn percent(row: &Row) -> u32 {
    if row.size == 0 {
        100
    } else {
        ((row.sent as f64 / row.size as f64) * 100.0)
            .round()
            .min(100.0) as u32
    }
}

/// Hata kaydı dosyasını Finder / Gezgin'de seçili gösterir.
fn reveal_log() {
    let Some(path) = log::path() else { return };
    if !path.exists() {
        logln!("hata kaydı açıldı");
    }
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg("-R")
        .arg(&path)
        .spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(&path))
        .spawn();
    if let Err(e) = result {
        logln!("hata kaydı açılamadı: {e}");
    }
}

fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = result {
        logln!("tarayıcı açılamadı: {e}");
    }
}

/// Türkçe biçim: "12,4 MB".
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit]).replace('.', ",")
    }
}

impl eframe::App for DriveApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.drain_events();
        let ctx = ui.ctx().clone();
        let p = Palette::of(ui);
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, p.page_bg);

        let (dropped, hovering) = ctx.input(|i| {
            let dropped: Vec<PathBuf> = i
                .raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect();
            (dropped, !i.raw.hovered_files.is_empty())
        });
        let dropped_any = !dropped.is_empty();
        if dropped_any {
            if self.screen == Screen::Main {
                self.enqueue(&ctx, dropped);
            } else {
                self.form.error = Some("Dosya yüklemek için önce hesabınızı bağlayın.".into());
            }
        }

        // Metronic app-header: kart renginde üst bar. Başlık çubuğu olmadığı için
        // pencere buradan sürüklenir.
        let header = Rect::from_min_size(full.min, Vec2::new(full.width(), HEADER_HEIGHT));
        ui.painter().rect_filled(header, 0.0, p.card_bg);
        ui.painter().hline(
            header.x_range(),
            header.bottom(),
            Stroke::new(1.0, p.border),
        );
        let drag = ui.interact(header, Id::unique("window-drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(header.shrink2(Vec2::new(20.0, 0.0)))
                .layout(Layout::left_to_right(Align::Center)),
            |ui| self.header_ui(ui, &p, header),
        );

        let body = Rect::from_min_max(
            Pos2::new(full.left() + 20.0, header.bottom() + 20.0),
            Pos2::new(full.right() - 20.0, full.bottom() - 20.0),
        );
        ui.scope_builder(
            UiBuilder::new()
                .max_rect(body)
                .layout(Layout::top_down(Align::Min)),
            |ui| match self.screen {
                Screen::Connect => {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false; 2])
                        .show(ui, |ui| self.connect_ui(ui, &p));
                }
                Screen::Main => self.main_ui(ui, &p, hovering),
            },
        );

        if dropped_any || hovering {
            ctx.request_repaint();
        }
    }
}

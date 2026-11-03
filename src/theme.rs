//! Görsel dil: DGMOS web arayüzü (Metronic 8) ile aynı token'lar ve bileşen
//! ölçüleri — gri sayfa zemini, beyaz kartlar, `form-control-solid` girişler,
//! `btn-primary` / `btn-light-primary`, `badge-light-*`, `symbol-light-*` ve
//! kesikli `dropzone`. Açık/koyu tema sistemle birlikte değişir.
//!
//! Renkler: frontend `src/_metronic/assets/sass/components/_variables.custom.scss`.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin,
    Pos2, Rect, Sense, Shadow, Stroke, TextStyle, TextureHandle, TextureOptions, Theme, Ui, Vec2,
    epaint,
};

pub const PRIMARY: Color32 = Color32::from_rgb(0x00, 0x9E, 0xF7);
pub const PRIMARY_ACTIVE: Color32 = Color32::from_rgb(0x00, 0x95, 0xE8);
pub const SUCCESS: Color32 = Color32::from_rgb(0x50, 0xCD, 0x89);
pub const DANGER: Color32 = Color32::from_rgb(0xF1, 0x41, 0x6C);
pub const WARNING: Color32 = Color32::from_rgb(0xFF, 0xC7, 0x00);
pub const INFO: Color32 = Color32::from_rgb(0x02, 0x84, 0xC7);

/// Metronic `$btn-border-radius` (0.475rem) ve `$card-border-radius` (0.625rem), 13px tabanla.
pub const RADIUS: u8 = 7;
pub const CARD_RADIUS: u8 = 10;

const BOLD: &str = "bold";
const MEDIUM: &str = "medium";

#[derive(Clone, Copy)]
pub enum Tone {
    Primary,
    Success,
    Danger,
    Warning,
    Info,
    Gray,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub page_bg: Color32,
    pub card_bg: Color32,
    pub border: Color32,
    pub input_bg: Color32,
    pub gray_900: Color32,
    pub gray_800: Color32,
    pub gray_700: Color32,
    pub gray_600: Color32,
    pub gray_500: Color32,
    primary_light: Color32,
    success_light: Color32,
    danger_light: Color32,
    warning_light: Color32,
    info_light: Color32,
    gray_light: Color32,
    shadow: Color32,
}

impl Palette {
    pub fn of(ui: &Ui) -> Self {
        if ui.visuals().dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
    }

    fn light() -> Self {
        Self {
            dark: false,
            page_bg: Color32::from_rgb(0xF5, 0xF8, 0xFA),
            card_bg: Color32::WHITE,
            border: Color32::from_rgb(0xEF, 0xF2, 0xF5),
            input_bg: Color32::from_rgb(0xF5, 0xF8, 0xFA),
            gray_900: Color32::from_rgb(0x18, 0x1C, 0x32),
            gray_800: Color32::from_rgb(0x3F, 0x42, 0x54),
            gray_700: Color32::from_rgb(0x5E, 0x62, 0x78),
            gray_600: Color32::from_rgb(0x7E, 0x82, 0x99),
            gray_500: Color32::from_rgb(0xA1, 0xA5, 0xB7),
            primary_light: Color32::from_rgb(0xF1, 0xFA, 0xFF),
            success_light: Color32::from_rgb(0xE8, 0xFF, 0xF3),
            danger_light: Color32::from_rgb(0xFF, 0xF5, 0xF8),
            warning_light: Color32::from_rgb(0xFF, 0xF8, 0xDD),
            info_light: Color32::from_rgb(0xF0, 0xF9, 0xFF),
            gray_light: Color32::from_rgb(0xF5, 0xF8, 0xFA),
            shadow: Color32::from_rgba_unmultiplied(76, 87, 125, 14),
        }
    }

    fn dark() -> Self {
        Self {
            dark: true,
            page_bg: Color32::from_rgb(0x15, 0x15, 0x21),
            card_bg: Color32::from_rgb(0x1E, 0x1E, 0x2D),
            border: Color32::from_rgb(0x2B, 0x2B, 0x40),
            input_bg: Color32::from_rgb(0x1B, 0x1B, 0x29),
            gray_900: Color32::WHITE,
            gray_800: Color32::from_rgb(0xCD, 0xCD, 0xDE),
            gray_700: Color32::from_rgb(0x92, 0x92, 0x9F),
            gray_600: Color32::from_rgb(0x6D, 0x6D, 0x80),
            gray_500: Color32::from_rgb(0x56, 0x56, 0x74),
            primary_light: Color32::from_rgb(0x21, 0x2E, 0x48),
            success_light: Color32::from_rgb(0x1C, 0x32, 0x38),
            danger_light: Color32::from_rgb(0x3A, 0x24, 0x34),
            warning_light: Color32::from_rgb(0x39, 0x2F, 0x28),
            info_light: Color32::from_rgb(0x16, 0x2C, 0x46),
            gray_light: Color32::from_rgb(0x2B, 0x2B, 0x40),
            shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 40),
        }
    }

    /// (ana renk, `-light` zemin) — Metronic `badge-light-*` / `symbol-light-*`.
    pub fn tone(&self, tone: Tone) -> (Color32, Color32) {
        match tone {
            Tone::Primary => (PRIMARY, self.primary_light),
            Tone::Success => (SUCCESS, self.success_light),
            Tone::Danger => (DANGER, self.danger_light),
            Tone::Warning => (
                if self.dark {
                    WARNING
                } else {
                    Color32::from_rgb(0xB7, 0x8A, 0x00)
                },
                self.warning_light,
            ),
            Tone::Info => (INFO, self.info_light),
            Tone::Gray => (self.gray_600, self.gray_light),
        }
    }
}

// ---------------------------------------------------------------- kurulum

pub struct Logos {
    pub wordmark: TextureHandle,
    pub mark: TextureHandle,
}

pub fn install(ctx: &egui::Context) -> Logos {
    install_fonts(ctx);
    for theme in [Theme::Light, Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            let p = if theme == Theme::Dark {
                Palette::dark()
            } else {
                Palette::light()
            };
            apply_style(style, &p);
        });
    }
    Logos {
        wordmark: load_texture(
            ctx,
            "logo-wordmark",
            include_bytes!("../assets/logo-wordmark.png"),
        ),
        mark: load_texture(ctx, "logo-mark", include_bytes!("../assets/logo-mark.png")),
    }
}

pub fn app_icon() -> egui::IconData {
    let img = image::load_from_memory(include_bytes!("../assets/app-icon.png"))
        .expect("gömülü ikon geçerli PNG")
        .into_rgba8();
    egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}

/// Logolar beyaz alfa maskesi olarak gömülüdür; çizerken temaya göre renklendirilir.
fn load_texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let img = image::load_from_memory(bytes)
        .expect("gömülü logo geçerli PNG")
        .into_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    ctx.load_texture(name, color, TextureOptions::LINEAR)
}

fn apply_style(style: &mut egui::Style, p: &Palette) {
    use TextStyle::*;
    style.text_styles = [
        (Heading, bold(20.0)),
        (Body, FontId::proportional(14.0)),
        (Button, medium(14.0)),
        (Small, FontId::proportional(12.0)),
        (Monospace, FontId::monospace(13.0)),
    ]
    .into();

    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.spacing.interact_size.y = 36.0;

    let v = &mut style.visuals;
    v.override_text_color = Some(p.gray_800);
    v.panel_fill = p.page_bg;
    v.window_fill = p.card_bg;
    v.extreme_bg_color = p.input_bg;
    v.text_edit_bg_color = Some(p.input_bg);
    v.selection.bg_fill = PRIMARY.gamma_multiply(0.30);
    v.selection.stroke = Stroke::new(1.0, PRIMARY);
    v.hyperlink_color = PRIMARY;
    v.error_fg_color = DANGER;
    v.warn_fg_color = WARNING;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(RADIUS);
    }
    // form-control-solid: kenarlık yok, odakta zemin biraz koyulaşır.
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.weak_bg_fill = p.gray_light;
    v.widgets.inactive.bg_fill = p.input_bg;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = p.primary_light;
    v.widgets.active.bg_stroke = Stroke::NONE;
    v.widgets.active.weak_bg_fill = p.primary_light;
    v.window_corner_radius = CornerRadius::same(CARD_RADIUS);
    v.menu_corner_radius = CornerRadius::same(CARD_RADIUS);
    v.window_stroke = Stroke::new(1.0, p.border);
    v.popup_shadow = Shadow {
        offset: [0, 8],
        blur: 30,
        spread: 0,
        color: p.shadow.gamma_multiply(3.0),
    };
    v.window_shadow = v.popup_shadow;
}

/// DGMOS web arayüzü Inter kullanır. Sistemde Inter varsa onu, yoksa platformun
/// arayüz fontunu (SF Pro / Segoe UI) yükler; hiçbiri yoksa egui varsayılanı kalır.
/// Değişken fontlarda Metronic ağırlıkları kullanılır: fw-semibold 500, fw-bold 600.
fn install_fonts(ctx: &egui::Context) {
    let home = dirs::home_dir().unwrap_or_default();
    let local = dirs::data_local_dir().unwrap_or_default();
    let candidates: Vec<PathBuf> = vec![
        home.join("Library/Fonts/InterVariable.ttf"),
        PathBuf::from("/Library/Fonts/InterVariable.ttf"),
        local.join("Microsoft/Windows/Fonts/InterVariable.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\InterVariable.ttf"),
        home.join(".local/share/fonts/InterVariable.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/inter/InterVariable.ttf"),
        PathBuf::from("/System/Library/Fonts/SFNS.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\segoeui.ttf"),
    ];
    let Some(bytes) = candidates.iter().find_map(|p| std::fs::read(p).ok()) else {
        return;
    };

    let base = FontData::from_owned(bytes);
    let wght = epaint::text::IntoTag::into_tag("wght");
    let variable = base.variation_axes().iter().any(|a| a.tag == wght);
    let weighted = |w: f32| {
        let mut tweak = base.tweak.clone();
        tweak.coords = epaint::text::VariationCoords::new([("wght", w)]);
        base.clone().tweak(tweak)
    };
    let (medium_font, bold_font) = if variable {
        (weighted(500.0), weighted(600.0))
    } else {
        let semibold = std::fs::read(r"C:\Windows\Fonts\seguisb.ttf")
            .ok()
            .map(FontData::from_owned);
        (base.clone(), semibold.unwrap_or_else(|| base.clone()))
    };

    let mut fonts = FontDefinitions::default();
    let defaults = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for (key, data, family) in [
        (
            "ui",
            if variable {
                weighted(400.0)
            } else {
                base.clone()
            },
            FontFamily::Proportional,
        ),
        ("ui-medium", medium_font, FontFamily::Name(MEDIUM.into())),
        ("ui-bold", bold_font, FontFamily::Name(BOLD.into())),
    ] {
        fonts.font_data.insert(key.into(), Arc::new(data));
        let mut list = vec![key.to_owned()];
        list.extend(defaults.iter().cloned());
        fonts.families.insert(family, list);
    }
    ctx.set_fonts(fonts);
}

/// Metronic `fw-bold` (600).
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD.into()))
}

/// Metronic `fw-semibold` (500).
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

// ---------------------------------------------------------------- bileşenler

/// Metronic `card`: beyaz zemin, çok hafif gölge.
pub fn card(p: &Palette, margin: i8) -> Frame {
    Frame::new()
        .fill(p.card_bg)
        .corner_radius(CARD_RADIUS)
        .inner_margin(Margin::same(margin))
        .shadow(Shadow {
            offset: [0, 0],
            blur: 20,
            spread: 0,
            color: p.shadow,
        })
}

/// Logo kelime işaretini `center` noktasına, verilen yükseklikte ve temaya uygun renkte çizer.
pub fn wordmark(ui: &Ui, logos: &Logos, p: &Palette, center: Pos2, height: f32) {
    let size = logos.wordmark.size_vec2();
    let rect = Rect::from_center_size(center, Vec2::new(size.x * height / size.y, height));
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    ui.painter()
        .image(logos.wordmark.id(), rect, uv, p.gray_900);
}

/// Dünya simgesi, `symbol-light-primary` zemin üzerinde.
pub fn mark_symbol(ui: &mut Ui, logos: &Logos, p: &Palette, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter().rect_filled(rect, CARD_RADIUS, p.primary_light);
    let inner = rect.shrink(size * 0.2);
    ui.painter().image(
        logos.mark.id(),
        inner,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        PRIMARY,
    );
}

/// Metronic `badge badge-light-*`.
pub fn badge(ui: &mut Ui, p: &Palette, text: &str, tone: Tone) {
    let (fg, bg) = p.tone(tone);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), bold(11.5), fg);
    let size = Vec2::new(galley.size().x + 14.0, 22.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, 6.0, bg);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, fg);
}

struct ButtonLook {
    fill: Color32,
    fg: Color32,
    hover_fill: Color32,
    hover_fg: Color32,
}

fn button(
    ui: &mut Ui,
    text: &str,
    look: ButtonLook,
    width: Option<f32>,
    enabled: bool,
) -> egui::Response {
    let galley_w = ui
        .painter()
        .layout_no_wrap(text.to_owned(), medium(14.0), look.fg)
        .size()
        .x;
    let w = width.unwrap_or(galley_w + 40.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, 42.0), sense);
    let hovered = enabled && response.hovered();
    let (fill, fg) = if hovered {
        (look.hover_fill, look.hover_fg)
    } else {
        (look.fill, look.fg)
    };
    let alpha = if enabled { 1.0 } else { 0.65 };
    ui.painter()
        .rect_filled(rect, RADIUS, fill.gamma_multiply(alpha));
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), medium(14.0), fg);
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        fg.gamma_multiply(alpha),
    );
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// Metronic `btn btn-primary`.
pub fn btn_primary(ui: &mut Ui, text: &str, width: Option<f32>, enabled: bool) -> egui::Response {
    let look = ButtonLook {
        fill: PRIMARY,
        fg: Color32::WHITE,
        hover_fill: PRIMARY_ACTIVE,
        hover_fg: Color32::WHITE,
    };
    button(ui, text, look, width, enabled)
}

/// Metronic `btn btn-light-primary`.
pub fn btn_light_primary(
    ui: &mut Ui,
    p: &Palette,
    text: &str,
    width: Option<f32>,
) -> egui::Response {
    let look = ButtonLook {
        fill: p.primary_light,
        fg: PRIMARY,
        hover_fill: PRIMARY,
        hover_fg: Color32::WHITE,
    };
    button(ui, text, look, width, true)
}

/// Metronic `btn btn-icon btn-light btn-active-light-primary`.
pub fn btn_icon(ui: &mut Ui, p: &Palette, glyph: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
    let hovered = response.hovered();
    let (bg, fg) = if hovered {
        (p.primary_light, PRIMARY)
    } else {
        (p.gray_light, p.gray_600)
    };
    ui.painter().rect_filled(rect, RADIUS, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        bold(14.0),
        fg,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Metin bağlantısı (`link-primary fw-semibold`).
pub fn link(ui: &mut Ui, text: &str, color: Color32, size: f32) -> egui::Response {
    let r = ui.add(
        egui::Label::new(egui::RichText::new(text).color(color).font(medium(size)))
            .sense(Sense::click()),
    );
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Metronic `form-label fw-semibold`.
pub fn form_label(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(medium(13.5))
            .color(p.gray_900),
    );
}

/// Metronic `form-control form-control-solid`.
pub fn form_control(
    ui: &mut Ui,
    value: &mut String,
    hint: &str,
    password: bool,
    width: f32,
) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .password(password)
            .desired_width(width)
            .font(medium(14.0))
            .margin(Margin::symmetric(14, 11)),
    )
}

/// Metronic `progress h-6px bg-light-primary` + `progress-bar bg-primary`.
pub fn progress(ui: &Ui, p: &Palette, rect: Rect, fraction: f32) {
    ui.painter()
        .rect_filled(rect, rect.height() / 2.0, p.primary_light);
    let fill = Rect::from_min_size(
        rect.min,
        Vec2::new(rect.width() * fraction.clamp(0.0, 1.0), rect.height()),
    );
    ui.painter().rect_filled(fill, rect.height() / 2.0, PRIMARY);
}

/// Metronic `dropzone`: kesikli birincil kenarlık + `primary-light` zemin.
pub fn paint_dropzone(ui: &Ui, p: &Palette, rect: Rect, emphasis: f32) {
    let bg = p.primary_light.lerp_to_gamma(
        PRIMARY.gamma_multiply(if p.dark { 0.35 } else { 0.12 }),
        emphasis,
    );
    ui.painter().rect_filled(rect, RADIUS, bg);
    let stroke = Stroke::new(1.0 + emphasis, PRIMARY);
    let path = rounded_rect_path(rect.shrink(0.5), RADIUS as f32 + 2.0);
    ui.painter()
        .extend(epaint::Shape::dashed_line(&path, stroke, 7.0, 5.0));
}

fn rounded_rect_path(r: Rect, radius: f32) -> Vec<Pos2> {
    use std::f32::consts::{FRAC_PI_2, PI};
    let corners = [
        (Pos2::new(r.right() - radius, r.top() + radius), -FRAC_PI_2),
        (Pos2::new(r.right() - radius, r.bottom() - radius), 0.0),
        (Pos2::new(r.left() + radius, r.bottom() - radius), FRAC_PI_2),
        (Pos2::new(r.left() + radius, r.top() + radius), PI),
    ];
    let mut points = vec![Pos2::new(r.left() + radius, r.top())];
    for (center, start) in corners {
        for i in 0..=8 {
            let a = start + FRAC_PI_2 * i as f32 / 8.0;
            points.push(center + Vec2::angled(a) * radius);
        }
    }
    points.push(Pos2::new(r.left() + radius, r.top()));
    points
}

/// Metronic `ki-duotone ki-file-up`: soluk belge + üzerine yukarı ok.
pub fn paint_file_up(ui: &Ui, center: Pos2, size: f32, color: Color32) {
    let painter = ui.painter();
    let doc = Rect::from_center_size(center, Vec2::new(size * 0.72, size * 0.9));
    painter.rect_filled(doc, size * 0.12, color.gamma_multiply(0.3));
    let s = Stroke::new(size * 0.085, color);
    let top = center + Vec2::new(0.0, -size * 0.2);
    let bottom = center + Vec2::new(0.0, size * 0.22);
    painter.line_segment([top, bottom], s);
    painter.line_segment([top, top + Vec2::new(-size * 0.16, size * 0.16)], s);
    painter.line_segment([top, top + Vec2::new(size * 0.16, size * 0.16)], s);
}

pub fn paint_check(ui: &Ui, center: Pos2, size: f32, color: Color32) {
    let s = Stroke::new(size * 0.12, color);
    let a = center + Vec2::new(-size * 0.28, 0.0);
    let b = center + Vec2::new(-size * 0.08, size * 0.20);
    let c = center + Vec2::new(size * 0.30, -size * 0.22);
    ui.painter().line_segment([a, b], s);
    ui.painter().line_segment([b, c], s);
}

/// Dosya türü rozeti: Metronic `symbol symbol-40px symbol-light-*`.
pub fn file_symbol(ui: &mut Ui, p: &Palette, name: &str) {
    let ext = std::path::Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let tone = match ext.as_str() {
        "pdf" => Tone::Danger,
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "heic" | "svg" => Tone::Success,
        "doc" | "docx" | "txt" | "rtf" | "odt" => Tone::Primary,
        "xls" | "xlsx" | "csv" | "ods" => Tone::Info,
        "zip" | "rar" | "7z" => Tone::Warning,
        _ => Tone::Gray,
    };
    let (fg, bg) = p.tone(tone);
    let label = if ext.is_empty() {
        "—".to_owned()
    } else {
        ext.chars().take(4).collect::<String>().to_uppercase()
    };
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
    ui.painter().rect_filled(rect, RADIUS, bg);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        bold(10.5),
        fg,
    );
}

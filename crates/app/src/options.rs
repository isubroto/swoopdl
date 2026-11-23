//! Options (Downloads → Options, the toolbar's Options): IDM's configuration dialog, nine pages
//! in a two-row tab control (notes/ui-spec/options-dialog.md). Light in both modes, as IDM's.
//! It edits a copy of the settings: OK applies it, the close box and Escape discard it.

use egui::{Pos2, Rect, Sense, Ui, pos2, vec2};

use crate::dialogs::{Outcome, window};
use crate::icons::Icon;
use crate::model::Category;
use crate::regional::DateOrder;
use crate::settings::{self, Settings};
// Pages are laid out in English pixels: `fill` makes each one where it is on screen, `text`
// starts at an English pen, and a widget given a screen rect (from `r`) turns it back into its
// English rect (`winui::rect` both ways) before laying out its inside.
use crate::winui::{
    self, ButtonKind, LIGHT_BG, LIGHT_DISABLED, LIGHT_PAGE, LIGHT_TEXT, fill_l as fill, r, text,
};

pub const DUPLICATE: [&str; 4] = [
    crate::i18n::mark("Show a dialog and ask what to do."),
    crate::i18n::mark("Add the duplicate with a numbered file name"),
    crate::i18n::mark("Add the duplicate and overwrite the existing file"),
    crate::i18n::mark(
        "If existing file is complete, show download complete dialog; otherwise resume it.",
    ),
];
const SOUND_EVENTS: [&str; 4] = [
    crate::i18n::mark("Download complete"),
    crate::i18n::mark("Download failed"),
    crate::i18n::mark("Queue processing started"),
    crate::i18n::mark("Queue processing stopped/finished"),
];

/// A small entry dialog opened from a page.
enum Sub {
    /// Sites Logins → New / Edit: (row being edited, site/path, user, password).
    Login(Option<usize>, String, String, String),
    /// Connection → New / Edit exception: (row, server, number).
    Exception(Option<usize>, String, String),
    /// General → Download panels → Edit...: (the button over videos on, the sites it stays off).
    Panels(bool, String, String),
    /// General → Keys...: (also take the files the browser would open).
    Keys(bool),
    /// General → Test browser integration...: the checks, as found when it opened.
    Test(Vec<crate::extension::Check>),
    /// File types → "Don't start downloading automatically from the following addresses" →
    /// Edit list...: one address pattern per line.
    Addresses(String),
    /// Connection → DNS → Settings...: how the DNS is asked, its servers, a test.
    Dns(DnsEdit),
    /// Accounts → Add...: the service, a pasted token, and the sign-in running (#44).
    Account(AccountEdit),
    /// Remote → Pair a device...: the code shown, until a device pairs or it is closed.
    Pair,
}

/// Options → Remote's page number (the tabs' numbers).
const REMOTE_PAGE: usize = 11;

/// Connection → DNS's sub-dialog: a copy of the DNS fields, kept on OK.
struct DnsEdit {
    mode: u8,
    provider: String,
    servers: String,
    tls_name: String,
    doh_url: String,
    nextdns_id: String,
    fallback: bool,
    /// The Test button's answer (shared with its thread): None while it runs.
    test: Option<TestSlot>,
}

/// A test's answer line and whether it worked, filled in by its thread.
type TestSlot = std::sync::Arc<std::sync::Mutex<Option<(String, bool)>>>;

impl DnsEdit {
    fn from(s: &Settings) -> Self {
        DnsEdit {
            mode: if s.dns_mode == 0 { 3 } else { s.dns_mode },
            provider: s.dns_provider.clone(),
            servers: s.dns_servers.clone(),
            tls_name: s.dns_tls_name.clone(),
            doh_url: s.dns_doh_url.clone(),
            nextdns_id: s.dns_nextdns_id.clone(),
            fallback: s.dns_fallback,
            test: None,
        }
    }

    fn choice(&self) -> Result<sdl_engine::DnsChoice, String> {
        settings::dns_choice(
            self.mode,
            &self.provider,
            &self.servers,
            &self.tls_name,
            &self.doh_url,
            &self.nextdns_id,
            self.fallback,
        )
    }

    fn save(self, s: &mut Settings) {
        s.dns_mode = self.mode;
        s.dns_provider = self.provider;
        s.dns_servers = self.servers.trim().to_owned();
        s.dns_tls_name = self.tls_name.trim().to_owned();
        s.dns_doh_url = self.doh_url.trim().to_owned();
        s.dns_nextdns_id = self.nextdns_id.trim().to_lowercase();
        s.dns_fallback = self.fallback;
    }
}

/// Sites Logins → Cookies for video sites' browser at first (#229): the one chosen, else the
/// first browser whose extension reached SDL when yt-dlp reads it (SDL doesn't know the
/// system's default browser), else Firefox, which yt-dlp reads on every system.
fn cookie_browser_at(s: &Settings) -> usize {
    let browsers = sdl_tools::proc::cookie_browsers();
    let seen = sdl_ipc::extension::seen_browsers();
    let by_extension = seen.iter().find_map(|(name, _)| {
        let ytdlp = match name.as_str() {
            "Mozilla Firefox" => "firefox",
            "Google Chrome" => "chrome",
            "Microsoft Edge" => "edge",
            "Brave Browser" => "brave",
            "Opera" => "opera",
            "Apple Safari" => "safari",
            _ => return None,
        };
        browsers.iter().position(|b| *b == ytdlp)
    });
    browsers
        .iter()
        .position(|b| *b == s.video_cookies)
        .or(by_extension)
        .unwrap_or(0)
}

/// Connection → DNS's list: the system's, the services by name, the user's own.
fn dns_names() -> Vec<String> {
    let mut out = vec![
        // i18n: Options → Connection → DNS: the operating system's own DNS (the default)
        crate::i18n::t("The system's DNS").to_owned(),
    ];
    out.extend(sdl_engine::dns::PRESETS.iter().map(|p| p.name.to_owned()));
    // i18n: Options → Connection → DNS: a DNS server the user types in
    out.push(crate::i18n::t("Another DNS...").to_owned());
    out
}

fn dns_index(s: &Settings) -> usize {
    if s.dns_mode == 0 {
        return 0;
    }
    let presets = sdl_engine::dns::PRESETS;
    presets
        .iter()
        .position(|p| p.id == s.dns_provider)
        .map_or(presets.len() + 1, |i| i + 1)
}

fn set_dns_index(s: &mut Settings, i: usize) {
    let presets = sdl_engine::dns::PRESETS;
    if i == 0 {
        s.dns_mode = 0;
        return;
    }
    s.dns_provider = presets
        .get(i - 1)
        .map_or("custom".to_owned(), |p| p.id.to_owned());
    if s.dns_mode == 0 {
        s.dns_mode = 3;
    }
    // A service that isn't offered this way (NextDNS plain) moves to DNS over HTTPS.
    if let (Some(p), Some(m)) = (presets.get(i - 1), settings::dns_mode(s.dns_mode))
        && !p.offers(m)
    {
        s.dns_mode = 3;
    }
}

pub struct OptionsDialog {
    s: Settings,
    /// The settings it opened with: OK applies only what changed from them.
    pub from: Settings,
    page: usize,
    /// The browsers check list's scroll and selection.
    browser_rows: winui::RowList,
    category: usize,
    cat_dir: String,
    /// Connection → "Default max. conn. number", as typed.
    conns: String,
    mb: String,
    /// File types → "Don't take files smaller than", as typed (KB).
    min_kb: String,
    hours: String,
    /// The exceptions' and site logins' lists: scroll and selection.
    exc_rows: winui::RowList,
    login_rows: winui::RowList,
    /// Accounts' list (#44).
    account_rows: winui::RowList,
    /// The Sounds list's scroll and selection.
    sound_rows: winui::RowList,
    sub: Option<Sub>,
    /// Save to → New / Edit...: the category editor (D-021).
    cat_edit: Option<crate::categories::CategoryDialog>,
    /// Save to → Restore the built-in categories: its question (#93).
    restore_ask: Option<crate::dialogs::Confirm>,
    /// Connection caps the engine learned, shown as "(auto)" rows under Exceptions.
    learned: Vec<(String, u32)>,
    /// 7-Zip's program (Archives: RAR goes through it), looked for when the window opens.
    seven_zip: Option<std::path::PathBuf>,
    /// Proxy / Socks → Network adapter (#35): the computer's adapters, read again every 2 s.
    adapters: Option<(std::time::Instant, Vec<sdl_engine::net::Adapter>)>,
    /// Remote (#141): the running remote control (its state, paired devices, pairing), the
    /// port as typed, and the paired devices list's scroll and selection.
    remote: Option<sdl_remote::Remote>,
    port: String,
    device_rows: winui::RowList,
    /// Test browser integration → Set it up's runs seen finished (crate::extension).
    set_up_seen: u64,
    /// Sites Logins → Cookies for video sites (#229): the browser in the list (an index into
    /// `sdl_tools::proc::cookie_browsers`), kept while Off is chosen.
    cookie_browser: usize,
}

impl OptionsDialog {
    /// Open on tab `page` (the numbers in `tabs`: 0 General … 4 Connection, 5 Proxy / Socks,
    /// 6 Sites Logins, 7 Accounts, 8 Sounds, 9 Regional, 10 Archives); for `/show options:<tab>`.
    pub fn on_page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }

    /// With the app's remote control (Remote: its state, devices and pairing).
    pub fn with_remote(mut self, remote: Option<sdl_remote::Remote>) -> Self {
        self.remote = remote;
        self
    }

    /// With Remote → Pair a device... open (`/show options:pair`).
    pub fn with_pairing(mut self) -> Self {
        self.page = REMOTE_PAGE;
        self.pair();
        self
    }

    /// With General → Download panels → Edit... open (`/show options:panels`).
    pub fn with_panels(mut self) -> Self {
        self.sub = Some(Sub::Panels(
            self.s.video_panel,
            self.s.video_panel_sites.clone(),
            String::new(),
        ));
        self
    }

    /// With General → Test browser integration's window over it, or Accounts → Add's (`/show
    /// options:test`, `options:account`: screenshots, #192).
    pub fn with_window(mut self, name: &str) -> Self {
        match name {
            "test" => self.sub = Some(Sub::Test(crate::extension::checks(&self.s))),
            "account" => self.sub = Some(Sub::Account(AccountEdit::default())),
            _ => {}
        }
        self
    }

    pub fn new(
        settings: &Settings,
        downloads: &std::path::Path,
        learned: Vec<(String, u32)>,
    ) -> Self {
        let s = settings.clone();
        OptionsDialog {
            from: s.clone(),
            browser_rows: Default::default(),
            cat_dir: display_dir(&s.category_dir(Category::General, downloads)),
            conns: s.max_connections.clamp(1, s.connection_limit()).to_string(),
            mb: s.limits_mb.to_string(),
            min_kb: s.capture_min_kb.to_string(),
            hours: s.limits_hours.to_string(),
            s,
            page: 0,
            category: 0,
            exc_rows: Default::default(),
            login_rows: Default::default(),
            account_rows: Default::default(),
            sound_rows: Default::default(),
            sub: None,
            set_up_seen: 0,
            cat_edit: None,
            restore_ask: None,
            learned,
            seven_zip: crate::extractq::seven_zip(),
            adapters: None,
            remote: None,
            port: settings.remote_port.to_string(),
            device_rows: Default::default(),
            cookie_browser: cookie_browser_at(settings),
        }
    }

    /// `Done(settings)` on OK.
    pub fn show(&mut self, ctx: &egui::Context, downloads: &std::path::Path) -> Outcome<Settings> {
        let modal = self.sub.is_some() || self.cat_edit.is_some() || self.restore_ask.is_some();
        // The Fluent design keeps the original twelve page painters, but presents them
        // inside a spacious, separate navigation shell. Other themes remain pixel-identical.
        let fluent = crate::theme::modern_layout();
        let client = if fluent {
            crate::dialogs::shown_size(ctx, [1010.0, 720.0])
        } else {
            [430.0, 506.0]
        };
        let outcome = window(
            ctx,
            "options",
            "Swoopdl Configuration",
            client,
            false,
            LIGHT_BG(),
            |ui, o| {
                // Ctrl+Tab through the pages in the tabs' order (Archives, 10, came last).
                const ORDER: [usize; 12] = [0, 1, 2, 3, 4, 11, 5, 6, 7, 8, 9, 10];
                let at = ORDER.iter().position(|p| *p == self.page).unwrap_or(0);
                let by_key = winui::tab_keys(ui, ORDER.len(), at).map(|i| ORDER[i]);
                let page_o = o;
                if crate::theme::current().id.as_ref() == "fluent" {
                    graphite_settings_shell(ui, o, client);
                } else if fluent {
                    let p = ui.painter();
                    let c = crate::theme::colors();
                    p.rect_filled(
                        r(o, 229.0, 0.0, client[0] - 229.0, client[1] - 64.0),
                        0.0,
                        c.window,
                    );
                    p.rect_filled(
                        r(o, 0.0, client[1] - 64.0, client[0], 64.0),
                        0.0,
                        c.egui_window,
                    );
                    p.line_segment(
                        [
                            pos2(winui::mx(o.x), o.y + client[1] - 64.0),
                            pos2(winui::mx(o.x + client[0]), o.y + client[1] - 64.0),
                        ],
                        egui::Stroke::new(1.0, c.group_line),
                    );
                }
                let nav_choice = if fluent {
                    let navigation = r(o, 0.0, 0.0, 229.0, client[1] - 64.0);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(navigation), |ui| {
                        crate::scale::pane(
                            ui,
                            egui::Id::unique("fluent-settings-nav"),
                            vec2(229.0, 544.0),
                            |ui, origin| {
                                let _surface = winui::Surface::enter(origin.x, origin.x + 229.0);
                                fluent_options_nav(ui, origin, self.page)
                            },
                        )
                    })
                    .inner
                } else {
                    tabs(ui, o, self.page)
                };
                if let Some(p) = nav_choice.or(by_key) {
                    self.page = p;
                }
                if fluent {
                    let pane = r(
                        o,
                        245.0,
                        12.0,
                        (client[0] - 261.0).max(80.0),
                        (client[1] - 88.0).max(80.0),
                    );
                    ui.scope_builder(egui::UiBuilder::new().max_rect(pane), |ui| {
                        crate::scale::pane(
                            ui,
                            egui::Id::unique(("fluent-settings-page", self.page)),
                            vec2(720.0, modern_page_height(self.page)),
                            |ui, origin| {
                                // Preserve the existing page event handlers and scroll offsets;
                                // only the visual treatment is changed for modern themes.
                                let _surface = winui::Surface::enter(origin.x, origin.x + 720.0);
                                let page_origin = pos2(origin.x, origin.y - 45.0);
                                if !matches!(self.page, 0 | 1 | 3)
                                    && !(matches!(
                                        self.page,
                                        2 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | REMOTE_PAGE
                                    ) && matches!(
                                        crate::theme::current().id.as_ref(),
                                        "win" | "fluent"
                                    ))
                                {
                                    modern_legacy_page_surface(ui, page_origin, self.page);
                                }
                                let _font = (!matches!(self.page, 0 | 1 | 3)
                                    && !(matches!(
                                        self.page,
                                        2 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | REMOTE_PAGE
                                    ) && matches!(
                                        crate::theme::current().id.as_ref(),
                                        "win" | "fluent"
                                    )))
                                .then(winui::ModernCompactDialogFont::enter);
                                self.paint_page(ui, page_origin, downloads);
                            },
                        );
                    });
                } else {
                    self.paint_page(ui, page_o, downloads);
                }
                let footer_o = if fluent {
                    pos2(o.x + client[0] - 450.0, o.y + client[1] - 524.0)
                } else {
                    o
                };
                let _footer_font = fluent.then(winui::ModernDialogFont::enter);
                let ok = winui::button_light(
                    ui,
                    r(
                        footer_o,
                        if fluent { 350.0 } else { 269.0 },
                        477.0,
                        73.0,
                        if fluent { 34.0 } else { 21.0 },
                    ),
                    "OK",
                    if fluent {
                        ButtonKind::Accent
                    } else {
                        ButtonKind::Default
                    },
                    false,
                )
                .clicked();
                if winui::button_light(
                    ui,
                    r(
                        footer_o,
                        if fluent { 269.0 } else { 350.0 },
                        477.0,
                        73.0,
                        if fluent { 34.0 } else { 21.0 },
                    ),
                    // i18n: noun: button that opens the help
                    "Help",
                    ButtonKind::Normal,
                    false,
                )
                .clicked()
                {
                    crate::help::open_page("options.md");
                }
                if modal {
                    Outcome::Open
                } else if winui::escape_pressed(ui) {
                    Outcome::Closed
                } else if ok || winui::enter_pressed(ui) {
                    Outcome::Done(())
                } else {
                    Outcome::Open
                }
            },
        );
        if self.sub.is_some() {
            self.sub_dialog(ctx);
        }
        if let Some(d) = &mut self.cat_edit {
            match d.show(ctx, &crate::categories::names()) {
                Outcome::Open => {}
                Outcome::Closed => self.cat_edit = None,
                Outcome::Done(e) => {
                    self.cat_edit = None;
                    let cat = crate::categories::apply(&mut self.s, e, downloads);
                    // Show the category just made or changed.
                    if let Some(i) = Category::all().iter().position(|c| *c == cat) {
                        self.category = i;
                    }
                    self.cat_dir = display_dir(&self.s.category_dir(cat, downloads));
                }
            }
        }
        if let Some(d) = &mut self.restore_ask {
            match d.show(ctx) {
                Outcome::Open => {}
                Outcome::Closed => self.restore_ask = None,
                Outcome::Done(()) => {
                    self.restore_ask = None;
                    // In this draft: Cancel undoes it, as it undoes the dialog's other edits.
                    crate::categories::restore(&mut self.s);
                    let cats = Category::all();
                    self.category = self.category.min(cats.len() - 1);
                    self.cat_dir =
                        display_dir(&self.s.category_dir(cats[self.category], downloads));
                }
            }
        }
        match outcome {
            Outcome::Done(()) => {
                let mut s = self.s.clone();
                s.max_connections = self
                    .conns
                    .parse()
                    .unwrap_or(8)
                    .clamp(1, s.connection_limit());
                s.limits_mb = self.mb.parse().unwrap_or(200).max(1);
                s.capture_min_kb = self.min_kb.parse().unwrap_or(0);
                s.limits_hours = self.hours.parse().unwrap_or(5).max(1);
                s.remote_port = self.remote_port();
                Outcome::Done(s)
            }
            Outcome::Closed => Outcome::Closed,
            Outcome::Open => Outcome::Open,
        }
    }

    fn paint_page(&mut self, ui: &mut Ui, o: Pos2, downloads: &std::path::Path) {
        match self.page {
            0 => self.general(ui, o),
            1 => self.file_types(ui, o),
            2 => self.save_to(ui, o, downloads),
            3 => self.downloads(ui, o),
            4 => self.connection(ui, o),
            5 => self.proxy(ui, o),
            6 => self.logins(ui, o),
            7 => self.accounts(ui, o),
            9 => self.regional(ui, o),
            10 => self.archives(ui, o),
            REMOTE_PAGE => self.remote_page(ui, o),
            8 => self.sounds(ui, o),
            _ => self.regional(ui, o),
        }
    }

    // ---- pages --------------------------------------------------------------------------

    fn general(&mut self, ui: &mut Ui, o: Pos2) {
        let p = ui.painter().clone();
        let fluent = crate::theme::modern_layout();
        header(
            ui,
            o,
            Icon::PageGeneral,
            if fluent {
                "General"
            } else {
                "Browser/System Integration"
            },
            false,
            260.0,
            63.0,
            405.0,
        );
        let portable = sdl_ipc::portable_dir().is_some();
        let s = &mut self.s;
        if portable {
            s.launch_on_startup = false;
        }
        let checks: [(f32, &mut bool, &str); 7] = [
            (
                94.0,
                &mut s.launch_on_startup,
                crate::i18n::mark("Launch Swoopdl on startup"),
            ),
            (
                113.0,
                &mut s.clipboard_autostart,
                crate::i18n::mark("Offer to download links copied to the clipboard"),
            ),
            (
                133.0,
                &mut s.mv2_extensions,
                crate::i18n::mark("Extend the availability of Manifest 2 Chrome extensions"),
            ),
            (
                152.0,
                &mut s.advanced_integration,
                crate::i18n::mark("Use advanced browser integration"),
            ),
            // SDL's: turns back on the delete confirmation that "Don't show this dialog again"
            // turned off. The browsers' group below starts a row lower for it.
            (
                172.0,
                &mut s.confirm_delete,
                crate::i18n::mark("Confirm before deleting downloads"),
            ),
            // SDL's (#63): Minimize hides the main window and progress windows in the tray too.
            // The browsers' group starts a row lower again, its list a row shorter (it scrolls).
            (
                191.0,
                &mut s.minimize_to_tray,
                // i18n: checkbox: the Minimize button hides SDL's windows to the notification area (system tray) instead of the taskbar
                crate::i18n::mark("Minimize to the notification area"),
            ),
            // SDL's: the × hides the window in the notification area, or quits.
            (
                210.0,
                &mut s.close_to_tray,
                // i18n: checkbox: closing keeps SDL running, in the notification area when available or else the taskbar, instead of quitting
                crate::i18n::mark("Keep running when closing the window"),
            ),
        ];
        // Greyed where no tray shows SDL's icon (some Linux desktops): nothing could bring the
        // window back from there.
        let tray = crate::tray::available();
        for (i, (y, v, label)) in checks.into_iter().enumerate() {
            // Portable: nothing is written to the registry, so no start with Windows. Manifest 2
            // extensions: an IDM-era Chrome policy with no meaning for SDL's extension (MV3):
            // shown as IDM has it, greyed (D-021).
            let enabled = !(portable && i == 0) && i != 2 && (i < 5 || tray);
            let check = if fluent {
                let descriptions = [
                    "Automatically run after signing into your computer",
                    "Detect supported URLs copied to the clipboard",
                    "Legacy browser policy; unavailable for the current extension",
                    "Use compatible browser integration features",
                    "Ask before removing downloads",
                    "Keep Swoopdl accessible from the system tray",
                    "Continue downloads when the window is closed",
                ];
                fluent_setting(
                    ui,
                    o + vec2(22.0, 104.0 + i as f32 * 64.0),
                    v,
                    label,
                    descriptions[i],
                    enabled,
                )
            } else {
                winui::checkbox_light(ui, o + vec2(30.0, y), v, label, enabled)
            };
            // Where a start with the session leaves the window (#226).
            if i == 0 && enabled && check.hovered() {
                check.on_hover_text(if crate::can_start_in_tray() {
                    // i18n: tooltip of "Launch Swoopdl on startup": started with the computer, SDL's window stays hidden, its icon in the notification area (system tray)
                    crate::i18n::t("Started with the computer, it waits in the notification area")
                } else {
                    // i18n: the same tooltip where no notification area (system tray) shows SDL's icon (some Linux desktops): its window starts minimized instead
                    crate::i18n::t(
                        "Started with the computer, it starts minimized: no notification area shows its icon here",
                    )
                });
            } else if i == 6 && check.hovered() {
                // i18n: tooltip of the close-setting: checked keeps SDL running when the window closes, in the notification area when available or else the taskbar; downloads continue; unchecked closing quits
                check.on_hover_text(crate::i18n::t(
                    "Checked, closing the window keeps SDL running: it hides in the notification area when available, or minimizes to the taskbar otherwise, and downloads continue; unchecked, closing quits SDL",
                ));
            }
        }
        // Modern pages use a true full-width browser section; keep the original
        // coordinates below for Classic, Metro and High Contrast modes.
        if fluent {
            self.general_browser_modern(ui, o);
            return;
        }
        // The browser list and its original actions follow the classic controls.
        group(
            ui,
            o,
            16.0,
            234.0,
            if fluent { 670.0 } else { 392.0 },
            160.0,
            "Capture downloads from the following browsers:",
        );
        // Check list: one row per browser in the shared row list (Space ticks the selected
        // one; the box and its name tick it too).
        let browsers = &mut s.browsers;
        // Named by its group's caption, which says what the list is (#102).
        self.browser_rows.name = crate::i18n::t("Capture downloads from the following browsers:");
        let ev = winui::row_list(
            ui,
            "browsers",
            r(o, 30.0, 244.0, if fluent { 640.0 } else { 362.0 }, 78.0),
            browsers.len(),
            17.0,
            &mut self.browser_rows,
            |ui, clip, i, row, look| {
                let (name, on) = &mut browsers[i];
                look.name = name.clone();
                winui::row_check(
                    ui,
                    clip,
                    pos2(row.left() + 5.0, row.top() + 2.0),
                    on,
                    name,
                    ("browser", i),
                );
                // Its name in the row's ink (white on the selection).
                text(clip, row.left() + 23.0, row.top() + 12.0, name, look.ink);
            },
        );
        if let Some(i) = ev.space {
            browsers[i].1 = !browsers[i].1;
        }
        if ev.focused {
            self.browser_rows
                .type_to(ui, "browsers", browsers.len(), |i| browsers[i].0.clone());
        }
        // Adding browsers and the browser menu editor are the extension's settings (M8): shown as
        // IDM has them, not yet available.
        let off = |ui: &mut Ui, rc: Rect, label: &str| {
            winui::button_light(ui, rc, label, ButtonKind::Disabled, false);
        };
        // Both as wide as their labels in this language (#192), within the group box.
        let [test_at, add_at] = winui::button_row(
            &p,
            &[
                ("Test browser integration...", 30.0, 180.0),
                ("Add browser...", 286.0, 106.0),
            ],
            8.0,
            30.0,
            260.0,
        )[..] else {
            unreachable!("two buttons")
        };
        off(
            ui,
            r(o, add_at.0, 328.0, add_at.1, 21.0),
            crate::i18n::mark("Add browser..."),
        );
        // SDL's: what stands between a browser and SDL, checked in order.
        if winui::button_light(
            ui,
            r(o, test_at.0, 328.0, test_at.1, 21.0),
            "Test browser integration...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Test(crate::extension::checks(&self.s)));
        }
        winui::etched_h(&p, o.x + 34.0, o.y + 356.0, 357.0);
        // The buttons at the right as wide as their labels in this language (#192), their
        // captions ending 6 px before them.
        let fit_right =
            |label: &str| winui::buttons_to_right(&p, 392.0, 0.0, &[(label, 73.0)], 160.0)[0];
        let keys_at = fit_right("Keys...");
        let label = crate::i18n::t("Customize keys to prevent or force downloading with SDL");
        // Inside the browsers' group box: it starts no further out than the group's frame.
        let (rect, cut) = winui::text_right_within(
            &p,
            o.x + keys_at.0 - 6.0,
            o.y + 379.0,
            label,
            LIGHT_TEXT(),
            o.x + 22.0,
        );
        if cut {
            winui::full_on_hover(ui, rect, ("general-right", 379), label);
        }
        if winui::button_light(
            ui,
            r(o, keys_at.0, 365.0, keys_at.1, 21.0),
            "Keys...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Keys(self.s.capture_opened));
        }
        let label = crate::i18n::t("Customize SDL menu items in context menu of browsers");
        let (rect, cut) = winui::text_right(&p, o.x + 313.0, o.y + 419.0, label, LIGHT_DISABLED());
        if cut {
            winui::full_on_hover(ui, rect, ("general-right", 419), label);
        }
        // i18n: verb: edit the item
        off(ui, r(o, 289.0, 406.0, 103.0, 21.0), "Edit...");
        // The download button over videos in the browsers (our own panel, Sub::Panels).
        let label = crate::i18n::t("Customize SDL Download panels in browsers");
        let panels_at = fit_right("Edit...");
        let (rect, cut) = winui::text_right(
            &p,
            o.x + panels_at.0 - 6.0,
            o.y + 452.0,
            label,
            LIGHT_TEXT(),
        );
        if cut {
            winui::full_on_hover(ui, rect, ("general-right", 452), label);
        }
        if winui::button_light(
            ui,
            r(o, panels_at.0, 438.0, panels_at.1, 21.0),
            // i18n: verb: edit the item
            "Edit...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Panels(
                self.s.video_panel,
                self.s.video_panel_sites.clone(),
                String::new(),
            ));
        }
    }

    /// Full-width browser integration area. Uses the same mutable browser rows and
    /// the original Test/Keys/Panels commands as the legacy General form.
    fn general_browser_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let p = ui.painter().clone();
        settings_card(ui, o, 572.0, 230.0);
        settings_label(
            ui,
            o,
            598.0,
            "Capture downloads from the following browsers:",
        );
        let browsers = &mut self.s.browsers;
        self.browser_rows.name = crate::i18n::t("Capture downloads from the following browsers:");
        let event = winui::row_list(
            ui,
            "browsers",
            r(o, 32.0, 612.0, 628.0, 129.0),
            browsers.len(),
            24.0,
            &mut self.browser_rows,
            |ui, clip, i, row, look| {
                let (name, on) = &mut browsers[i];
                look.name = name.clone();
                winui::row_check(
                    ui,
                    clip,
                    pos2(row.left() + 7.0, row.top() + 5.0),
                    on,
                    name,
                    ("browser", i),
                );
                text(clip, row.left() + 28.0, row.top() + 17.0, name, look.ink);
            },
        );
        if let Some(i) = event.space {
            browsers[i].1 = !browsers[i].1;
        }
        if event.focused {
            self.browser_rows
                .type_to(ui, "browsers", browsers.len(), |i| browsers[i].0.clone());
        }
        if winui::button_light(
            ui,
            r(o, 32.0, 754.0, 208.0, 34.0),
            "Test browser integration...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Test(crate::extension::checks(&self.s)));
        }
        winui::button_light(
            ui,
            r(o, 250.0, 754.0, 160.0, 34.0),
            "Add browser...",
            ButtonKind::Disabled,
            false,
        );

        settings_card(ui, o, 814.0, 206.0);
        settings_label(ui, o, 841.0, "Browser download controls");
        let label = crate::i18n::t("Customize keys to prevent or force downloading with SDL");
        let shown = winui::ellipsize(&p, &winui::tahoma(), label, 468.0);
        let area = text(&p, o.x + 32.0, o.y + 877.0, &shown, LIGHT_TEXT());
        if shown != label {
            winui::full_on_hover(ui, area, "general-modern-keys", label);
        }
        if winui::button_light(
            ui,
            r(o, 544.0, 853.0, 116.0, 34.0),
            "Keys...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Keys(self.s.capture_opened));
        }
        let label = crate::i18n::t("Customize SDL menu items in context menu of browsers");
        let shown = winui::ellipsize(&p, &winui::tahoma(), label, 468.0);
        let area = text(&p, o.x + 32.0, o.y + 928.0, &shown, LIGHT_DISABLED());
        if shown != label {
            winui::full_on_hover(ui, area, "general-modern-menu", label);
        }
        winui::button_light(
            ui,
            r(o, 544.0, 904.0, 116.0, 34.0),
            "Edit...",
            ButtonKind::Disabled,
            false,
        );
        let label = crate::i18n::t("Customize SDL Download panels in browsers");
        let shown = winui::ellipsize(&p, &winui::tahoma(), label, 468.0);
        let area = text(&p, o.x + 32.0, o.y + 977.0, &shown, LIGHT_TEXT());
        if shown != label {
            winui::full_on_hover(ui, area, "general-modern-panels", label);
        }
        if winui::button_light(
            ui,
            r(o, 544.0, 953.0, 116.0, 34.0),
            "Edit...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Panels(
                self.s.video_panel,
                self.s.video_panel_sites.clone(),
                String::new(),
            ));
        }
    }

    fn file_types(&mut self, ui: &mut Ui, o: Pos2) {
        if crate::theme::modern_layout() {
            self.file_types_modern(ui, o);
            return;
        }
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageFileTypes,
            "Downloaded file types",
            true,
            254.0,
            63.0,
            405.0,
        );
        text(
            &p,
            o.x + 21.0,
            o.y + 103.0,
            crate::i18n::t("Automatically start downloading the following file types:"),
            LIGHT_TEXT(),
        );
        multiline(
            ui,
            r(o, 22.0, 110.0, 380.0, 59.0),
            &mut self.s.auto_types,
            "auto-types",
            true,
        );
        // Default as wide as its label in this language (#192), growing to the left.
        let default_at = winui::buttons_to_right(&p, 389.0, 0.0, &[("Default", 73.0)], 200.0)[0];
        if winui::button_light(
            ui,
            r(o, default_at.0, 175.0, default_at.1, 21.0),
            // i18n: button: put back the default list
            "Default",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.s.auto_types = settings::DEFAULT_AUTO_TYPES.into();
        }
        text(
            &p,
            o.x + 21.0,
            o.y + 222.0,
            crate::i18n::t("Don't start downloading automatically from the following sites:"),
            LIGHT_TEXT(),
        );
        multiline(
            ui,
            r(o, 22.0, 229.0, 380.0, 49.0),
            &mut self.s.excluded_sites,
            "excluded-sites",
            true,
        );
        let spaces = text(
            &p,
            o.x + 21.0,
            o.y + 293.0,
            crate::i18n::t("(separate names by spaces)"),
            LIGHT_TEXT(),
        );
        // Short of the note before it.
        let default_at = winui::button_row(
            &p,
            &[("Default", 316.0, 73.0)],
            0.0,
            winui::rect(spaces).right() - o.x + 8.0,
            200.0,
        )[0];
        if winui::button_light(
            ui,
            r(o, default_at.0, 284.0, default_at.1, 21.0),
            // i18n: button: put back the default list
            "Default",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.s.excluded_sites = settings::DEFAULT_EXCLUDED_SITES.into();
        }
        text(
            &p,
            o.x + 21.0,
            o.y + 327.0,
            crate::i18n::t("Don't start downloading automatically from the following addresses:"),
            LIGHT_TEXT(),
        );
        let (list_x, list_w) =
            winui::button_row_from_left(&p, &[("Edit list ...", 22.0, 135.0)], 0.0, 389.0, 260.0)
                [0];
        if winui::button_light(
            ui,
            r(o, list_x, 334.0, list_w, 21.0),
            "Edit list ...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Addresses(self.s.excluded_addresses.join("\n")));
        }
        // SDL's: a receipt or an icon isn't worth a download window.
        let label = crate::i18n::t("Don't take files smaller than (KB, 0 = any size):");
        // Before its box: a longer translation is cut with "..." and shown whole on hover.
        let (rect, cut) = winui::label_into(
            &p,
            o.x + 21.0,
            o.y + 383.0,
            label,
            LIGHT_TEXT(),
            o.x + 316.0,
            o.x + 21.0,
        );
        if cut {
            winui::full_on_hover(ui, rect, ("types-min", 383), label);
        }
        winui::number_field(
            ui,
            r(o, 316.0, 370.0, 73.0, 21.0),
            &mut self.min_kb,
            0,
            10_000_000,
            true,
            LIGHT_PAGE(),
        );
    }

    /// The wide settings shell needs real page geometry, not a narrow legacy form.
    /// Keep the existing draft fields, reset values, address editor and numeric validation.
    fn file_types_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        header(
            ui,
            o,
            Icon::PageFileTypes,
            "Downloaded file types",
            true,
            254.0,
            63.0,
            405.0,
        );
        let p = ui.painter().clone();
        settings_card(ui, o, 108.0, 172.0);
        settings_label(
            ui,
            o,
            133.0,
            "Automatically start downloading the following file types:",
        );
        multiline(
            ui,
            r(o, 32.0, 146.0, 628.0, 80.0),
            &mut self.s.auto_types,
            "auto-types",
            true,
        );
        let width = winui::button_width(&p, "Default", 108.0).min(628.0);
        if winui::button_light(
            ui,
            r(o, 660.0 - width, 236.0, width, 32.0),
            "Default",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.s.auto_types = settings::DEFAULT_AUTO_TYPES.into();
        }
        settings_card(ui, o, 292.0, 172.0);
        settings_label(
            ui,
            o,
            317.0,
            "Don't start downloading automatically from the following sites:",
        );
        multiline(
            ui,
            r(o, 32.0, 330.0, 628.0, 78.0),
            &mut self.s.excluded_sites,
            "excluded-sites",
            true,
        );
        let note = crate::i18n::t("(separate names by spaces)");
        let shown = winui::ellipsize(&p, &winui::tahoma(), note, (610.0 - width).max(0.0));
        let note_rect = winui::text(
            &p,
            o.x + 32.0,
            o.y + 437.0,
            &shown,
            crate::theme::colors().muted,
        );
        if shown != note {
            winui::full_on_hover(ui, note_rect, "file-types-spaces", note);
        }
        if winui::button_light(
            ui,
            r(o, 660.0 - width, 420.0, width, 32.0),
            "Default",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.s.excluded_sites = settings::DEFAULT_EXCLUDED_SITES.into();
        }
        settings_card(ui, o, 476.0, 100.0);
        settings_label(
            ui,
            o,
            501.0,
            "Don't start downloading automatically from the following addresses:",
        );
        let edit_width = winui::button_width(&p, "Edit list ...", 146.0).min(628.0);
        if winui::button_light(
            ui,
            r(o, 32.0, 524.0, edit_width, 36.0),
            "Edit list ...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Addresses(self.s.excluded_addresses.join("\n")));
        }
        settings_card(ui, o, 588.0, 100.0);
        settings_label(
            ui,
            o,
            613.0,
            "Don't take files smaller than (KB, 0 = any size):",
        );
        winui::number_field(
            ui,
            r(o, 32.0, 631.0, 160.0, 36.0),
            &mut self.min_kb,
            0,
            10_000_000,
            true,
            LIGHT_PAGE(),
        );
    }

    fn save_to(&mut self, ui: &mut Ui, o: Pos2, downloads: &std::path::Path) {
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            self.save_to_modern(ui, o, downloads);
            return;
        }
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageSaveTo,
            "Categories, file types, folders",
            true,
            201.0,
            58.0,
            398.0,
        );
        group(ui, o, 21.0, 94.0, 380.0, 201.0, "Save To...");
        // i18n: noun: label before the list of categories
        text(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 127.0,
            crate::i18n::t("Category"),
            LIGHT_TEXT(),
        );
        // IDM's, Torrents and the user's (the registry follows this dialog's edits).
        let cats = Category::all();
        self.category = self.category.min(cats.len() - 1);
        let names: Vec<String> = cats.iter().map(|c| c.label().into_owned()).collect();
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        if let Some(i) = winui::combo_light(
            ui,
            settings_form_rect(o, 30.0, 136.0, 212.0, 21.0),
            &labels,
            self.category,
            true,
            "opt-category",
        ) {
            // Keep the folder typed for the category being left.
            self.commit_cat_dir(downloads);
            self.category = i;
            self.cat_dir = display_dir(&self.s.category_dir(cats[i], downloads));
        }
        // New / Edit... (D-021): a category of the user's; a built-in one's folder. One width for
        // both, as wide as the longer label in this language (#192), into the room after the
        // list (it ends at 242).
        let (col_x, col_w) = winui::buttons_to_right(
            &p,
            392.0,
            0.0,
            &[(winui::widest_label(&p, &["New", "Edit..."]), 73.0)],
            142.0,
        )[0];
        if winui::button_light(
            ui,
            settings_form_rect(o, col_x, 105.0, col_w, 21.0),
            // i18n: button: create a new item
            "New",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.commit_cat_dir(downloads);
            self.cat_edit = Some(crate::categories::CategoryDialog::new());
        }
        if winui::button_light(
            ui,
            settings_form_rect(o, col_x, 137.0, col_w, 21.0),
            // i18n: verb: edit the item
            "Edit...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.commit_cat_dir(downloads);
            let cat = cats[self.category];
            let folder = self.s.category_dir(cat, downloads);
            self.cat_edit = Some(crate::categories::CategoryDialog::edit(cat, &folder));
        }
        let cat = cats[self.category];
        let label = cat.shown();
        let with_category = |template: &str| crate::i18n::fmt(template, &[("category", &label)]);
        let line = with_category(crate::i18n::t(
            "Automatically put in \"{category}\" category the following file types:",
        ));
        winui::text_in_window(
            ui,
            &p,
            settings_form_x(o.x, 31.0),
            o.y + 178.0,
            &line,
            LIGHT_TEXT(),
        );
        let types = cat.types_text();
        readonly(ui, settings_form_rect(o, 31.0, 185.0, 362.0, 23.0), &types);
        let dir_line = with_category(crate::i18n::t(
            "Default download directory for \"{category}\" category",
        ));
        winui::text_in_window(
            ui,
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 230.0,
            &dir_line,
            LIGHT_TEXT(),
        );
        // Browse as wide as its label in this language (#192); the box ends 7 px before it.
        let (browse_x, browse_w) =
            winui::buttons_to_right(&p, 392.0, 0.0, &[("Browse", 73.0)], 160.0)[0];
        winui::edit_light_on(
            ui,
            settings_form_rect(o, 31.0, 239.0, browse_x - 7.0 - 31.0, 23.0),
            &mut self.cat_dir,
            true,
            false,
            LIGHT_PAGE(),
        );
        if winui::button_light(
            ui,
            settings_form_rect(o, browse_x, 240.0, browse_w, 21.0),
            // i18n: verb: choose a folder
            "Browse",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            let (title, from) = (dir_line.clone(), self.cat_dir.clone());
            winui::start_pick(ui.ctx(), "options-category-folder", move || {
                rfd::FileDialog::new()
                    .set_title(title)
                    .set_directory(from)
                    .pick_folder()
            });
        }
        if let Some(dir) = winui::picked(ui.ctx(), "options-category-folder") {
            self.cat_dir = display_dir(&dir);
        }
        self.commit_cat_dir(downloads);
        let line = with_category(crate::i18n::t(
            "Change folder for \"{category}\" category on last selected",
        ));
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 33.0, 271.0),
            &mut self.s.change_folder_on_last,
            &line,
            true,
        );
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 33.0, 306.0),
            &mut self.s.set_creation_date,
            "Set file creation date as provided by the server",
            true,
        );
        group(ui, o, 21.0, 334.0, 380.0, 118.0, "Temporary directory");
        // D-004: no temporary directory — parts are written next to the file, so there is
        // nothing to assemble at the end.
        let none = crate::i18n::t("Not used: file parts are written next to the file");
        let (browse_x, browse_w) =
            winui::buttons_to_right(&p, 390.0, 0.0, &[("Browse", 73.0)], 160.0)[0];
        readonly(
            ui,
            settings_form_rect(o, 30.0, 349.0, browse_x - 6.0 - 30.0, 23.0),
            none,
        );
        winui::button_light(
            ui,
            settings_form_rect(o, browse_x, 350.0, browse_w, 21.0),
            // i18n: verb: choose a folder
            "Browse",
            ButtonKind::Disabled,
            false,
        );
        winui::paragraph(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 387.0,
            crate::i18n::t(
                "Swoopdl writes every connection straight into the final\nfile (a sparse part file next to it), so no temporary directory and no\nassembling step are needed.",
            ),
            LIGHT_TEXT(),
            settings_form_x(o.x, 426.0),
            13.0,
        );
        // #93: the built-in categories by file type as shipped again (beside OK, on this page).
        if winui::button_light(
            ui,
            settings_form_rect(o, 21.0, 477.0, 232.0, 21.0),
            "Restore the built-in categories",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.commit_cat_dir(downloads);
            let text = crate::i18n::fmt(
                crate::i18n::t(
                    "Bring back {names} as they were shipped: their names, file types and folders? Your own categories, General and Torrents stay as they are. Downloads already moved to General stay there.",
                ),
                &[("names", &crate::categories::restored_names())],
            );
            self.restore_ask = Some(crate::dialogs::Confirm::new(
                crate::i18n::mark("Restore the built-in categories"),
                text,
            ));
        }
    }

    /// Dedicated Win/Fluent page layout. The same category draft, folder picker,
    /// New/Edit category windows and restore confirmation are used as in the legacy page.
    /// Coordinates and clickable rectangles are composed together (no 1.55x transform).
    fn save_to_modern(&mut self, ui: &mut Ui, o: Pos2, downloads: &std::path::Path) {
        let _font = winui::ModernDialogFont::enter();
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageSaveTo,
            "Categories, file types, folders",
            true,
            201.0,
            58.0,
            398.0,
        );
        settings_card(ui, o, 108.0, 358.0);
        modern_section_title(ui, o, 138.0, "Save To...");
        modern_field_label(ui, o, 168.0, "Category");

        let cats = Category::all();
        self.category = self.category.min(cats.len() - 1);
        let names: Vec<String> = cats.iter().map(|c| c.label().into_owned()).collect();
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 32.0, 179.0, 438.0, 35.0),
            &labels,
            self.category,
            true,
            "opt-category",
        ) {
            self.commit_cat_dir(downloads);
            self.category = i;
            self.cat_dir = display_dir(&self.s.category_dir(cats[i], downloads));
        }
        if winui::button_light(
            ui,
            r(o, 482.0, 179.0, 80.0, 35.0),
            "New",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.commit_cat_dir(downloads);
            self.cat_edit = Some(crate::categories::CategoryDialog::new());
        }
        if winui::button_light(
            ui,
            r(o, 573.0, 179.0, 87.0, 35.0),
            "Edit...",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.commit_cat_dir(downloads);
            let cat = cats[self.category];
            let folder = self.s.category_dir(cat, downloads);
            self.cat_edit = Some(crate::categories::CategoryDialog::edit(cat, &folder));
        }

        let cat = cats[self.category];
        let label = cat.shown();
        let with_category = |template: &str| crate::i18n::fmt(template, &[("category", &label)]);
        let types_label = with_category(crate::i18n::t(
            "Automatically put in \"{category}\" category the following file types:",
        ));
        modern_field_label(ui, o, 241.0, &types_label);
        let types = cat.types_text();
        readonly(ui, r(o, 32.0, 252.0, 628.0, 35.0), &types);

        let dir_line = with_category(crate::i18n::t(
            "Default download directory for \"{category}\" category",
        ));
        modern_field_label(ui, o, 316.0, &dir_line);
        winui::edit_light_on(
            ui,
            r(o, 32.0, 327.0, 514.0, 35.0),
            &mut self.cat_dir,
            true,
            false,
            LIGHT_PAGE(),
        );
        if winui::button_light(
            ui,
            r(o, 558.0, 327.0, 102.0, 35.0),
            "Browse",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            let (title, from) = (dir_line.clone(), self.cat_dir.clone());
            winui::start_pick(ui.ctx(), "options-category-folder", move || {
                rfd::FileDialog::new()
                    .set_title(title)
                    .set_directory(from)
                    .pick_folder()
            });
        }
        if let Some(dir) = winui::picked(ui.ctx(), "options-category-folder") {
            self.cat_dir = display_dir(&dir);
        }
        self.commit_cat_dir(downloads);

        let last = with_category(crate::i18n::t(
            "Change folder for \"{category}\" category on last selected",
        ));
        winui::checkbox_light(
            ui,
            o + vec2(34.0, 380.0),
            &mut self.s.change_folder_on_last,
            &last,
            true,
        );
        winui::checkbox_light(
            ui,
            o + vec2(34.0, 420.0),
            &mut self.s.set_creation_date,
            "Set file creation date as provided by the server",
            true,
        );

        settings_card(ui, o, 483.0, 165.0);
        modern_section_title(ui, o, 512.0, "Temporary directory");
        let none = crate::i18n::t("Not used: file parts are written next to the file");
        readonly(ui, r(o, 32.0, 525.0, 514.0, 35.0), none);
        winui::button_light(
            ui,
            r(o, 558.0, 525.0, 102.0, 35.0),
            "Browse",
            ButtonKind::Disabled,
            false,
        );
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 589.0,
            crate::i18n::t(
                "Swoopdl writes every connection straight into the final\nfile (a sparse part file next to it), so no temporary directory and no\nassembling step are needed.",
            ),
            crate::theme::colors().muted,
            o.x + 646.0,
            17.0,
        );
        if winui::button_light(
            ui,
            r(o, 16.0, 668.0, 284.0, 36.0),
            "Restore the built-in categories",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            self.commit_cat_dir(downloads);
            let text = crate::i18n::fmt(
                crate::i18n::t(
                    "Bring back {names} as they were shipped: their names, file types and folders? Your own categories, General and Torrents stay as they are. Downloads already moved to General stay there.",
                ),
                &[("names", &crate::categories::restored_names())],
            );
            self.restore_ask = Some(crate::dialogs::Confirm::new(
                crate::i18n::mark("Restore the built-in categories"),
                text,
            ));
        }
    }

    /// The typed folder for the selected category becomes its own (or the default again).
    fn commit_cat_dir(&mut self, downloads: &std::path::Path) {
        let cats = Category::all();
        let Some(&cat) = cats.get(self.category) else {
            return;
        };
        let default = display_dir(&cat.default_folder(downloads));
        let typed = self.cat_dir.trim();
        let own = (!typed.is_empty()
            && typed.trim_end_matches('\\') != default.trim_end_matches('\\'))
        .then(|| typed.to_owned());
        self.s.set_category_dir(cat, own);
    }

    /// Fluent/Aqua settings presentation. Reuses the same draft fields and commit path as
    /// the legacy page; the two unavailable editor actions remain explicitly disabled.
    fn downloads_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageDownloads,
            "Default download settings",
            true,
            222.0,
            58.0,
            398.0,
        );
        let s = &mut self.s;
        const ROWS: [(f32, &str, &str); 7] = [
            (
                112.0,
                "Show start download dialog",
                "Open the file information dialog before starting.",
            ),
            (
                182.0,
                "Do not start downloading, only add files to the queue",
                "Queue new files instead of starting them immediately.",
            ),
            (
                252.0,
                "Show download complete dialog",
                "Notify when a download has finished.",
            ),
            (
                322.0,
                "Start downloading immediately while displaying \"Download File Info\" dialog",
                "Begin the transfer while file details are open.",
            ),
            (
                392.0,
                "Show queue selection panel on pressing \"Download Later\" button",
                "Ask which queue should receive a postponed download.",
            ),
            (
                462.0,
                "Show queue selection panel on closing batch download dialogs",
                "Choose a queue when closing batch download dialogs.",
            ),
            (
                532.0,
                "Ignore file modification time changes when resuming a download",
                "Resume without comparing the remote file modification time.",
            ),
        ];
        let toggles: [&mut bool; 7] = [
            &mut s.show_start_dialog,
            &mut s.only_add_to_queue,
            &mut s.show_complete,
            &mut s.start_immediately,
            &mut s.queue_panel_later,
            &mut s.queue_panel_batch,
            &mut s.ignore_mtime,
        ];
        // The second option is unavailable while the start dialog is enabled, exactly
        // as on the original page. Do not change its stored value while disabled.
        let allow_queue_only = !*toggles[0];
        for (i, (y, label, detail)) in ROWS.iter().enumerate() {
            let enabled = i != 1 || allow_queue_only;
            fluent_setting(
                ui,
                pos2(o.x + 16.0, o.y + *y),
                &mut *toggles[i],
                label,
                detail,
                enabled,
            );
        }
        settings_card(ui, o, 620.0, 118.0);
        settings_label(ui, o, 647.0, "If a duplicate download link is added:");
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 32.0, 663.0, 628.0, 36.0),
            &DUPLICATE,
            s.duplicate_action as usize,
            true,
            "opt-duplicate",
        ) {
            s.duplicate_action = i as u8;
        }
        settings_card(ui, o, 750.0, 118.0);
        settings_label(ui, o, 776.0, "User-Agent for manually added downloads:");
        winui::edit_light(
            ui,
            r(o, 32.0, 792.0, 628.0, 36.0),
            &mut s.user_agent,
            true,
            false,
        );
        settings_card(ui, o, 880.0, 125.0);
        winui::text_font(
            &p,
            winui::segoe(),
            o.x + 32.0,
            o.y + 910.0,
            crate::i18n::t("Customize \"Download progress\" dialog"),
            LIGHT_DISABLED(),
        );
        winui::button_light(
            ui,
            r(o, 548.0, 890.0, 112.0, 34.0),
            "Edit...",
            ButtonKind::Disabled,
            false,
        );
        winui::text_font(
            &p,
            winui::segoe(),
            o.x + 32.0,
            o.y + 976.0,
            crate::i18n::t("Virus checking settings"),
            LIGHT_DISABLED(),
        );
        winui::button_light(
            ui,
            r(o, 548.0, 956.0, 112.0, 34.0),
            "Edit...",
            ButtonKind::Disabled,
            false,
        );
    }

    fn downloads(&mut self, ui: &mut Ui, o: Pos2) {
        if crate::theme::modern_layout() {
            self.downloads_modern(ui, o);
            return;
        }
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageDownloads,
            "Default download settings",
            true,
            222.0,
            58.0,
            398.0,
        );
        let s = &mut self.s;
        // Edit... as wide as its label in this language (#192), its caption before it.
        let (edit_x, edit_w) =
            winui::buttons_to_right(&p, 392.0, 0.0, &[("Edit...", 73.0)], 160.0)[0];
        winui::text_right(
            &p,
            o.x + edit_x - 13.0,
            o.y + 104.0,
            crate::i18n::t("Customize \"Download progress\" dialog"),
            LIGHT_DISABLED(),
        );
        // The progress dialog customisation dialog is not built yet.
        winui::button_light(
            ui,
            r(o, edit_x, 89.0, edit_w, 21.0),
            // i18n: verb: edit the item
            "Edit...",
            ButtonKind::Disabled,
            false,
        );
        winui::checkbox_light(
            ui,
            o + vec2(24.0, 120.0),
            &mut s.show_start_dialog,
            "Show start download dialog",
            true,
        );
        let on = s.show_start_dialog;
        winui::checkbox_light(
            ui,
            o + vec2(50.0, 141.0),
            &mut s.only_add_to_queue,
            "Do not start downloading, only add files to the queue",
            !on,
        );
        winui::checkbox_light(
            ui,
            o + vec2(24.0, 160.0),
            &mut s.show_complete,
            "Show download complete dialog",
            true,
        );
        text(
            &p,
            o.x + 31.0,
            o.y + 191.0,
            crate::i18n::t("Note: These settings don't relate to queue processing"),
            LIGHT_TEXT(),
        );
        winui::etched_h(&p, o.x + 21.0, o.y + 198.0, 381.0);
        winui::checkbox_light(
            ui,
            o + vec2(24.0, 212.0),
            &mut s.start_immediately,
            "Start downloading immediately while displaying \"Download File Info\" dialog",
            true,
        );
        winui::checkbox_light(
            ui,
            o + vec2(24.0, 241.0),
            &mut s.queue_panel_later,
            "Show queue selection panel on pressing \"Download Later\" button",
            true,
        );
        winui::checkbox_light(
            ui,
            o + vec2(24.0, 266.0),
            &mut s.queue_panel_batch,
            "Show queue selection panel on closing batch download dialogs",
            true,
        );
        winui::checkbox_light(
            ui,
            o + vec2(24.0, 290.0),
            &mut s.ignore_mtime,
            "Ignore file modification time changes when resuming a download",
            true,
        );
        winui::etched_h(&p, o.x + 21.0, o.y + 314.0, 381.0);
        text(
            &p,
            o.x + 21.0,
            o.y + 334.0,
            crate::i18n::t("If a duplicate download link is added:"),
            LIGHT_TEXT(),
        );
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 21.0, 340.0, 380.0, 21.0),
            &DUPLICATE,
            s.duplicate_action as usize,
            true,
            "opt-duplicate",
        ) {
            s.duplicate_action = i as u8;
        }
        winui::etched_h(&p, o.x + 21.0, o.y + 367.0, 381.0);
        text(
            &p,
            o.x + 21.0,
            o.y + 388.0,
            crate::i18n::t("User-Agent for manually added downloads:"),
            LIGHT_TEXT(),
        );
        winui::edit_light(
            ui,
            r(o, 21.0, 398.0, 381.0, 23.0),
            &mut s.user_agent,
            true,
            false,
        );
        winui::etched_h(&p, o.x + 21.0, o.y + 426.0, 381.0);
        winui::text_right(
            &p,
            o.x + edit_x - 17.0,
            o.y + 451.0,
            crate::i18n::t("Virus checking settings"),
            LIGHT_DISABLED(),
        );
        winui::button_light(
            ui,
            r(o, edit_x, 436.0, edit_w, 21.0),
            // i18n: verb: edit the item
            "Edit...",
            ButtonKind::Disabled,
            false,
        );
    }

    fn connection(&mut self, ui: &mut Ui, o: Pos2) {
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            self.connection_modern(ui, o);
            return;
        }
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageConnection,
            "Connections and Limits",
            true,
            245.0,
            63.0,
            399.0,
        );
        group(ui, o, 21.0, 102.0, 380.0, 195.0, "Max. connections number");
        let label = crate::i18n::t("Default max. conn. number");
        let (rect, cut) = winui::label_into(
            &p,
            settings_form_x(o.x, 130.0),
            o.y + 126.0,
            label,
            LIGHT_TEXT(),
            settings_form_x(o.x, 268.0),
            settings_form_x(o.x, 30.0),
        );
        if cut {
            winui::full_on_hover(ui, rect, "max-conns", label);
        }
        // Typed, or stepped with the arrows (IDM's drop-down offered 1 to 32; issue #12).
        winui::spin_box(
            ui,
            settings_form_rect(o, 268.0, 112.0, 101.0, 21.0),
            &mut self.conns,
            1,
            u64::from(self.s.connection_limit()),
            true,
            "opt-conns",
        );
        let locked = sdl_engine::download::MAX_CONNECTIONS;
        let line = crate::i18n::fmt(
            // i18n: checkbox under the number of connections per download; {n} is 32
            crate::i18n::t("Allow more than {n} connections"),
            &[("n", &crate::i18n::integer(locked as u64))],
        );
        let was = self.s.many_connections;
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 33.0, 142.0),
            &mut self.s.many_connections,
            &line,
            true,
        );
        // Locked again: what went beyond comes back to the ceiling.
        if was && !self.s.many_connections {
            if self.conns.parse::<u32>().is_ok_and(|n| n > locked) {
                self.conns = locked.to_string();
            }
            for (_, n) in &mut self.s.conn_exceptions {
                *n = (*n).min(locked);
            }
        }
        settings_etched_h(&p, settings_form_x(o.x, 30.0), o.y + 163.0, 359.0);
        // i18n: plural noun: label before a list of exceptions
        text(
            &p,
            settings_form_x(o.x, 33.0),
            o.y + 180.0,
            crate::i18n::t("Exceptions:"),
            LIGHT_TEXT(),
        );
        // The user's exceptions, then the caps the engine learned (read-only "(auto)" rows).
        let rows: Vec<Vec<String>> = self
            .s
            .conn_exceptions
            .iter()
            .map(|(server, n)| vec![server.clone(), n.to_string()])
            .chain(
                self.learned
                    .iter()
                    .filter(|(h, _)| {
                        !self
                            .s
                            .conn_exceptions
                            .iter()
                            .any(|(s, _)| s.eq_ignore_ascii_case(h))
                    })
                    .map(|(h, n)| {
                        // i18n: a number of connections the app learned by itself for a server
                        let auto = crate::i18n::t("{n} (auto)");
                        vec![
                            h.clone(),
                            crate::i18n::fmt(auto, &[("n", &crate::i18n::integer(u64::from(*n)))]),
                        ]
                    }),
            )
            .collect();
        // New, Delete and Edit: one width, as wide as the longest label in this language (#192),
        // the list ending 8 px before them (its Number column ends at 291).
        let (col_x, col_w) = winui::buttons_to_right(
            &p,
            392.0,
            0.0,
            &[(winui::widest_label(&p, &["New", "Delete", "Edit"]), 73.0)],
            92.0,
        )[0];
        let (dbl, del_key) = settings_report(
            ui,
            o,
            settings_form_rect(o, 30.0, 184.0, col_x - 8.0 - 30.0, 100.0),
            // i18n: column titles: the server, and the number of connections allowed to it
            &[
                (crate::i18n::mark("Server"), 241.0),
                (crate::i18n::mark("Number"), 291.0),
            ],
            &rows,
            &mut self.exc_rows,
            true,
            "opt-exceptions",
        );
        // Learned rows can't be edited or deleted (they fade by themselves).
        let sel = self
            .exc_rows
            .selected
            .filter(|i| *i < self.s.conn_exceptions.len());
        let new = winui::button_light(
            ui,
            settings_form_rect(o, col_x, 195.0, col_w, 21.0),
            // i18n: button: create a new item
            "New",
            ButtonKind::Normal,
            false,
        )
        .clicked();
        let kind = if sel.is_some() {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        // i18n: verb: delete the item
        let del = winui::button_light(
            ui,
            settings_form_rect(o, col_x, 224.0, col_w, 21.0),
            "Delete",
            kind,
            false,
        )
        .clicked()
            || del_key;
        let edit =
            // i18n: verb: edit the item
            winui::button_light(ui, settings_form_rect(o, col_x, 254.0, col_w, 21.0), "Edit", kind, false)
                .clicked();
        if new {
            self.sub = Some(Sub::Exception(None, String::new(), "1".into()));
        }
        if let Some(i) = sel {
            if del {
                self.s.conn_exceptions.remove(i);
                self.exc_rows.selected = None;
            } else if edit || dbl {
                let (server, n) = self.s.conn_exceptions[i].clone();
                self.sub = Some(Sub::Exception(Some(i), server, n.to_string()));
            }
        }
        // Download limits (shared with the Scheduler's page).
        let gp = settings_form_rect(o, 21.0, 315.0, 380.0, 82.0);
        let gap = winui::caption_gap(
            &p,
            settings_form_x(o.x, 37.0),
            settings_form_x(o.x, 132.0),
            "Download limits",
            true,
        );
        winui::group_box(&p, gp, Some(gap));
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 37.0, 310.0),
            &mut self.s.limits_on,
            "Download limits",
            true,
        );
        let on = self.s.limits_on;
        let line =
            |lead: f32, lead_baseline: f32, y: f32, gap: f32, baseline: f32| winui::SentenceLine {
                lead: winui::Lead::Right(settings_form_x(o.x, lead)),
                lead_baseline: o.y + lead_baseline,
                boxes: settings_form_pos(o, 178.0, y),
                gap,
                baseline: o.y + baseline,
            };
        let boxes = winui::sentence(
            ui,
            &p,
            crate::i18n::t(crate::quota::LIMITS_SENTENCE),
            // Each line's words on its box's text baseline (the 20 px boxes at 330 and 351:
            // winui::baseline_in gives 343 and 364; they sat 2–3 px below the numbers).
            &[
                line(170.0, 343.0, 330.0, 8.0, 343.0),
                line(170.0, 364.0, 351.0, 11.0, 364.0),
            ],
            &[("mb", vec2(36.0, 20.0)), ("hours", vec2(36.0, 20.0))],
            |p, x, baseline, s| {
                winui::label(p, x, baseline, s, on);
            },
        );
        winui::number_field(ui, boxes[0], &mut self.mb, 1, 999_999, on, LIGHT_PAGE());
        winui::number_field(ui, boxes[1], &mut self.hours, 1, 999, on, LIGHT_PAGE());
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 37.0, 376.0),
            &mut self.s.limits_warn,
            "Show warning before stopping downloads",
            on,
        );
        // DNS (#36): who answers the names; the details in Settings....
        // i18n: label before the choice of DNS (the service that turns names into addresses)
        text(
            &p,
            settings_form_x(o.x, 21.0),
            o.y + 421.0,
            crate::i18n::t("DNS:"),
            LIGHT_TEXT(),
        );
        let names = dns_names();
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        if let Some(i) = winui::combo_light(
            ui,
            settings_form_rect(o, 70.0, 406.0, 211.0, 21.0),
            &labels,
            dns_index(&self.s),
            true,
            "opt-dns",
        ) {
            set_dns_index(&mut self.s, i);
            if i > 0 {
                self.sub = Some(Sub::Dns(DnsEdit::from(&self.s)));
            }
        }
        let kind = if self.s.dns_mode == 0 {
            ButtonKind::Disabled
        } else {
            ButtonKind::Normal
        };
        // i18n: button: opens the DNS's details (how it's asked, its servers, a test)
        if winui::button_light(
            ui,
            settings_form_rect(o, 289.0, 406.0, 103.0, 21.0),
            "Settings...",
            kind,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Dns(DnsEdit::from(&self.s)));
        }
    }

    /// Win and Fluent share a dedicated, full-width Connection layout while retaining
    /// the existing editable draft, learned read-only rows, quota and DNS selection flow.
    fn connection_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageConnection,
            "Connections and Limits",
            true,
            245.0,
            63.0,
            399.0,
        );

        settings_card(ui, o, 108.0, 142.0);
        modern_section_title(ui, o, 137.0, "Max. connections number");
        modern_field_label(ui, o, 170.0, "Default max. conn. number");
        winui::spin_box(
            ui,
            r(o, 32.0, 181.0, 156.0, 35.0),
            &mut self.conns,
            1,
            u64::from(self.s.connection_limit()),
            true,
            "opt-conns",
        );
        let locked = sdl_engine::download::MAX_CONNECTIONS;
        let line = crate::i18n::fmt(
            crate::i18n::t("Allow more than {n} connections"),
            &[("n", &crate::i18n::integer(locked as u64))],
        );
        let was = self.s.many_connections;
        winui::checkbox_light(
            ui,
            o + vec2(216.0, 190.0),
            &mut self.s.many_connections,
            &line,
            true,
        );
        if was && !self.s.many_connections {
            if self.conns.parse::<u32>().is_ok_and(|n| n > locked) {
                self.conns = locked.to_string();
            }
            for (_, n) in &mut self.s.conn_exceptions {
                *n = (*n).min(locked);
            }
        }

        settings_card(ui, o, 265.0, 244.0);
        modern_section_title(ui, o, 294.0, "Exceptions:");
        let rows: Vec<Vec<String>> = self
            .s
            .conn_exceptions
            .iter()
            .map(|(server, n)| vec![server.clone(), n.to_string()])
            .chain(
                self.learned
                    .iter()
                    .filter(|(h, _)| {
                        !self
                            .s
                            .conn_exceptions
                            .iter()
                            .any(|(s, _)| s.eq_ignore_ascii_case(h))
                    })
                    .map(|(h, n)| {
                        let auto = crate::i18n::t("{n} (auto)");
                        vec![
                            h.clone(),
                            crate::i18n::fmt(auto, &[("n", &crate::i18n::integer(u64::from(*n)))]),
                        ]
                    }),
            )
            .collect();
        let (dbl, del_key) = report(
            ui,
            o,
            r(o, 32.0, 309.0, 506.0, 173.0),
            &[
                (crate::i18n::mark("Server"), 410.0),
                (crate::i18n::mark("Number"), 538.0),
            ],
            &rows,
            &mut self.exc_rows,
            true,
            "opt-exceptions",
        );
        let sel = self
            .exc_rows
            .selected
            .filter(|i| *i < self.s.conn_exceptions.len());
        let new = winui::button_light(
            ui,
            r(o, 552.0, 309.0, 108.0, 36.0),
            "New",
            ButtonKind::Normal,
            false,
        )
        .clicked();
        let kind = if sel.is_some() {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        let del = winui::button_light(ui, r(o, 552.0, 358.0, 108.0, 36.0), "Delete", kind, false)
            .clicked()
            || del_key;
        let edit =
            winui::button_light(ui, r(o, 552.0, 407.0, 108.0, 36.0), "Edit", kind, false).clicked();
        if new {
            self.sub = Some(Sub::Exception(None, String::new(), "1".into()));
        }
        if let Some(i) = sel {
            if del {
                self.s.conn_exceptions.remove(i);
                self.exc_rows.selected = None;
            } else if edit || dbl {
                let (server, n) = self.s.conn_exceptions[i].clone();
                self.sub = Some(Sub::Exception(Some(i), server, n.to_string()));
            }
        }

        settings_card(ui, o, 523.0, 164.0);
        winui::checkbox_light(
            ui,
            o + vec2(32.0, 548.0),
            &mut self.s.limits_on,
            "Download limits",
            true,
        );
        let on = self.s.limits_on;
        let limits_lines = [
            winui::SentenceLine {
                lead: winui::Lead::Left(o.x + 42.0),
                lead_baseline: o.y + 594.0,
                boxes: o + vec2(295.0, 578.0),
                gap: 10.0,
                baseline: o.y + 594.0,
            },
            winui::SentenceLine {
                lead: winui::Lead::Left(o.x + 42.0),
                lead_baseline: o.y + 631.0,
                boxes: o + vec2(295.0, 615.0),
                gap: 10.0,
                baseline: o.y + 631.0,
            },
        ];
        let boxes = winui::sentence(
            ui,
            &p,
            crate::i18n::t(crate::quota::LIMITS_SENTENCE),
            &limits_lines,
            &[("mb", vec2(108.0, 30.0)), ("hours", vec2(108.0, 30.0))],
            |p, x, baseline, s| {
                winui::label(p, x, baseline, s, on);
            },
        );
        winui::number_field(ui, boxes[0], &mut self.mb, 1, 999_999, on, LIGHT_PAGE());
        winui::number_field(ui, boxes[1], &mut self.hours, 1, 999, on, LIGHT_PAGE());
        winui::checkbox_light(
            ui,
            o + vec2(32.0, 658.0),
            &mut self.s.limits_warn,
            "Show warning before stopping downloads",
            on,
        );

        settings_card(ui, o, 704.0, 124.0);
        modern_section_title(ui, o, 733.0, "DNS:");
        let names = dns_names();
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 32.0, 748.0, 506.0, 36.0),
            &labels,
            dns_index(&self.s),
            true,
            "opt-dns",
        ) {
            set_dns_index(&mut self.s, i);
            if i > 0 {
                self.sub = Some(Sub::Dns(DnsEdit::from(&self.s)));
            }
        }
        let kind = if self.s.dns_mode == 0 {
            ButtonKind::Disabled
        } else {
            ButtonKind::Normal
        };
        if winui::button_light(
            ui,
            r(o, 552.0, 748.0, 108.0, 36.0),
            "Settings...",
            kind,
            false,
        )
        .clicked()
        {
            self.sub = Some(Sub::Dns(DnsEdit::from(&self.s)));
        }
    }

    /// Win/Fluent-specific layout for Proxy / Socks. Keep all Settings fields and
    /// command handling in the existing draft; only their visible geometry changes.
    fn proxy_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        header(
            ui,
            o,
            Icon::PageProxy,
            "Proxy / socks configuration",
            true,
            112.0,
            63.0,
            399.0,
        );
        let p = ui.painter().clone();
        settings_card(ui, o, 108.0, 125.0);
        modern_section_title(ui, o, 147.0, "Browser proxy and system settings");
        let s = &mut self.s;
        if winui::button_light(
            ui,
            r(o, 512.0, 125.0, 144.0, 35.0),
            "Get System",
            ButtonKind::Normal,
            false,
        )
        .clicked()
            && let Some((host, port)) = system_proxy()
        {
            s.proxy_mode = 3;
            s.proxy_host = host;
            s.proxy_port = port;
            s.proxy_http = true;
            s.proxy_https = true;
        }
        winui::checkbox_light(
            ui,
            pos2(o.x + 32.0, o.y + 170.0),
            &mut s.proxy_from_browser,
            "",
            true,
        );
        let browser_note = crate::i18n::t(
            "Use proxy/socks from a browser's request if there are download errors /\nfor the downloads intercepted from the browser",
        );
        let lines = winui::paragraph(
            &p,
            o.x + 59.0,
            o.y + 175.0,
            browser_note,
            LIGHT_TEXT(),
            o.x + 653.0,
            16.0,
        );
        winui::caption_click(
            ui,
            r(o, 59.0, 165.0, 594.0, 18.0 * lines as f32 + 4.0),
            "proxy-from-browser",
            &mut s.proxy_from_browser,
            true,
        );

        settings_card(ui, o, 250.0, 195.0);
        modern_section_title(ui, o, 288.0, "Proxy mode");
        let modes = [
            (303.0, crate::i18n::mark("No proxy/socks")),
            (338.0, crate::i18n::mark("Use system settings")),
            (
                373.0,
                crate::i18n::mark("Use automatic configuration script"),
            ),
            (408.0, crate::i18n::mark("Manual proxy/socks configuration")),
        ];
        let mut group = Vec::new();
        for (i, (y, label)) in modes.iter().enumerate() {
            let resp = winui::radio_light(
                ui,
                pos2(o.x + 32.0, o.y + *y),
                s.proxy_mode as usize == i,
                label,
                true,
            );
            if resp.clicked() {
                s.proxy_mode = i as u8;
            }
            group.push(resp);
        }
        if let Some(i) = winui::radio_keys(ui, &group) {
            s.proxy_mode = i as u8;
        }

        settings_card(ui, o, 460.0, 123.0);
        modern_section_title(ui, o, 498.0, "Automatic configuration script");
        modern_field_label(ui, o, 526.0, "Address");
        winui::edit_light_on(
            ui,
            r(o, 32.0, 540.0, 624.0, 35.0),
            &mut s.proxy_script,
            s.proxy_mode == 2,
            false,
            LIGHT_PAGE(),
        );

        settings_card(ui, o, 598.0, 274.0);
        modern_section_title(ui, o, 637.0, "Manual proxy/socks configuration");
        let manual = s.proxy_mode == 3;
        modern_field_label(ui, o, 669.0, "Proxy server address");
        winui::text_font(
            &p,
            winui::tahoma(),
            o.x + 506.0,
            o.y + 669.0,
            crate::i18n::t("Port"),
            crate::theme::colors().muted,
        );
        // The port label is repeated at the point of input rather than above the host.
        winui::edit_light_on(
            ui,
            r(o, 32.0, 681.0, 460.0, 35.0),
            &mut s.proxy_host,
            manual,
            false,
            LIGHT_PAGE(),
        );
        winui::number_field(
            ui,
            r(o, 506.0, 681.0, 150.0, 35.0),
            &mut s.proxy_port,
            1,
            65535,
            manual,
            LIGHT_PAGE(),
        );
        modern_field_label(ui, o, 746.0, "UserName");
        let pass_label = crate::i18n::t("Password");
        winui::text_font(
            &p,
            winui::tahoma(),
            o.x + 352.0,
            o.y + 746.0,
            pass_label,
            crate::theme::colors().muted,
        );
        winui::edit_light_on(
            ui,
            r(o, 32.0, 759.0, 304.0, 35.0),
            &mut s.proxy_user,
            manual,
            false,
            LIGHT_PAGE(),
        );
        password(
            ui,
            r(o, 352.0, 759.0, 304.0, 35.0),
            &mut s.proxy_pass,
            manual,
        );
        winui::checkbox_light(
            ui,
            pos2(o.x + 32.0, o.y + 816.0),
            &mut s.proxy_http,
            "http",
            manual,
        );
        winui::checkbox_light(
            ui,
            pos2(o.x + 117.0, o.y + 816.0),
            &mut s.proxy_https,
            "https",
            manual,
        );
        winui::checkbox_light(
            ui,
            pos2(o.x + 220.0, o.y + 816.0),
            &mut s.proxy_ftp,
            "ftp",
            false,
        );
        winui::button_light(
            ui,
            r(o, 452.0, 809.0, 204.0, 35.0),
            "Advanced / Socks...",
            ButtonKind::Disabled,
            false,
        );

        settings_card(ui, o, 889.0, 72.0);
        winui::checkbox_light(
            ui,
            pos2(o.x + 32.0, o.y + 912.0),
            &mut s.ftp_passive,
            "Use FTP in PASV mode",
            true,
        );
        self.adapter_row(ui, o);
    }

    /// Win/Fluent dedicated credentials and cookies layout; the original branch
    /// remains available for all other themes.
    fn logins_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        header(
            ui,
            o,
            Icon::PageLogins,
            "User names and passwords for servers/sites",
            true,
            117.0,
            63.0,
            399.0,
        );
        settings_card(ui, o, 108.0, 350.0);
        modern_section_title(ui, o, 145.0, "Saved site logins");
        let rows: Vec<Vec<String>> = self
            .s
            .site_logins
            .iter()
            .map(|(site, user, pass)| {
                vec![site.clone(), user.clone(), "*".repeat(pass.chars().count())]
            })
            .collect();
        let (dbl, del_key) = report(
            ui,
            o,
            r(o, 32.0, 162.0, 624.0, 219.0),
            &[
                (crate::i18n::mark("Site/path"), 354.0),
                (crate::i18n::mark("User"), 511.0),
                (crate::i18n::mark("Password"), 656.0),
            ],
            &rows,
            &mut self.login_rows,
            false,
            "opt-logins",
        );
        let sel = self.login_rows.selected.filter(|i| *i < rows.len());
        let new = winui::button_light(
            ui,
            r(o, 32.0, 399.0, 108.0, 35.0),
            "New",
            ButtonKind::Normal,
            false,
        )
        .clicked();
        let kind = if sel.is_some() {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        let edit =
            winui::button_light(ui, r(o, 150.0, 399.0, 108.0, 35.0), "Edit", kind, false).clicked();
        let remove =
            winui::button_light(ui, r(o, 268.0, 399.0, 108.0, 35.0), "Remove", kind, false)
                .clicked()
                || del_key;
        if new {
            self.sub = Some(Sub::Login(
                None,
                String::new(),
                String::new(),
                String::new(),
            ));
        }
        if let Some(i) = sel {
            if remove {
                self.s.site_logins.remove(i);
                self.login_rows.selected = None;
            } else if edit || dbl {
                let (site, user, pass) = self.s.site_logins[i].clone();
                self.sub = Some(Sub::Login(Some(i), site, user, pass));
            }
        }
        self.video_cookies_modern(ui, o);
    }

    fn video_cookies_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let p = ui.painter().clone();
        settings_card(ui, o, 477.0, 362.0);
        modern_section_title(ui, o, 515.0, "Cookies for video sites");
        let browsers = sdl_tools::proc::cookie_browsers();
        let at = self.cookie_browser.min(browsers.len() - 1);
        let on = !self.s.video_cookies.is_empty();
        let tip = crate::i18n::t(
            "yt-dlp signs in to video sites with this browser's cookies, read from its files on this computer only. Used when the browser's extension didn't pass the page's own: a pasted address, a playlist.",
        );
        let off = winui::radio_light(ui, pos2(o.x + 32.0, o.y + 539.0), !on, "Off", true)
            .on_hover_text(tip);
        let from = winui::radio_light(ui, pos2(o.x + 32.0, o.y + 591.0), on, "From browser:", true)
            .on_hover_text(tip);
        let choose = |s: &mut Settings, i: usize| s.video_cookies = browsers[i].to_owned();
        match winui::radio_keys(ui, &[off.clone(), from.clone()]) {
            Some(0) => self.s.video_cookies.clear(),
            Some(_) => choose(&mut self.s, at),
            None if off.clicked() => self.s.video_cookies.clear(),
            None if from.clicked() => choose(&mut self.s, at),
            None => {}
        }
        let on = !self.s.video_cookies.is_empty();
        let names: Vec<&str> = browsers
            .iter()
            .map(|b| crate::videodlg::browser_label(b))
            .collect();
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 252.0, 582.0, 404.0, 35.0),
            &names,
            at,
            on,
            "opt-cookie-browser",
        ) {
            self.cookie_browser = i;
            choose(&mut self.s, i);
        }
        modern_field_label(ui, o, 663.0, "Profile (optional):");
        winui::edit_light_on(
            ui, r(o, 32.0, 677.0, 565.0, 35.0), &mut self.s.video_cookies_profile,
            on, false, LIGHT_PAGE(),
        ).on_hover_text(crate::i18n::t(
            "A profile's name or its folder. Empty: the profile used last. For a browser built on Chromium that isn't listed (Helium, Thorium...), choose Chromium and its profile's folder.",
        ));
        let pick_title = crate::i18n::t("Choose the profile's folder");
        let kind = if on {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        if winui::button_light(ui, r(o, 606.0, 677.0, 50.0, 35.0), "...", kind, false)
            .on_hover_text(pick_title)
            .clicked()
        {
            let title = pick_title.to_owned();
            winui::start_pick(ui.ctx(), "options-cookie-profile", move || {
                rfd::FileDialog::new().set_title(title).pick_folder()
            });
        }
        if let Some(dir) = winui::picked(ui.ctx(), "options-cookie-profile") {
            self.s.video_cookies_profile = dir.display().to_string();
        }
        let chromium = on && !matches!(self.s.video_cookies.as_str(), "firefox" | "safari");
        let (note, ink) = if cfg!(windows) && chromium {
            (
                crate::i18n::t(
                    "On Windows, Chrome, Edge and Brave may lock their cookies. Firefox works.",
                ),
                winui::ERROR(),
            )
        } else {
            (
                crate::i18n::t("Read on this computer only, and sent only to the video's site."),
                LIGHT_DISABLED(),
            )
        };
        winui::paragraph(&p, o.x + 32.0, o.y + 773.0, note, ink, o.x + 653.0, 16.0);
    }

    /// Native-sized Win/Fluent Remote settings layout; the draft and external
    /// remote controller keep their original ownership and event semantics.
    fn remote_page_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageRemote,
            "Remote control",
            true,
            180.0,
            63.0,
            399.0,
        );
        settings_card(ui, o, 108.0, 196.0);
        modern_section_title(ui, o, 144.0, "Remote connection");
        let adapters = self.adapters_now();
        winui::checkbox_light(
            ui,
            pos2(o.x + 32.0, o.y + 157.0),
            &mut self.s.remote_on,
            "Let phones and computers on the local network control SDL",
            true,
        );
        let on = self.s.remote_on;
        modern_field_label(ui, o, 219.0, "Listen on:");
        let chosen = self.s.remote_listen.trim().to_owned();
        let mut values = vec![String::new(), sdl_remote::addrs::THIS_COMPUTER.to_owned()];
        let mut labels = vec![
            crate::i18n::t("Every local network address").to_owned(),
            crate::i18n::t("This computer only (127.0.0.1)").to_owned(),
        ];
        for a in &adapters {
            for ip in
                a.v4.iter()
                    .map(|v| v.to_string())
                    .chain(a.v6.iter().map(|v| v.to_string()))
            {
                labels.push(format!("{} \u{2014} {ip}", a.name));
                values.push(ip);
            }
        }
        if !values.contains(&chosen) {
            labels.push(crate::i18n::fmt(
                crate::i18n::t("{adapter} (not connected)"),
                &[("adapter", &chosen)],
            ));
            values.push(chosen.clone());
        }
        let at = values.iter().position(|v| *v == chosen).unwrap_or(0);
        let shown: Vec<&str> = labels.iter().map(String::as_str).collect();
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 32.0, 233.0, 488.0, 35.0),
            &shown,
            at,
            on,
            "opt-remote-at",
        ) {
            self.s.remote_listen = values[i].clone();
        }
        winui::text_font(
            &p,
            winui::tahoma(),
            o.x + 537.0,
            o.y + 219.0,
            crate::i18n::t("Port:"),
            crate::theme::colors().muted,
        );
        winui::spin_box(
            ui,
            r(o, 537.0, 233.0, 119.0, 35.0),
            &mut self.port,
            1024,
            65535,
            on,
            "opt-remote-port",
        );

        settings_card(ui, o, 320.0, 177.0);
        modern_section_title(ui, o, 358.0, "Status");
        let running = self.remote.as_ref().map(|r| r.status());
        let urls = self.remote.as_ref().map(|r| r.urls()).unwrap_or_default();
        let changed = self.remote_config() != crate::remote::config(&self.from);
        use sdl_remote::Status;
        let (line, ok) = match &running {
            _ if changed => (
                crate::i18n::t("The changes apply when you click OK.").to_owned(),
                true,
            ),
            None | Some(Status::Off) => (
                crate::i18n::t("Off: nothing on the network can reach SDL.").to_owned(),
                true,
            ),
            Some(Status::Listening(_)) => (
                crate::i18n::t("On. On the phone, open this address:").to_owned(),
                true,
            ),
            Some(Status::NoAddress) => (
                crate::i18n::t("No local network now: it starts when the computer joins one.")
                    .to_owned(),
                false,
            ),
            Some(Status::PortInUse(port)) => (
                crate::i18n::fmt(
                    crate::i18n::t("Port {port} is used by another program: choose another."),
                    &[("port", &port.to_string())],
                ),
                false,
            ),
            Some(Status::Failed(e)) => (
                crate::i18n::fmt(
                    crate::i18n::t("It could not start: {error}"),
                    &[("error", e)],
                ),
                false,
            ),
        };
        let ink = if ok {
            LIGHT_TEXT()
        } else {
            crate::theme::colors().error_strong
        };
        winui::paragraph(&p, o.x + 32.0, o.y + 392.0, &line, ink, o.x + 651.0, 16.0);
        if !changed && let Some(first) = urls.first() {
            readonly_ltr(ui, r(o, 32.0, 420.0, 624.0, 35.0), first);
        }
        if let Some(fp) = self.remote.as_ref().and_then(|r| r.fingerprint()) {
            let label = crate::i18n::fmt(
                crate::i18n::t("Certificate (SHA-256): {fingerprint}"),
                &[("fingerprint", &crate::i18n::ltr(&fp))],
            );
            let (rect, cut) = winui::label_into(
                &p,
                o.x + 32.0,
                o.y + 482.0,
                &label,
                LIGHT_TEXT(),
                o.x + 654.0,
                o.x + 32.0,
            );
            if cut {
                winui::full_on_hover(ui, rect, "opt-remote-cert", &label);
            }
        }

        settings_card(ui, o, 513.0, 301.0);
        modern_section_title(ui, o, 549.0, "Paired devices:");
        let devices = self
            .remote
            .as_ref()
            .map(|r| r.devices())
            .unwrap_or_default();
        let when = |secs: i64| {
            chrono::DateTime::from_timestamp(secs, 0)
                .map(|t| {
                    crate::regional::short_date_time(t.with_timezone(&chrono::Local).naive_local())
                })
                .unwrap_or_default()
        };
        let rows: Vec<Vec<String>> = devices
            .iter()
            .map(|d| vec![d.name.clone(), when(d.added), when(d.seen)])
            .collect();
        let (_, del_key) = report(
            ui,
            o,
            r(o, 32.0, 572.0, 624.0, 161.0),
            &[
                (crate::i18n::mark("Device"), 251.0),
                (crate::i18n::mark("Paired"), 447.0),
                (crate::i18n::mark("Last seen"), 656.0),
            ],
            &rows,
            &mut self.device_rows,
            false,
            "opt-remote-devices",
        );
        let sel = self.device_rows.selected.filter(|i| *i < rows.len());
        let pair_kind = if on {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        let pair = winui::why_disabled(
            winui::button_light(
                ui,
                r(o, 32.0, 753.0, 192.0, 35.0),
                "Pair a device...",
                pair_kind,
                false,
            ),
            !on,
            "Turn on the remote control first.",
        )
        .clicked();
        let remove_kind = if sel.is_some() {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        let remove = winui::button_light(
            ui,
            r(o, 237.0, 753.0, 132.0, 35.0),
            "Remove",
            remove_kind,
            false,
        )
        .clicked()
            || del_key;
        if pair && on {
            self.pair();
        }
        if remove
            && let (Some(i), Some(r)) = (sel, &self.remote)
            && let Some(d) = devices.get(i)
        {
            r.remove(&d.id);
            self.device_rows.selected = None;
        }
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 852.0,
            crate::i18n::t(
                "Only paired devices see or change anything. The first time, the phone's browser warns about SDL's own certificate: its fingerprint is the one above.",
            ),
            LIGHT_TEXT(),
            o.x + 653.0,
            16.0,
        );
    }

    fn pair_window_modern(ctx: &egui::Context, remote: sdl_remote::Remote) -> bool {
        let out = window(
            ctx,
            "opt-pair",
            "Pair a device",
            [560.0, 608.0],
            false,
            LIGHT_BG(),
            |ui, o| {
                let _font = winui::ModernDialogFont::enter();
                let p = ui.painter().clone();
                settings_card(ui, o, 18.0, 521.0);
                let urls = remote.urls();
                let (shown, paired) = remote.pairing();
                let close_label = if paired.is_some() { "Done" } else { "Close" };
                let close = winui::button_light(
                    ui,
                    r(o, 432.0, 558.0, 96.0, 36.0),
                    close_label,
                    ButtonKind::Default,
                    false,
                )
                .clicked();
                let Some(url) = urls.first() else {
                    winui::paragraph(
                        &p,
                        o.x + 32.0,
                        o.y + 62.0,
                        crate::i18n::t(
                            "SDL can't be reached on the network now: Options → Remote says why.",
                        ),
                        crate::theme::colors().error_strong,
                        o.x + 520.0,
                        18.0,
                    );
                    return if close || winui::escape_pressed(ui) {
                        Outcome::Closed
                    } else {
                        Outcome::Open
                    };
                };
                if let Some(name) = &paired {
                    let line = crate::i18n::fmt(
                        crate::i18n::t("{device} is paired. It can control SDL now."),
                        &[("device", &crate::i18n::isolate(name))],
                    );
                    winui::paragraph(
                        &p,
                        o.x + 32.0,
                        o.y + 64.0,
                        &line,
                        LIGHT_TEXT(),
                        o.x + 520.0,
                        18.0,
                    );
                    return if close || winui::escape_pressed(ui) || winui::enter_pressed(ui) {
                        Outcome::Done(())
                    } else {
                        Outcome::Open
                    };
                }
                winui::paragraph(
                    &p,
                    o.x + 32.0,
                    o.y + 54.0,
                    crate::i18n::t(
                        "On the phone, scan this code with the camera, or open the address and enter the code.",
                    ),
                    LIGHT_TEXT(),
                    o.x + 520.0,
                    18.0,
                );
                let link = match &shown {
                    Some((code, _)) => format!("{url}#pair={code}"),
                    None => url.clone(),
                };
                crate::remote::paint_qr(&p, r(o, 166.0, 108.0, 228.0, 228.0), &link);
                readonly_ltr(ui, r(o, 32.0, 364.0, 496.0, 35.0), url);
                match &shown {
                    Some((code, left)) => {
                        winui::text_font(
                            &p,
                            egui::FontId::new(29.0, winui::tahoma_bold().family),
                            o.x + 182.0,
                            o.y + 457.0,
                            &crate::i18n::ltr(&format!("{} {}", &code[..4], &code[4..])),
                            LIGHT_TEXT(),
                        );
                        let secs = left.as_secs();
                        let line = crate::i18n::fmt(
                            crate::i18n::t("Waiting for the device. The code works for {time}."),
                            &[("time", &format!("{}:{:02}", secs / 60, secs % 60))],
                        );
                        winui::paragraph(
                            &p,
                            o.x + 32.0,
                            o.y + 503.0,
                            &line,
                            LIGHT_TEXT(),
                            o.x + 521.0,
                            16.0,
                        );
                    }
                    None => {
                        winui::paragraph(
                            &p,
                            o.x + 32.0,
                            o.y + 443.0,
                            crate::i18n::t("This code can't be used any more."),
                            crate::theme::colors().error_strong,
                            o.x + 519.0,
                            16.0,
                        );
                        if winui::button_light(
                            ui,
                            r(o, 32.0, 481.0, 132.0, 36.0),
                            "New code",
                            ButtonKind::Normal,
                            false,
                        )
                        .clicked()
                        {
                            remote.start_pairing();
                        }
                    }
                }
                if close || winui::escape_pressed(ui) {
                    Outcome::Closed
                } else {
                    Outcome::Open
                }
            },
        );
        matches!(out, Outcome::Open)
    }

    fn proxy(&mut self, ui: &mut Ui, o: Pos2) {
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            self.proxy_modern(ui, o);
            return;
        }
        let p = ui.painter().clone();
        let modern = crate::theme::modern_layout();
        if modern {
            header(
                ui,
                o,
                Icon::PageProxy,
                "Proxy / socks configuration",
                true,
                112.0,
                63.0,
                399.0,
            );
        } else {
            Icon::PageProxy
                .image(true)
                .paint_at(ui, settings_form_rect(o, 21.0, 50.0, 32.0, 32.0));
        }
        // Get System as wide as its label in this language (#192), its right end kept. A
        // translated title ends where the English one does, unless the button needs that room or
        // it would start less than 8 px after the icon: then it ends 8 px before the button, in
        // two lines where one doesn't fit there.
        let font = winui::tahoma_bold();
        let english_end =
            112.0 + winui::text_width_font(&p, font.clone(), "Proxy / socks configuration");
        // The icon ends at 53; the button never moves right of its English place.
        let (start, max_w) = (53.0 + 8.0, 160.0);
        let get_whole = winui::button_fit_width(&p, "Get System", 93.0, max_w);
        let room = (400.0 - get_whole).min(307.0) - 8.0 - start;
        let (lines, title_w) =
            page_title_lines(&p, font.clone(), "Proxy / socks configuration", room);
        let from = (start + title_w + 8.0).min(307.0);
        let (get_x, get_w) =
            winui::button_row(&p, &[("Get System", 307.0, 93.0)], 0.0, from, max_w)[0];
        if !modern
            && lines.len() == 1
            && get_x - 8.0 >= english_end
            && english_end - title_w >= start
        {
            page_title(
                &p,
                font,
                settings_form_x(o.x, 112.0),
                o.y + 69.0,
                "Proxy / socks configuration",
            );
        } else if !modern {
            page_title_ending(
                &p,
                font,
                settings_form_x(o.x, get_x) - 8.0,
                o.y + 69.0,
                &lines,
            );
        }
        let s = &mut self.s;
        if winui::button_light(
            ui,
            settings_form_rect(o, get_x, 61.0, get_w, 21.0),
            "Get System",
            ButtonKind::Normal,
            false,
        )
        .clicked()
            && let Some((host, port)) = system_proxy()
        {
            s.proxy_mode = 3;
            s.proxy_host = host;
            s.proxy_port = port;
            s.proxy_http = true;
            s.proxy_https = true;
        }
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 30.0, 100.0),
            &mut s.proxy_from_browser,
            "",
            true,
        );
        // The checkbox's caption, in two lines as IDM has it (clicking it ticks the box).
        let lines = winui::paragraph(
            &p,
            settings_form_x(o.x, 46.0),
            o.y + 104.0,
            crate::i18n::t(
                "Use proxy/socks from a browser's request if there are download errors /\nfor the downloads intercepted from the browser",
            ),
            LIGHT_TEXT(),
            settings_form_x(o.x, 426.0),
            13.0,
        );
        winui::caption_click(
            ui,
            settings_form_rect(o, 46.0, 99.0, 380.0, 13.0 * lines as f32 + 2.0),
            "proxy-from-browser",
            &mut s.proxy_from_browser,
            true,
        );
        settings_etched_h(&p, settings_form_x(o.x, 22.0), o.y + 128.0, 380.0);
        let modes = [
            (21.0, 137.0, crate::i18n::mark("No proxy/socks")),
            (21.0, 167.0, crate::i18n::mark("Use system settings")),
            (
                21.0,
                194.0,
                crate::i18n::mark("Use automatic configuration script"),
            ),
            (
                22.0,
                250.0,
                crate::i18n::mark("Manual proxy/socks configuration"),
            ),
        ];
        let mut group = Vec::new();
        for (i, (x, y, label)) in modes.iter().enumerate() {
            let resp = winui::radio_light(
                ui,
                settings_form_pos(o, *x, *y),
                s.proxy_mode as usize == i,
                label,
                true,
            );
            if resp.clicked() {
                s.proxy_mode = i as u8;
            }
            group.push(resp);
        }
        if let Some(i) = winui::radio_keys(ui, &group) {
            s.proxy_mode = i as u8;
        }
        settings_etched_h(&p, settings_form_x(o.x, 22.0), o.y + 159.0, 380.0);
        settings_etched_h(&p, settings_form_x(o.x, 21.0), o.y + 187.0, 380.0);
        settings_etched_h(&p, settings_form_x(o.x, 21.0), o.y + 242.0, 380.0);
        // The script's address (http, https or file): SDL fetches and runs it (pacfetch.rs,
        // sdl_engine::pac).
        let script = s.proxy_mode == 2;
        // i18n: noun: the web address (URL)
        disabled_label(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 228.0,
            "Address",
            script,
            false,
        );
        winui::edit_light_on(
            ui,
            settings_form_rect(o, 90.0, 213.0, 305.0, 23.0),
            &mut s.proxy_script,
            script,
            false,
            LIGHT_PAGE(),
        );
        let manual = s.proxy_mode == 3;
        disabled_label(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 280.0,
            "Proxy server address",
            manual,
            false,
        );
        disabled_label(
            &p,
            settings_form_x(o.x, 172.0),
            o.y + 278.0,
            "Port",
            manual,
            false,
        );
        disabled_label(
            &p,
            settings_form_x(o.x, 229.0),
            o.y + 278.0,
            "UserName",
            manual,
            false,
        );
        disabled_label(
            &p,
            settings_form_x(o.x, 313.0),
            o.y + 278.0,
            "Password",
            manual,
            false,
        );
        winui::edit_light_on(
            ui,
            settings_form_rect(o, 30.0, 286.0, 137.0, 24.0),
            &mut s.proxy_host,
            manual,
            false,
            LIGHT_PAGE(),
        );
        winui::number_field(
            ui,
            settings_form_rect(o, 172.0, 286.0, 51.0, 24.0),
            &mut s.proxy_port,
            1,
            65535,
            manual,
            LIGHT_PAGE(),
        );
        winui::edit_light_on(
            ui,
            settings_form_rect(o, 229.0, 286.0, 78.0, 24.0),
            &mut s.proxy_user,
            manual,
            false,
            LIGHT_PAGE(),
        );
        password(
            ui,
            settings_form_rect(o, 312.0, 286.0, 83.0, 24.0),
            &mut s.proxy_pass,
            manual,
        );
        disabled_label(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 326.0,
            "Use this proxy for the following protocols:",
            manual,
            false,
        );
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 30.0, 334.0),
            &mut s.proxy_http,
            "http",
            manual,
        );
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 97.0, 334.0),
            &mut s.proxy_https,
            "https",
            manual,
        );
        // FTP connects to its servers directly, never through an HTTP proxy: the ftp box
        // stays greyed (D-021).
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 174.0, 334.0),
            &mut s.proxy_ftp,
            "ftp",
            false,
        );
        // Socks settings: with the proxy work in M10.
        winui::button_light(
            ui,
            settings_form_rect(o, 235.0, 339.0, 159.0, 21.0),
            "Advanced / Socks...",
            ButtonKind::Disabled,
            false,
        );
        settings_etched_h(&p, settings_form_x(o.x, 21.0), o.y + 447.0, 381.0);
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 27.0, 453.0),
            &mut s.ftp_passive,
            "Use FTP in PASV mode",
            true,
        );
        self.adapter_row(ui, o);
    }

    /// Proxy / Socks → Network adapter (#35): the adapter every connection goes through (a VPN's),
    /// and what it is doing now. The list is the computer's adapters, read every 2 s; the one
    /// chosen stays in it while it's gone, "(not connected)".
    fn adapter_row(&mut self, ui: &mut Ui, o: Pos2) {
        use sdl_engine::Route;
        let stale = self
            .adapters
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed() >= std::time::Duration::from_secs(2));
        if stale {
            self.adapters = Some((std::time::Instant::now(), crate::netwatch::adapters()));
        }
        let adapters = self
            .adapters
            .as_ref()
            .map(|(_, a)| a.clone())
            .unwrap_or_default();
        let p = ui.painter().clone();
        let modern = matches!(crate::theme::current().id.as_ref(), "win" | "fluent");
        if modern {
            settings_card(ui, o, 980.0, 172.0);
            modern_section_title(ui, o, 1020.0, "Network adapter");
        } else {
            settings_etched_h(&p, settings_form_x(o.x, 21.0), o.y + 372.0, 381.0);
        }
        // i18n: label before the choice of network adapter (Ethernet, Wi-Fi, a VPN's) every connection must go through
        if modern {
            modern_field_label(ui, o, 1061.0, "Network adapter:");
        } else {
            text(
                &p,
                settings_form_x(o.x, 27.0),
                o.y + 395.0,
                crate::i18n::t("Network adapter:"),
                LIGHT_TEXT(),
            );
        }
        // The choices: any, then each adapter with its address (the ones with none too: a VPN
        // switched off), the chosen one kept when it's gone.
        let chosen = self.s.net_adapter.trim().to_owned();
        let mut values: Vec<String> = vec![String::new()];
        let mut labels: Vec<String> =
            // i18n: Options → Network adapter: the first choice: no adapter is chosen, the system picks one for each connection
            vec![crate::i18n::t("Any (the system chooses)").to_owned()];
        for a in &adapters {
            let ip =
                a.v4.first()
                    .map(|v| v.to_string())
                    .or(a.v6.first().map(|v| v.to_string()));
            values.push(a.name.clone());
            labels.push(match ip {
                Some(ip) => format!("{} \u{2014} {ip}", a.name),
                None => crate::i18n::fmt(
                    // i18n: Options → Network adapter: an adapter with no address now (a VPN switched off, a cable out); {adapter} its name
                    crate::i18n::t("{adapter} (not connected)"),
                    &[("adapter", &a.name)],
                ),
            });
        }
        if !chosen.is_empty() && !values.iter().any(|v| v.eq_ignore_ascii_case(&chosen)) {
            values.push(chosen.clone());
            labels.push(crate::i18n::fmt(
                crate::i18n::t("{adapter} (not connected)"),
                &[("adapter", &chosen)],
            ));
        }
        let at = values
            .iter()
            .position(|v| v.eq_ignore_ascii_case(&chosen))
            .unwrap_or(0);
        let shown: Vec<&str> = labels.iter().map(String::as_str).collect();
        if let Some(i) = winui::combo_light(
            ui,
            if modern {
                r(o, 32.0, 1073.0, 624.0, 35.0)
            } else {
                settings_form_rect(o, 130.0, 380.0, 265.0, 21.0)
            },
            &shown,
            at,
            true,
            "opt-adapter",
        ) {
            self.s.net_adapter = values[i].clone();
        }
        // What it does now.
        let (line, ok) = match sdl_engine::net::route_for(&self.s.net_adapter, &adapters) {
            Route::Any => (
                // i18n: Options → Network adapter: what "Any" means
                crate::i18n::t("Each connection takes the system's route.").to_owned(),
                true,
            ),
            Route::Down { label } => (
                crate::i18n::fmt(
                    // i18n: Options → Network adapter: the chosen adapter has no address now; nothing is sent until it's back; {adapter} its name
                    crate::i18n::t(
                        "{adapter} is not connected: nothing is downloaded until it is back.",
                    ),
                    &[("adapter", &label)],
                ),
                false,
            ),
            Route::Bound(b) => {
                let ip = b.v4.map(|v| v.to_string()).or(b.v6.map(|v| v.to_string()));
                let line = crate::i18n::fmt(
                    // i18n: Options → Network adapter: every connection goes out through the chosen adapter, from this address (none goes another way); {ip} its address
                    crate::i18n::t("Every connection goes through it, from {ip}."),
                    &[("ip", &crate::i18n::ltr(&ip.unwrap_or_default()))],
                );
                // Downloads, torrents (#143), video pages and galleries (through the helpers'
                // relay, #35) all go through it on every system: nothing more to say.
                (line, true)
            }
        };
        let color = if ok {
            LIGHT_TEXT()
        } else {
            crate::theme::colors().error_strong
        };
        // Up to two lines, each cut with "..." (whole on hover) when a translation is longer.
        if modern {
            winui::paragraph(
                &p,
                o.x + 32.0,
                o.y + 1134.0,
                &line,
                color,
                o.x + 650.0,
                16.0,
            );
        } else {
            for (row, part) in line.lines().take(2).enumerate() {
                let (rect, cut) = winui::label_into(
                    &p,
                    settings_form_x(o.x, 27.0),
                    o.y + 422.0 + 15.0 * row as f32,
                    part,
                    color,
                    settings_form_x(o.x, 400.0),
                    settings_form_x(o.x, 27.0),
                );
                if cut {
                    winui::full_on_hover(ui, rect, ("opt-adapter-state", row), part);
                }
            }
        }
    }

    /// Win/Fluent Accounts: keep the existing ordered debrid-account model and keyboard
    /// selection; only the table, command buttons, and explanatory sections move.
    fn accounts_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageLogins,
            "Debrid service accounts",
            true,
            117.0,
            63.0,
            399.0,
        );
        let vault = crate::secrets::vault();
        let rows: Vec<Vec<String>> = self
            .s
            .debrid_accounts
            .iter()
            .map(|a| {
                let state = if a.account().is_none() {
                    if vault.unread(&format!("debrid/{}", a.id))
                        && vault.kept() == crate::secrets::Kept::Locked
                    {
                        crate::i18n::t("in the locked keyring").to_owned()
                    } else {
                        crate::i18n::t("signed out: sign in again").to_owned()
                    }
                } else {
                    a.state.clone()
                };
                vec![a.service.name().to_owned(), a.user.clone(), state]
            })
            .collect();

        settings_card(ui, o, 108.0, 394.0);
        modern_section_title(ui, o, 144.0, "Debrid service accounts");
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 174.0,
            crate::i18n::t(
                "Accounts are checked in list order. Select one to remove or change its priority.",
            ),
            crate::theme::colors().muted,
            o.x + 654.0,
            16.0,
        );
        let (_, del_key) = settings_report(
            ui,
            o,
            r(o, 32.0, 198.0, 628.0, 228.0),
            &[
                (crate::i18n::mark("Service"), 115.0),
                (crate::i18n::mark("Account"), 250.0),
                (crate::i18n::mark("State"), 416.0),
            ],
            &rows,
            &mut self.account_rows,
            false,
            "opt-accounts",
        );
        let sel = self.account_rows.selected.filter(|i| *i < rows.len());
        let kind = |on: bool| {
            if on {
                ButtonKind::Normal
            } else {
                ButtonKind::Disabled
            }
        };
        let add = winui::button_light(
            ui,
            r(o, 32.0, 443.0, 130.0, 36.0),
            "Add...",
            ButtonKind::Accent,
            false,
        )
        .clicked();
        let remove = winui::button_light(
            ui,
            r(o, 174.0, 443.0, 130.0, 36.0),
            "Remove",
            kind(sel.is_some()),
            false,
        )
        .clicked()
            || del_key;
        let up = winui::button_light(
            ui,
            r(o, 400.0, 443.0, 120.0, 36.0),
            "Up",
            kind(sel.is_some_and(|i| i > 0)),
            false,
        )
        .clicked();
        let down = winui::button_light(
            ui,
            r(o, 532.0, 443.0, 128.0, 36.0),
            "Down",
            kind(sel.is_some_and(|i| i + 1 < rows.len())),
            false,
        )
        .clicked();
        if add {
            self.sub = Some(Sub::Account(AccountEdit::default()));
        }
        if let Some(i) = sel {
            let list = &mut self.s.debrid_accounts;
            if remove {
                list.remove(i);
                self.account_rows.selected = None;
            } else if up && i > 0 {
                list.swap(i, i - 1);
                self.account_rows.selected = Some(i - 1);
            } else if down && i + 1 < list.len() {
                list.swap(i, i + 1);
                self.account_rows.selected = Some(i + 1);
            }
        }

        settings_card(ui, o, 518.0, 150.0);
        modern_section_title(ui, o, 555.0, "Download through accounts");
        winui::checkbox_light(
            ui,
            o + vec2(32.0, 586.0),
            &mut self.s.debrid_on,
            "Download supported file-host links through these accounts",
            true,
        );
        winui::checkbox_light(
            ui,
            o + vec2(32.0, 627.0),
            &mut self.s.debrid_torrents,
            "Send torrents and magnet links to the first account instead of to peers",
            self.s.debrid_on,
        );

        settings_card(ui, o, 686.0, 255.0);
        modern_section_title(ui, o, 722.0, "Security and service limitations");
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 753.0,
            crate::i18n::t(
                "Experimental: built from the services' public API documentation, not yet confirmed with real accounts.",
            ),
            crate::theme::colors().warning,
            o.x + 650.0,
            17.0,
        );
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 815.0,
            crate::i18n::t(
                "A debrid service downloads a file host's link with your own paid account and gives SDL a direct link. Use only your own account: sharing an account or its links breaks the services' terms and can get it suspended. Downloads the browser already started stay as they are.",
            ),
            LIGHT_TEXT(),
            o.x + 650.0,
            16.0,
        );
        let kept = vault.kept();
        if kept == crate::secrets::Kept::Looking {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        if let Some(info) = kept_text(kept) {
            winui::paragraph(
                &p,
                o.x + 32.0,
                o.y + 897.0,
                crate::i18n::t(info),
                crate::theme::colors().muted,
                o.x + 650.0,
                15.0,
            );
        }
    }

    /// Win/Fluent Sounds: a dedicated event/file table with accurate row hitboxes,
    /// preserved Space/Enter/Delete handling and the native WAV file picker.
    fn sounds_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageSounds,
            "Sound settings",
            true,
            304.0,
            61.0,
            406.0,
        );
        settings_card(ui, o, 108.0, 440.0);
        modern_section_title(ui, o, 145.0, "Select sounds for Swoopdl events");
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 176.0,
            crate::i18n::t(
                "Select an event to browse for its sound or play the currently assigned file.",
            ),
            crate::theme::colors().muted,
            o.x + 649.0,
            15.0,
        );
        let list_rc = r(o, 32.0, 195.0, 628.0, 275.0);
        list_frame(&p, list_rc);
        let list = winui::rect(list_rc);
        let head_y = list.top() + 1.0;
        text(
            &p,
            list.left() + 14.0,
            head_y + 20.0,
            crate::i18n::t("Event"),
            LIGHT_TEXT(),
        );
        let split = o.x + 332.0;
        fill(
            &p,
            split,
            head_y + 1.0,
            split,
            head_y + 31.0,
            winui::HEADER_LINE(),
        );
        text(
            &p,
            split + 12.0,
            head_y + 20.0,
            crate::i18n::t("Sound file"),
            LIGHT_TEXT(),
        );
        let body = Rect::from_min_max(
            pos2(list.left() + 1.0, head_y + 33.0),
            pos2(list.right() - 1.0, list.bottom() - 1.0),
        );
        let sounds = &mut self.s.sounds;
        self.sound_rows.name = crate::i18n::t("Sounds");
        let ev = winui::row_list(
            ui,
            "sounds",
            winui::rect(body.expand(1.0)),
            SOUND_EVENTS.len(),
            45.0,
            &mut self.sound_rows,
            |ui, clip, i, row, look| {
                let top = row.top();
                fill(
                    clip,
                    row.left(),
                    top + 44.0,
                    row.right(),
                    top + 44.0,
                    winui::GRID(),
                );
                fill(clip, split, top, split, top + 44.0, winui::GRID());
                let (on, file) = &mut sounds[i];
                let event = crate::i18n::t(SOUND_EVENTS[i]);
                look.name = event.to_owned();
                winui::row_check(
                    ui,
                    clip,
                    pos2(o.x + 41.0, top + 14.0),
                    on,
                    event,
                    ("sound", i),
                );
                winui::fit(
                    ui,
                    clip,
                    o.x + 67.0,
                    top + 28.0,
                    event,
                    look.ink,
                    split - 12.0,
                    ("sound-event", i),
                );
                let name = std::path::Path::new(file.as_str())
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let drawn = winui::fit(
                    ui,
                    clip,
                    split + 12.0,
                    top + 28.0,
                    &name,
                    look.ink,
                    row.right() - 12.0,
                    ("sound-file", i),
                );
                if !file.is_empty() {
                    winui::copyable(ui, drawn, ("sound-path", i), file, true);
                }
            },
        );
        if let Some(i) = ev.space {
            sounds[i].0 = !sounds[i].0;
        }
        if ev.focused {
            self.sound_rows
                .type_to(ui, "sounds", SOUND_EVENTS.len(), |i| {
                    crate::i18n::t(SOUND_EVENTS[i]).to_owned()
                });
        }
        if let Some(i) = ev.delete {
            sounds[i] = (false, String::new());
        }
        let pick_file = ev.double_clicked.or(ev.entered);
        if pick_file.is_some() {
            winui::take_key(ui, egui::Key::Enter);
        }
        let sel = self.sound_rows.selected.filter(|i| *i < SOUND_EVENTS.len());
        let browse = winui::button_light(
            ui,
            r(o, 32.0, 488.0, 160.0, 36.0),
            "Browse...",
            if sel.is_some() {
                ButtonKind::Normal
            } else {
                ButtonKind::Disabled
            },
            false,
        )
        .clicked();
        let has_file = sel.is_some_and(|i| !self.s.sounds[i].1.is_empty());
        let play = winui::button_light(
            ui,
            r(o, 207.0, 488.0, 125.0, 36.0),
            "Play",
            if has_file {
                ButtonKind::Normal
            } else {
                ButtonKind::Disabled
            },
            false,
        )
        .clicked();
        if let Some(i) = sel {
            if browse || pick_file == Some(i) {
                let title = crate::i18n::t(SOUND_EVENTS[i]).to_owned();
                let kind = crate::i18n::t("Sounds").to_owned();
                winui::start_pick(ui.ctx(), &format!("sound-{i}"), move || {
                    rfd::FileDialog::new()
                        .set_title(title)
                        .add_filter(kind, &["wav"])
                        .pick_file()
                });
            }
            if let Some(f) = winui::picked(ui.ctx(), &format!("sound-{i}")) {
                self.s.sounds[i] = (true, f.display().to_string());
            }
            if play && has_file {
                play_sound(&self.s.sounds[i].1);
            }
        }
        settings_card(ui, o, 565.0, 109.0);
        modern_section_title(ui, o, 602.0, "Keyboard controls");
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 636.0,
            crate::i18n::t(
                "Space toggles an event; Enter or double-click selects a WAV file; Delete clears the selected sound.",
            ),
            crate::theme::colors().muted,
            o.x + 649.0,
            16.0,
        );
    }

    /// Win/Fluent Regional: independent readable controls with the same region,
    /// date order, clock and week-start identifiers as the compact implementation.
    fn regional_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageRegional,
            "Dates and times",
            true,
            302.0,
            61.0,
            406.0,
        );
        let reg = &mut self.s.regional;
        settings_card(ui, o, 108.0, 158.0);
        modern_section_title(ui, o, 144.0, "Write dates and times");
        let system = winui::radio_light(
            ui,
            o + vec2(32.0, 170.0),
            !reg.custom,
            "As the system's region writes them",
            true,
        );
        let own = winui::radio_light(ui, o + vec2(32.0, 211.0), reg.custom, "My own way:", true);
        match winui::radio_keys(ui, &[system.clone(), own.clone()]) {
            Some(i) => reg.custom = i == 1,
            None if system.clicked() => reg.custom = false,
            None if own.clicked() => reg.custom = true,
            None => {}
        }
        let on = reg.custom;
        settings_card(ui, o, 282.0, 309.0);
        modern_section_title(ui, o, 318.0, "My own way:");
        let orders = [
            crate::i18n::mark("Day, month, year"),
            crate::i18n::mark("Month, day, year"),
            crate::i18n::mark("Year, month, day"),
        ];
        let order_at = match reg.order {
            DateOrder::Dmy => 0,
            DateOrder::Mdy => 1,
            DateOrder::Ymd => 2,
        };
        modern_field_label(ui, o, 355.0, "Date order:");
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 32.0, 366.0, 628.0, 35.0),
            &orders,
            order_at,
            on,
            "regional-order",
        ) {
            reg.order = [DateOrder::Dmy, DateOrder::Mdy, DateOrder::Ymd][i];
        }
        let clocks = [
            crate::i18n::mark("12-hour (9:41 PM)"),
            crate::i18n::mark("24-hour (21:41)"),
        ];
        modern_field_label(ui, o, 431.0, "Clock:");
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 32.0, 442.0, 628.0, 35.0),
            &clocks,
            reg.clock24 as usize,
            on,
            "regional-clock",
        ) {
            reg.clock24 = i == 1;
        }
        let days = [
            crate::i18n::mark("Sunday"),
            crate::i18n::mark("Monday"),
            crate::i18n::mark("Saturday"),
        ];
        let day_at = match reg.week_start {
            1 => 1,
            6 => 2,
            _ => 0,
        };
        modern_field_label(ui, o, 507.0, "First day of the week:");
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 32.0, 518.0, 628.0, 35.0),
            &days,
            day_at,
            on,
            "regional-week",
        ) {
            reg.week_start = [0, 1, 6][i];
        }
        settings_card(ui, o, 607.0, 135.0);
        modern_section_title(ui, o, 643.0, "Example:");
        let now = chrono::Local::now().naive_local();
        let sample = crate::regional::format_with(crate::regional::effective(reg), now);
        winui::text_in_window(
            ui,
            &p,
            o.x + 32.0,
            o.y + 675.0,
            &crate::i18n::ltr(&sample),
            LIGHT_TEXT(),
        );
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 714.0,
            crate::i18n::t("Used by the list of downloads and the Scheduler."),
            crate::theme::colors().muted,
            o.x + 649.0,
            15.0,
        );
    }

    /// Win/Fluent Archives: separate extraction and Link grabber cards, with
    /// all original checkbox, destination, and installed-tool state preserved.
    fn archives_modern(&mut self, ui: &mut Ui, o: Pos2) {
        let _font = winui::ModernDialogFont::enter();
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageArchives,
            "Archives and Link grabber",
            true,
            214.0,
            61.0,
            406.0,
        );
        let s = &mut self.s;
        settings_card(ui, o, 108.0, 428.0);
        modern_section_title(ui, o, 145.0, "Archives");
        winui::checkbox_light(
            ui,
            o + vec2(32.0, 185.0),
            &mut s.extract_auto,
            "Extract archives when their download completes",
            true,
        );
        winui::checkbox_light(
            ui,
            o + vec2(32.0, 230.0),
            &mut s.extract_delete,
            if cfg!(windows) {
                "Then move the archives to the Recycle Bin"
            } else {
                "Then move the archives to the Trash"
            },
            true,
        );
        modern_field_label(ui, o, 298.0, "What an archive holds goes:");
        const INTO: [&str; 3] = [
            crate::i18n::mark("Into a folder named after it, when it holds several things"),
            crate::i18n::mark("Always into a folder named after it"),
            crate::i18n::mark("Straight into the archive's folder"),
        ];
        if let Some(i) = winui::combo_light(
            ui,
            r(o, 32.0, 312.0, 628.0, 35.0),
            &INTO,
            s.extract_subfolder.min(2) as usize,
            true,
            "opt-extract-into",
        ) {
            s.extract_subfolder = i as u8;
        }
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 386.0,
            crate::i18n::t(
                "ZIP, 7Z, RAR, TAR, GZ and XZ; a split archive (.part1.rar, .7z.001) once\nevery volume is in. A file there already is kept: the new one is numbered.",
            ),
            LIGHT_TEXT(),
            o.x + 650.0,
            16.0,
        );
        let (rar, ink) = match &self.seven_zip {
            Some(path) => (
                crate::i18n::fmt(
                    crate::i18n::t("7z and RAR archives are extracted by 7-Zip ({path})."),
                    &[("path", &crate::i18n::ltr(&path.display().to_string()))],
                ),
                LIGHT_TEXT(),
            ),
            None => (
                crate::i18n::t(
                    "7z and RAR archives need 7-Zip (7-zip.org), which isn't installed.",
                )
                .to_owned(),
                winui::ERROR(),
            ),
        };
        let line = winui::fit(
            ui,
            &p,
            o.x + 32.0,
            o.y + 485.0,
            &rar,
            ink,
            o.x + 650.0,
            "opt-rar-tool",
        );
        if winui::text_width(&p, &rar) > 618.0 {
            winui::full_on_hover(ui, line, "opt-rar-tool-full", &rar);
        }

        settings_card(ui, o, 552.0, 205.0);
        modern_section_title(ui, o, 590.0, "Link grabber");
        winui::checkbox_light(
            ui,
            o + vec2(32.0, 629.0),
            &mut s.grabber_check,
            "Look links up when they arrive (name, size, online)",
            true,
        );
        winui::checkbox_light(
            ui,
            o + vec2(32.0, 671.0),
            &mut s.grabber_hide_pages,
            "Hide web pages",
            true,
        );
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 715.0,
            crate::i18n::t(
                "Tasks → Link grabber, or the browser's \"Download all links\": a split\narchive's volumes are one package, the other links of a page another.",
            ),
            crate::theme::colors().muted,
            o.x + 650.0,
            16.0,
        );
    }

    /// Accounts (#44, ours): debrid service accounts that download file-host links. The list is
    /// asked in its order; a link goes through the first account whose service takes its host.
    fn accounts(&mut self, ui: &mut Ui, o: Pos2) {
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            self.accounts_modern(ui, o);
            return;
        }
        header(
            ui,
            o,
            Icon::PageLogins,
            "Debrid service accounts",
            true,
            117.0,
            63.0,
            399.0,
        );
        let vault = crate::secrets::vault();
        let rows: Vec<Vec<String>> = self
            .s
            .debrid_accounts
            .iter()
            .map(|a| {
                vec![
                    a.service.name().to_owned(),
                    a.user.clone(),
                    if a.account().is_none() {
                        if vault.unread(&format!("debrid/{}", a.id))
                            && vault.kept() == crate::secrets::Kept::Locked
                        {
                            // i18n: Options → Accounts: the account's sign-in is in the system's keyring (Linux), which is locked now; SDL never unlocks it
                            crate::i18n::t("in the locked keyring").to_owned()
                        } else {
                            // i18n: Options → Accounts: the account's sign-in can't be read (signed out, or the settings came from another computer)
                            crate::i18n::t("signed out: sign in again").to_owned()
                        }
                    } else {
                        a.state.clone()
                    },
                ]
            })
            .collect();
        let (_, del_key) = settings_report(
            ui,
            o,
            settings_form_rect(o, 21.0, 93.0, 383.0, 124.0),
            &[
                // i18n: column title: the debrid service (Real-Debrid…)
                (crate::i18n::mark("Service"), 110.0),
                // i18n: column title: the account's name at the service
                (crate::i18n::mark("Account"), 205.0),
                // i18n: column title: the account's state (premium, days left)
                (crate::i18n::mark("State"), 387.0),
            ],
            &rows,
            &mut self.account_rows,
            false,
            "opt-accounts",
        );
        let sel = self.account_rows.selected.filter(|i| *i < rows.len());
        // i18n: button: add a debrid service account (opens the sign-in)
        let add = winui::button_light(
            ui,
            settings_form_rect(o, 21.0, 225.0, 90.0, 21.0),
            "Add...",
            ButtonKind::Normal,
            false,
        )
        .clicked();
        let kind = |on: bool| {
            if on {
                ButtonKind::Normal
            } else {
                ButtonKind::Disabled
            }
        };
        let remove = winui::button_light(
            ui,
            settings_form_rect(o, 117.0, 225.0, 90.0, 21.0),
            "Remove",
            kind(sel.is_some()),
            false,
        )
        .clicked()
            || del_key;
        // i18n: button (short): move the account up the list (accounts are asked in the list's order)
        let up = winui::button_light(
            ui,
            settings_form_rect(o, 213.0, 225.0, 90.0, 21.0),
            "Up",
            kind(sel.is_some_and(|i| i > 0)),
            false,
        )
        .clicked();
        // i18n: button (short): move the account down the list
        let down = winui::button_light(
            ui,
            settings_form_rect(o, 309.0, 225.0, 95.0, 21.0),
            "Down",
            kind(sel.is_some_and(|i| i + 1 < rows.len())),
            false,
        )
        .clicked();
        if add {
            self.sub = Some(Sub::Account(AccountEdit::default()));
        }
        if let Some(i) = sel {
            let list = &mut self.s.debrid_accounts;
            if remove {
                list.remove(i);
                self.account_rows.selected = None;
            } else if up && i > 0 {
                list.swap(i, i - 1);
                self.account_rows.selected = Some(i - 1);
            } else if down && i + 1 < list.len() {
                list.swap(i, i + 1);
                self.account_rows.selected = Some(i + 1);
            }
        }
        // Experimental until each service is confirmed with a real account (#44).
        let p = ui.painter().clone();
        let note = crate::i18n::t(
            // i18n: Options → Accounts: a note that the debrid services' support (Real-Debrid, AllDebrid, Premiumize) is new and not yet tried with real paid accounts
            "Experimental: built from the services' public API documentation, not yet confirmed with real accounts.",
        );
        winui::paragraph(
            &p,
            settings_form_x(o.x, 21.0),
            o.y + 256.0,
            note,
            crate::theme::colors().warning,
            settings_form_x(o.x, 404.0),
            14.0,
        );
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 21.0, 302.0),
            &mut self.s.debrid_on,
            // i18n: Options → Accounts: checkbox: links of the file hosts these services support are downloaded through the accounts
            "Download supported file-host links through these accounts",
            true,
        );
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 21.0, 324.0),
            &mut self.s.debrid_torrents,
            // i18n: Options → Accounts: checkbox: magnet links and .torrent files go to the first account's service, which downloads them; SDL then downloads the files from it (no connections to peers)
            "Send torrents and magnet links to the first account instead of to peers",
            self.s.debrid_on,
        );
        let p = ui.painter().clone();
        let lines = winui::paragraph(
            &p,
            settings_form_x(o.x, 21.0),
            o.y + 358.0,
            // i18n: Options → Accounts: what these accounts are, and the services' rule against sharing
            crate::i18n::t(
                "A debrid service downloads a file host's link with your own paid account and gives SDL a direct link. Use only your own account: sharing an account or its links breaks the services' terms and can get it suspended. Downloads the browser already started stay as they are.",
            ),
            LIGHT_TEXT(),
            settings_form_x(o.x, 404.0),
            14.0,
        );
        // Where the sign-ins are kept (#44, secrets.rs); the list is two rows shorter for it.
        let kept = vault.kept();
        if kept == crate::secrets::Kept::Looking {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        if let Some(text) = kept_text(kept) {
            winui::paragraph(
                &p,
                settings_form_x(o.x, 21.0),
                o.y + 358.0 + lines as f32 * 14.0 + 6.0,
                crate::i18n::t(text),
                LIGHT_TEXT(),
                settings_form_x(o.x, 404.0),
                14.0,
            );
        }
    }

    fn logins(&mut self, ui: &mut Ui, o: Pos2) {
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            self.logins_modern(ui, o);
            return;
        }
        header(
            ui,
            o,
            Icon::PageLogins,
            "User names and passwords for servers/sites",
            true,
            117.0,
            63.0,
            399.0,
        );
        let rows: Vec<Vec<String>> = self
            .s
            .site_logins
            .iter()
            .map(|(site, user, pass)| {
                vec![site.clone(), user.clone(), "*".repeat(pass.chars().count())]
            })
            .collect();
        // The list leaves the page's foot to Cookies for video sites (#229).
        let (dbl, del_key) = settings_report(
            ui,
            o,
            settings_form_rect(o, 21.0, 93.0, 383.0, 140.0),
            &[
                // i18n: column title: the site (and path) a login is for
                (crate::i18n::mark("Site/path"), 262.0),
                // i18n: column title: the user name
                (crate::i18n::mark("User"), 332.0),
                (crate::i18n::mark("Password"), 387.0),
            ],
            &rows,
            &mut self.login_rows,
            false,
            "opt-logins",
        );
        let sel = self.login_rows.selected.filter(|i| *i < rows.len());
        // As wide as their labels in this language (#192), under the list (it ends at 404).
        let [new_at, edit_at, remove_at] = winui::button_row_from_left(
            &ui.painter().clone(),
            &[
                ("New", 31.0, 73.0),
                ("Edit", 125.0, 73.0),
                ("Remove", 220.0, 73.0),
            ],
            21.0,
            404.0,
            140.0,
        )[..] else {
            unreachable!("three buttons")
        };
        let new = winui::button_light(
            ui,
            settings_form_rect(o, new_at.0, 241.0, new_at.1, 21.0),
            // i18n: button: create a new item
            "New",
            ButtonKind::Normal,
            false,
        )
        .clicked();
        let kind = if sel.is_some() {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        let edit =
            // i18n: verb: edit the item
            winui::button_light(ui, settings_form_rect(o, edit_at.0, 241.0, edit_at.1, 21.0), "Edit", kind, false).clicked();
        // i18n: verb: remove the item
        let remove = winui::button_light(
            ui,
            settings_form_rect(o, remove_at.0, 241.0, remove_at.1, 21.0),
            "Remove",
            kind,
            false,
        )
        .clicked()
            || del_key;
        if new {
            self.sub = Some(Sub::Login(
                None,
                String::new(),
                String::new(),
                String::new(),
            ));
        }
        if let Some(i) = sel {
            if remove {
                self.s.site_logins.remove(i);
                self.login_rows.selected = None;
            } else if edit || dbl {
                let (site, user, pass) = self.s.site_logins[i].clone();
                self.sub = Some(Sub::Login(Some(i), site, user, pass));
            }
        }
        self.video_cookies(ui, o);
    }

    /// Sites Logins → Cookies for video sites (#229): Off, or the browser whose cookies yt-dlp
    /// reads on this computer (`--cookies-from-browser`) when the extension passed none, and its
    /// profile (a name, or a folder: a Chromium fork's).
    fn video_cookies(&mut self, ui: &mut Ui, o: Pos2) {
        let p = ui.painter().clone();
        // i18n: Options → Sites Logins: group box title: yt-dlp signs in to video sites with a browser's cookies
        group(ui, o, 16.0, 270.0, 392.0, 130.0, "Cookies for video sites");
        let browsers = sdl_tools::proc::cookie_browsers();
        let at = self.cookie_browser.min(browsers.len() - 1);
        let on = !self.s.video_cookies.is_empty();
        // i18n: Options → Sites Logins → Cookies for video sites: the tooltip of its choices
        let tip = crate::i18n::t(
            "yt-dlp signs in to video sites with this browser's cookies, read from its files on this computer only. Used when the browser's extension didn't pass the page's own: a pasted address, a playlist.",
        );
        // i18n: Options → Sites Logins → Cookies for video sites: don't read any browser's cookies (the default)
        let off = winui::radio_light(ui, settings_form_pos(o, 30.0, 288.0), !on, "Off", true)
            .on_hover_text(tip);
        let from = winui::radio_light_until(
            ui,
            settings_form_pos(o, 30.0, 314.0),
            on,
            // i18n: Options → Sites Logins → Cookies for video sites: read the cookies of the browser chosen in the list after it
            "From browser:",
            true,
            Some(if crate::theme::modern_layout() {
                settings_form_x(o.x, 194.0)
            } else {
                194.0
            }),
        )
        .on_hover_text(tip);
        let choose = |s: &mut Settings, i: usize| s.video_cookies = browsers[i].to_owned();
        match winui::radio_keys(ui, &[off.clone(), from.clone()]) {
            Some(0) => self.s.video_cookies.clear(),
            Some(_) => choose(&mut self.s, at),
            None if off.clicked() => self.s.video_cookies.clear(),
            None if from.clicked() => choose(&mut self.s, at),
            None => {}
        }
        let on = !self.s.video_cookies.is_empty();
        let names: Vec<&str> = browsers
            .iter()
            .map(|b| crate::videodlg::browser_label(b))
            .collect();
        if let Some(i) = winui::combo_light(
            ui,
            settings_form_rect(o, 200.0, 311.0, 192.0, 21.0),
            &names,
            at,
            on,
            "opt-cookie-browser",
        ) {
            self.cookie_browser = i;
            choose(&mut self.s, i);
        }
        // The profile: a name (Firefox's, "Profile 1") or a folder; empty, the last one used.
        let ink = if on { LIGHT_TEXT() } else { LIGHT_DISABLED() };
        // i18n: Options → Sites Logins → Cookies for video sites: label before the browser profile's name or folder (may stay empty)
        let label = crate::i18n::t("Profile (optional):");
        let (rect, cut) = winui::text_right_within(
            &p,
            settings_form_x(o.x, 194.0),
            o.y + 354.0,
            label,
            ink,
            settings_form_x(o.x, 30.0),
        );
        if cut {
            winui::full_on_hover(ui, rect, "cookie-profile", label);
        }
        winui::edit_light_on(
            ui,
            settings_form_rect(o, 200.0, 338.0, 160.0, 23.0),
            &mut self.s.video_cookies_profile,
            on,
            false,
            LIGHT_PAGE(),
        )
        .on_hover_text(crate::i18n::t(
            // i18n: Options → Sites Logins → Cookies for video sites: the tooltip of the profile box
            "A profile's name or its folder. Empty: the profile used last. For a browser built on Chromium that isn't listed (Helium, Thorium...), choose Chromium and its profile's folder.",
        ));
        // i18n: Options → Sites Logins → Cookies for video sites: the title of the window, and the tooltip of the button, that pick a browser profile's folder
        let pick_title = crate::i18n::t("Choose the profile's folder");
        let kind = if on {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        if winui::button_light(
            ui,
            settings_form_rect(o, 365.0, 339.0, 27.0, 21.0),
            "...",
            kind,
            false,
        )
        .on_hover_text(pick_title)
        .clicked()
        {
            let title = pick_title.to_owned();
            winui::start_pick(ui.ctx(), "options-cookie-profile", move || {
                rfd::FileDialog::new().set_title(title).pick_folder()
            });
        }
        if let Some(dir) = winui::picked(ui.ctx(), "options-cookie-profile") {
            self.s.video_cookies_profile = dir.display().to_string();
        }
        // What to expect: Chrome-family browsers on Windows lock their cookies (Chrome 127 and
        // later's app-bound encryption); yt-dlp says so when it meets it, Options says so first.
        let chromium = on && !matches!(self.s.video_cookies.as_str(), "firefox" | "safari");
        let (note, ink) = if cfg!(windows) && chromium {
            (
                // i18n: Options → Sites Logins → Cookies for video sites, a Chrome-like browser chosen on Windows
                crate::i18n::t(
                    "On Windows, Chrome, Edge and Brave may lock their cookies. Firefox works.",
                ),
                winui::ERROR(),
            )
        } else {
            (
                // i18n: Options → Sites Logins → Cookies for video sites: a note under the choices
                crate::i18n::t("Read on this computer only, and sent only to the video's site."),
                LIGHT_DISABLED(),
            )
        };
        // Two lines at most: a longer language wraps rather than being cut.
        winui::paragraph(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 380.0,
            note,
            ink,
            settings_form_x(o.x, 400.0),
            13.0,
        );
    }

    /// Archives (ours, #37): extracting archives when their download completes, what becomes
    /// of them, and Link grabber's ways.
    fn archives(&mut self, ui: &mut Ui, o: Pos2) {
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            self.archives_modern(ui, o);
            return;
        }
        let p = ui.painter().clone();
        // i18n: Options page title: extracting downloaded archives, and Link grabber
        let title = crate::i18n::mark("Archives and Link grabber");
        let w = winui::text_width_font(&p, winui::tahoma_bold(), title);
        header(
            ui,
            o,
            Icon::PageArchives,
            title,
            true,
            406.0 - w,
            61.0,
            406.0,
        );
        let s = &mut self.s;
        // i18n: Options → Archives: group box title
        group(ui, o, 16.0, 92.0, 392.0, 196.0, "Archives");
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 30.0, 110.0),
            &mut s.extract_auto,
            // i18n: Options → Archives check box (a Link grabber package has its own "Extract when done")
            "Extract archives when their download completes",
            true,
        );
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 30.0, 134.0),
            &mut s.extract_delete,
            if cfg!(windows) {
                // i18n: Options → Archives check box
                "Then move the archives to the Recycle Bin"
            } else {
                // i18n: Options → Archives check box (Linux and macOS)
                "Then move the archives to the Trash"
            },
            true,
        );
        text(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 174.0,
            // i18n: Options → Archives: a label before the list of where extracted files go
            crate::i18n::t("What an archive holds goes:"),
            LIGHT_TEXT(),
        );
        const INTO: [&str; 3] = [
            // i18n: Options → Archives: where extracted files go
            crate::i18n::mark("Into a folder named after it, when it holds several things"),
            // i18n: Options → Archives: where extracted files go
            crate::i18n::mark("Always into a folder named after it"),
            // i18n: Options → Archives: where extracted files go
            crate::i18n::mark("Straight into the archive's folder"),
        ];
        if let Some(i) = winui::combo_light(
            ui,
            settings_form_rect(o, 30.0, 181.0, 364.0, 21.0),
            &INTO,
            s.extract_subfolder.min(2) as usize,
            true,
            "opt-extract-into",
        ) {
            s.extract_subfolder = i as u8;
        }
        winui::paragraph(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 226.0,
            crate::i18n::t(
                "ZIP, 7Z, RAR, TAR, GZ and XZ; a split archive (.part1.rar, .7z.001) once\nevery volume is in. A file there already is kept: the new one is numbered.",
            ),
            LIGHT_TEXT(),
            settings_form_x(o.x, 400.0),
            13.0,
        );
        let (rar, ink) = match &self.seven_zip {
            Some(path) => (
                crate::i18n::fmt(
                    // i18n: Options → Archives: {path} is where 7-Zip's program is
                    crate::i18n::t("7z and RAR archives are extracted by 7-Zip ({path})."),
                    &[("path", &crate::i18n::ltr(&path.display().to_string()))],
                ),
                LIGHT_TEXT(),
            ),
            None => (
                // i18n: Options → Archives, when 7-Zip isn't installed
                crate::i18n::t(
                    "7z and RAR archives need 7-Zip (7-zip.org), which isn't installed.",
                )
                .to_owned(),
                winui::ERROR(),
            ),
        };
        winui::fit(
            ui,
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 272.0,
            &rar,
            ink,
            settings_form_x(o.x, 400.0),
            "opt-rar-tool",
        );
        // i18n: Options → Archives: group box title
        group(ui, o, 16.0, 300.0, 392.0, 100.0, "Link grabber");
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 30.0, 318.0),
            &mut s.grabber_check,
            // i18n: Options → Archives → Link grabber check box (IDM's Download all links looks every link up)
            "Look links up when they arrive (name, size, online)",
            true,
        );
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 30.0, 342.0),
            &mut s.grabber_hide_pages,
            "Hide web pages",
            true,
        );
        winui::paragraph(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 380.0,
            crate::i18n::t(
                "Tasks → Link grabber, or the browser's \"Download all links\": a split\narchive's volumes are one package, the other links of a page another.",
            ),
            LIGHT_DISABLED(),
            settings_form_x(o.x, 400.0),
            13.0,
        );
    }

    /// Regional (ours): how dates and times are written in the list and the Scheduler, as the
    /// system's region writes them or the user's own way.
    fn regional(&mut self, ui: &mut Ui, o: Pos2) {
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            self.regional_modern(ui, o);
            return;
        }
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageRegional,
            "Dates and times",
            true,
            302.0,
            61.0,
            406.0,
        );
        let reg = &mut self.s.regional;
        group(ui, o, 16.0, 92.0, 392.0, 250.0, "Write dates and times");
        let system = winui::radio_light(
            ui,
            settings_form_pos(o, 30.0, 112.0),
            !reg.custom,
            "As the system's region writes them",
            true,
        );
        let own = winui::radio_light(
            ui,
            settings_form_pos(o, 30.0, 140.0),
            reg.custom,
            "My own way:",
            true,
        );
        match winui::radio_keys(ui, &[system.clone(), own.clone()]) {
            Some(i) => reg.custom = i == 1,
            None if system.clicked() => reg.custom = false,
            None if own.clicked() => reg.custom = true,
            None => {}
        }
        let on = reg.custom;
        // Each choice: its label right-aligned before its box.
        let row = |ui: &mut Ui, y: f32, label: &str, items: &[&str], at: usize, id: &str| {
            let label = crate::i18n::t(label);
            let color = if on { LIGHT_TEXT() } else { LIGHT_DISABLED() };
            let (rect, cut) = winui::text_right_within(
                &p,
                settings_form_x(o.x, 186.0),
                o.y + y + 15.0,
                label,
                color,
                settings_form_x(o.x, 48.0),
            );
            if cut {
                winui::full_on_hover(ui, rect, ("regional-label", y as i32), label);
            }
            winui::combo_light(
                ui,
                settings_form_rect(o, 192.0, y, 200.0, 21.0),
                items,
                at,
                on,
                id,
            )
        };
        let orders = [
            // i18n: a date's order: day first (30/10/2026)
            crate::i18n::mark("Day, month, year"),
            // i18n: a date's order: month first (10/30/2026)
            crate::i18n::mark("Month, day, year"),
            // i18n: a date's order: year first (2026-10-30)
            crate::i18n::mark("Year, month, day"),
        ];
        let order_at = match reg.order {
            DateOrder::Dmy => 0,
            DateOrder::Mdy => 1,
            DateOrder::Ymd => 2,
        };
        // i18n: label before the choice of a date's order
        if let Some(i) = row(
            ui,
            168.0,
            crate::i18n::mark("Date order:"),
            &orders,
            order_at,
            "regional-order",
        ) {
            reg.order = [DateOrder::Dmy, DateOrder::Mdy, DateOrder::Ymd][i];
        }
        let clocks = [
            // i18n: a 12-hour clock, with an example
            crate::i18n::mark("12-hour (9:41 PM)"),
            // i18n: a 24-hour clock, with an example
            crate::i18n::mark("24-hour (21:41)"),
        ];
        // i18n: label before the choice of a 12- or 24-hour clock
        if let Some(i) = row(
            ui,
            198.0,
            crate::i18n::mark("Clock:"),
            &clocks,
            reg.clock24 as usize,
            "regional-clock",
        ) {
            reg.clock24 = i == 1;
        }
        let days = [
            crate::i18n::mark("Sunday"),
            crate::i18n::mark("Monday"),
            crate::i18n::mark("Saturday"),
        ];
        let day_at = match reg.week_start {
            1 => 1,
            6 => 2,
            _ => 0,
        };
        // i18n: label before the choice of the week's first day (in the Scheduler's calendar)
        if let Some(i) = row(
            ui,
            228.0,
            crate::i18n::mark("First day of the week:"),
            &days,
            day_at,
            "regional-week",
        ) {
            reg.week_start = [0, 1, 6][i];
        }
        // What it looks like, now.
        let now = chrono::Local::now().naive_local();
        let sample = crate::regional::format_with(crate::regional::effective(reg), now);
        // i18n: label before an example date and time written the chosen way
        let label = crate::i18n::t("Example:");
        text(
            &p,
            settings_form_x(o.x, 48.0),
            o.y + 290.0,
            label,
            LIGHT_TEXT(),
        );
        let lw = winui::text_width(&p, label);
        winui::text_in_window(
            ui,
            &p,
            settings_form_x(o.x, 48.0) + lw + 8.0,
            o.y + 290.0,
            &crate::i18n::ltr(&sample),
            LIGHT_TEXT(),
        );
        // i18n: a note under the date and time choices
        winui::text_in_window(
            ui,
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 320.0,
            crate::i18n::t("Used by the list of downloads and the Scheduler."),
            LIGHT_DISABLED(),
        );
    }

    fn sounds(&mut self, ui: &mut Ui, o: Pos2) {
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            self.sounds_modern(ui, o);
            return;
        }
        let p = ui.painter().clone();
        header(
            ui,
            o,
            Icon::PageSounds,
            "Sound settings",
            true,
            304.0,
            61.0,
            406.0,
        );
        group(
            ui,
            o,
            25.0,
            102.0,
            383.0,
            299.0,
            "Select sounds for Swoopdl events",
        );
        let list_on_screen = settings_form_rect(o, 33.0, 112.0, 366.0, 250.0);
        list_frame(&p, list_on_screen);
        let list = winui::rect(list_on_screen);
        let head_y = list.top() + 1.0;
        // i18n: column title: the event a sound plays for
        text(
            &p,
            list.left() + 8.0,
            head_y + 16.0,
            crate::i18n::t("Event"),
            LIGHT_TEXT(),
        );
        fill(
            &p,
            settings_form_x(o.x, 224.0),
            head_y + 1.0,
            settings_form_x(o.x, 224.0),
            head_y + 23.0,
            winui::HEADER_LINE(),
        );
        text(
            &p,
            settings_form_x(o.x, 231.0),
            head_y + 16.0,
            crate::i18n::t("Sound file"),
            LIGHT_TEXT(),
        );
        // The events: the shared row list under the header (its frame on the list's). Space
        // ticks one, a double-click or Enter chooses its file, Delete clears it.
        let body = Rect::from_min_max(
            pos2(list.left() + 1.0, head_y + 25.0),
            pos2(list.right() - 1.0, list.bottom() - 1.0),
        );
        let grid = winui::GRID();
        let modern = crate::theme::modern_layout();
        let sound_row_h = if modern { 34.0 } else { 17.0 };
        let sounds = &mut self.s.sounds;
        self.sound_rows.name = crate::i18n::t("Sounds");
        let ev = winui::row_list(
            ui,
            "sounds",
            winui::rect(body.expand(1.0)),
            SOUND_EVENTS.len(),
            sound_row_h,
            &mut self.sound_rows,
            |ui, clip, i, row, look| {
                let top = row.top();
                fill(
                    clip,
                    row.left(),
                    top + sound_row_h - 1.0,
                    row.right(),
                    top + sound_row_h - 1.0,
                    grid,
                );
                fill(
                    clip,
                    settings_form_x(o.x, 225.0),
                    top,
                    settings_form_x(o.x, 225.0),
                    top + sound_row_h - 1.0,
                    grid,
                );
                let (on, file) = &mut sounds[i];
                let event = crate::i18n::t(SOUND_EVENTS[i]);
                look.name = event.to_owned();
                winui::row_check(
                    ui,
                    clip,
                    pos2(
                        settings_form_x(o.x, 39.0),
                        top + if modern { 9.0 } else { 2.0 },
                    ),
                    on,
                    event,
                    ("sound", i),
                );
                winui::fit(
                    ui,
                    clip,
                    settings_form_x(o.x, 57.0),
                    top + if modern { 22.0 } else { 12.0 },
                    event,
                    look.ink,
                    settings_form_x(o.x, 221.0),
                    ("sound-event", i),
                );
                let name = std::path::Path::new(file.as_str())
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let drawn = winui::fit(
                    ui,
                    clip,
                    settings_form_x(o.x, 231.0),
                    top + if modern { 22.0 } else { 12.0 },
                    &name,
                    look.ink,
                    row.right() - 4.0,
                    ("sound-file", i),
                );
                // The whole path on hover (the name alone says little).
                if !file.is_empty() {
                    winui::copyable(ui, drawn, ("sound-path", i), file, true);
                }
            },
        );
        if let Some(i) = ev.space {
            sounds[i].0 = !sounds[i].0;
        }
        if ev.focused {
            self.sound_rows
                .type_to(ui, "sounds", SOUND_EVENTS.len(), |i| {
                    crate::i18n::t(SOUND_EVENTS[i]).to_owned()
                });
        }
        if let Some(i) = ev.delete {
            sounds[i] = (false, String::new());
        }
        let pick_file = ev.double_clicked.or(ev.entered);
        if pick_file.is_some() {
            winui::take_key(ui, egui::Key::Enter);
        }
        let sel = self.sound_rows.selected;
        let kind = if sel.is_some() {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        // Both as wide as their labels in this language (#192), each around its English centre.
        let browse_at = winui::button_around(&p, "Browse...", 106.0, 73.0, 21.0, 233.0);
        let play_at = winui::button_around(
            &p,
            "Play",
            241.0,
            73.0,
            browse_at.0 + browse_at.1 + 8.0,
            404.0,
        );
        let browse = winui::button_light(
            ui,
            settings_form_rect(o, browse_at.0, 371.0, browse_at.1, 21.0),
            "Browse...",
            kind,
            false,
        )
        .clicked();
        let has_file = sel.is_some_and(|i| !self.s.sounds[i].1.is_empty());
        let play_kind = if has_file {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        let play =
            // i18n: verb: plays the sound
            winui::button_light(ui, settings_form_rect(o, play_at.0, 371.0, play_at.1, 21.0), "Play", play_kind, false)
                .clicked();
        if let Some(i) = sel {
            if browse || pick_file == Some(i) {
                let title = crate::i18n::t(SOUND_EVENTS[i]).to_owned();
                // i18n: the kind of file the Open dialog lists (sound files)
                let kind = crate::i18n::t("Sounds").to_owned();
                winui::start_pick(ui.ctx(), &format!("sound-{i}"), move || {
                    rfd::FileDialog::new()
                        .set_title(title)
                        .add_filter(kind, &["wav"])
                        .pick_file()
                });
            }
            if let Some(f) = winui::picked(ui.ctx(), &format!("sound-{i}")) {
                self.s.sounds[i] = (true, f.display().to_string());
            }
            if play && has_file {
                play_sound(&self.s.sounds[i].1);
            }
        }
    }

    // ---- entry dialogs --------------------------------------------------------------------

    /// The computer's adapters, read again every 2 s (Proxy / Socks' adapter, Remote's address).
    fn adapters_now(&mut self) -> Vec<sdl_engine::net::Adapter> {
        let stale = self
            .adapters
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed() >= std::time::Duration::from_secs(2));
        if stale {
            self.adapters = Some((std::time::Instant::now(), crate::netwatch::adapters()));
        }
        self.adapters
            .as_ref()
            .map(|(_, a)| a.clone())
            .unwrap_or_default()
    }

    /// Remote's port as typed, or the one saved when it isn't a port SDL takes.
    fn remote_port(&self) -> u16 {
        self.port
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|p| *p >= 1024)
            .unwrap_or(self.from.remote_port)
    }

    /// The remote control as this draft says (on, address, port).
    fn remote_config(&self) -> sdl_remote::Config {
        let mut s = self.s.clone();
        s.remote_port = self.remote_port();
        crate::remote::config(&s)
    }

    /// Remote → Pair a device...: the remote control starts as the page says (Cancel puts it
    /// back as it was), a new code is shown.
    fn pair(&mut self) {
        let config = self.remote_config();
        if let Some(r) = &self.remote {
            r.apply(config);
            r.start_pairing();
        }
        self.sub = Some(Sub::Pair);
    }

    /// Remote (#141, ours): phones and computers on the local network add and control
    /// downloads from a page SDL serves, once paired with a code shown here.
    fn remote_page(&mut self, ui: &mut Ui, o: Pos2) {
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            self.remote_page_modern(ui, o);
            return;
        }
        let p = ui.painter().clone();
        // i18n: Options page title: control SDL from a phone or another computer on the local network
        let title = crate::i18n::mark("Remote control");
        let w = winui::text_width_font(&p, winui::tahoma_bold(), title);
        header(ui, o, Icon::PageRemote, title, true, 406.0 - w, 61.0, 406.0);
        let adapters = self.adapters_now();
        winui::checkbox_light(
            ui,
            settings_form_pos(o, 21.0, 92.0),
            &mut self.s.remote_on,
            // i18n: Options → Remote check box (off by default)
            "Let phones and computers on the local network control SDL",
            true,
        );
        let on = self.s.remote_on;
        let color = if on { LIGHT_TEXT() } else { LIGHT_DISABLED() };
        text(
            &p,
            settings_form_x(o.x, 21.0),
            o.y + 131.0,
            // i18n: Options → Remote: label before the choice of the address SDL listens on
            crate::i18n::t("Listen on:"),
            color,
        );
        let chosen = self.s.remote_listen.trim().to_owned();
        let mut values = vec![String::new(), sdl_remote::addrs::THIS_COMPUTER.to_owned()];
        let mut labels = vec![
            // i18n: Options → Remote → Listen on: the default: each of the computer's private local network addresses (never one the internet reaches)
            crate::i18n::t("Every local network address").to_owned(),
            // i18n: Options → Remote → Listen on: only programs on this computer can connect (127.0.0.1)
            crate::i18n::t("This computer only (127.0.0.1)").to_owned(),
        ];
        for a in &adapters {
            for ip in
                a.v4.iter()
                    .map(|v| v.to_string())
                    .chain(a.v6.iter().map(|v| v.to_string()))
            {
                labels.push(format!("{} \u{2014} {ip}", a.name));
                values.push(ip);
            }
        }
        if !values.contains(&chosen) {
            labels.push(crate::i18n::fmt(
                crate::i18n::t("{adapter} (not connected)"),
                &[("adapter", &chosen)],
            ));
            values.push(chosen.clone());
        }
        let at = values.iter().position(|v| *v == chosen).unwrap_or(0);
        let shown: Vec<&str> = labels.iter().map(String::as_str).collect();
        if let Some(i) = winui::combo_light(
            ui,
            settings_form_rect(o, 130.0, 118.0, 278.0, 21.0),
            &shown,
            at,
            on,
            "opt-remote-at",
        ) {
            self.s.remote_listen = values[i].clone();
        }
        // i18n: Options → Remote: label before the port number box
        text(
            &p,
            settings_form_x(o.x, 21.0),
            o.y + 159.0,
            crate::i18n::t("Port:"),
            color,
        );
        winui::spin_box(
            ui,
            settings_form_rect(o, 130.0, 146.0, 80.0, 21.0),
            &mut self.port,
            1024,
            65535,
            on,
            "opt-remote-port",
        );
        // What it does now: as saved, or what OK changes.
        group(ui, o, 16.0, 178.0, 392.0, 96.0, "Status");
        let running = self.remote.as_ref().map(|r| r.status());
        let urls = self.remote.as_ref().map(|r| r.urls()).unwrap_or_default();
        let changed = self.remote_config() != crate::remote::config(&self.from);
        use sdl_remote::Status;
        let (line, ok) = match &running {
            _ if changed => (
                // i18n: Options → Remote → Status: the settings were changed; they apply on OK
                crate::i18n::t("The changes apply when you click OK.").to_owned(),
                true,
            ),
            None | Some(Status::Off) => (
                // i18n: Options → Remote → Status: the remote control is off
                crate::i18n::t("Off: nothing on the network can reach SDL.").to_owned(),
                true,
            ),
            Some(Status::Listening(_)) => (
                // i18n: Options → Remote → Status: on; the address follows in a box below
                crate::i18n::t("On. On the phone, open this address:").to_owned(),
                true,
            ),
            Some(Status::NoAddress) => (
                // i18n: Options → Remote → Status: on, but the computer has no local network address now
                crate::i18n::t("No local network now: it starts when the computer joins one.")
                    .to_owned(),
                false,
            ),
            Some(Status::PortInUse(port)) => (
                crate::i18n::fmt(
                    // i18n: Options → Remote → Status: another program uses the port; {port} = its number
                    crate::i18n::t("Port {port} is used by another program: choose another."),
                    &[("port", &port.to_string())],
                ),
                false,
            ),
            Some(Status::Failed(e)) => (
                crate::i18n::fmt(
                    // i18n: Options → Remote → Status: it could not start; {error} = the system's reason
                    crate::i18n::t("It could not start: {error}"),
                    &[("error", e)],
                ),
                false,
            ),
        };
        let ink = if ok {
            LIGHT_TEXT()
        } else {
            crate::theme::colors().error_strong
        };
        let (rect, cut) = winui::label_into(
            &p,
            settings_form_x(o.x, 30.0),
            o.y + 207.0,
            &line,
            ink,
            settings_form_x(o.x, 400.0),
            settings_form_x(o.x, 30.0),
        );
        if cut {
            winui::full_on_hover(ui, rect, "opt-remote-state", &line);
        }
        if !changed && let Some(first) = urls.first() {
            readonly_ltr(ui, settings_form_rect(o, 30.0, 215.0, 364.0, 21.0), first);
        }
        if let Some(fp) = self.remote.as_ref().and_then(|r| r.fingerprint()) {
            let label = crate::i18n::fmt(
                // i18n: Options → Remote → Status: the certificate's fingerprint, which the phone's browser shows when it warns about it; {fingerprint} = hex digits
                crate::i18n::t("Certificate (SHA-256): {fingerprint}"),
                &[("fingerprint", &crate::i18n::ltr(&fp))],
            );
            let (rect, cut) = winui::label_into(
                &p,
                settings_form_x(o.x, 30.0),
                o.y + 260.0,
                &label,
                LIGHT_TEXT(),
                settings_form_x(o.x, 400.0),
                settings_form_x(o.x, 30.0),
            );
            if cut {
                winui::full_on_hover(ui, rect, "opt-remote-cert", &label);
            }
        }
        // The paired devices.
        text(
            &p,
            settings_form_x(o.x, 21.0),
            o.y + 296.0,
            // i18n: Options → Remote: label above the list of paired phones and computers
            crate::i18n::t("Paired devices:"),
            LIGHT_TEXT(),
        );
        let devices = self
            .remote
            .as_ref()
            .map(|r| r.devices())
            .unwrap_or_default();
        let when = |secs: i64| {
            chrono::DateTime::from_timestamp(secs, 0)
                .map(|t| {
                    crate::regional::short_date_time(t.with_timezone(&chrono::Local).naive_local())
                })
                .unwrap_or_default()
        };
        let rows: Vec<Vec<String>> = devices
            .iter()
            .map(|d| vec![d.name.clone(), when(d.added), when(d.seen)])
            .collect();
        let (_, del_key) = settings_report(
            ui,
            o,
            settings_form_rect(o, 16.0, 302.0, 392.0, 88.0),
            &[
                // i18n: column title: a paired phone's or computer's name
                (crate::i18n::mark("Device"), 120.0),
                // i18n: column title: when the device was paired
                (crate::i18n::mark("Paired"), 256.0),
                // i18n: column title: when the device last reached SDL
                (crate::i18n::mark("Last seen"), 392.0),
            ],
            &rows,
            &mut self.device_rows,
            false,
            "opt-remote-devices",
        );
        let sel = self.device_rows.selected.filter(|i| *i < rows.len());
        let pair_kind = if on {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        let pair = winui::why_disabled(
            winui::button_light(
                ui,
                settings_form_rect(o, 16.0, 398.0, 130.0, 21.0),
                // i18n: Options → Remote button: shows a code (and a QR code) that pairs a phone
                "Pair a device...",
                pair_kind,
                false,
            ),
            !on,
            // i18n: why Options → Remote → Pair a device... is greyed
            "Turn on the remote control first.",
        )
        .clicked();
        let remove_kind = if sel.is_some() {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        };
        let remove = winui::button_light(
            ui,
            settings_form_rect(o, 154.0, 398.0, 90.0, 21.0),
            "Remove",
            remove_kind,
            false,
        )
        .clicked()
            || del_key;
        if pair && on {
            self.pair();
        }
        // Unpaired at once (not on OK): a lost phone is refused from its next request on.
        if remove
            && let (Some(i), Some(r)) = (sel, &self.remote)
            && let Some(d) = devices.get(i)
        {
            r.remove(&d.id);
            self.device_rows.selected = None;
        }
        winui::paragraph(
            &p,
            settings_form_x(o.x, 21.0),
            o.y + 438.0,
            // i18n: Options → Remote, at the bottom of the page
            crate::i18n::t(
                "Only paired devices see or change anything. The first time, the phone's browser warns about SDL's own certificate: its fingerprint is the one above.",
            ),
            LIGHT_TEXT(),
            settings_form_x(o.x, 406.0),
            13.0,
        );
    }

    /// Remote → Pair a device...: the address (and a QR code of it), the code, and the device
    /// once it paired. False when it closes.
    fn pair_window(&mut self, ctx: &egui::Context) -> bool {
        let Some(remote) = self.remote.clone() else {
            return false;
        };
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
        if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") {
            return Self::pair_window_modern(ctx, remote);
        }
        let out = window(
            ctx,
            "opt-pair",
            // i18n: window title: Options → Remote → Pair a device...
            "Pair a device",
            [360.0, 420.0],
            false,
            LIGHT_BG(),
            |ui, o| {
                let p = ui.painter().clone();
                let urls = remote.urls();
                let (shown, paired) = remote.pairing();
                let close_label = if paired.is_some() {
                    // i18n: button: close the window (the device is paired)
                    "Done"
                } else {
                    "Close"
                };
                let close = winui::button_light(
                    ui,
                    r(o, 274.0, 387.0, 73.0, 21.0),
                    close_label,
                    ButtonKind::Default,
                    false,
                )
                .clicked();
                let Some(url) = urls.first() else {
                    winui::paragraph(
                        &p,
                        o.x + 14.0,
                        o.y + 30.0,
                        // i18n: Options → Remote → Pair a device: the remote control isn't listening (no network, port taken): the Remote page says why
                        crate::i18n::t(
                            "SDL can't be reached on the network now: Options → Remote says why.",
                        ),
                        crate::theme::colors().error_strong,
                        o.x + 346.0,
                        14.0,
                    );
                    return if close || winui::escape_pressed(ui) {
                        Outcome::Closed
                    } else {
                        Outcome::Open
                    };
                };
                if let Some(name) = &paired {
                    let line = crate::i18n::fmt(
                        // i18n: Options → Remote → Pair a device: done; {device} = the name it was given
                        crate::i18n::t("{device} is paired. It can control SDL now."),
                        &[("device", &crate::i18n::isolate(name))],
                    );
                    winui::paragraph(
                        &p,
                        o.x + 14.0,
                        o.y + 30.0,
                        &line,
                        LIGHT_TEXT(),
                        o.x + 346.0,
                        14.0,
                    );
                    return if close || winui::escape_pressed(ui) || winui::enter_pressed(ui) {
                        Outcome::Done(())
                    } else {
                        Outcome::Open
                    };
                }
                winui::paragraph(
                    &p,
                    o.x + 14.0,
                    o.y + 24.0,
                    // i18n: Options → Remote → Pair a device: how to pair
                    crate::i18n::t(
                        "On the phone, scan this code with the camera, or open the address and enter the code.",
                    ),
                    LIGHT_TEXT(),
                    o.x + 346.0,
                    14.0,
                );
                let link = match &shown {
                    Some((code, _)) => format!("{url}#pair={code}"),
                    None => url.clone(),
                };
                crate::remote::paint_qr(&p, r(o, 90.0, 56.0, 180.0, 180.0), &link);
                readonly_ltr(ui, r(o, 14.0, 246.0, 332.0, 21.0), url);
                match &shown {
                    Some((code, left)) => {
                        text(
                            &p,
                            o.x + 14.0,
                            o.y + 300.0,
                            // i18n: Options → Remote → Pair a device: label before the 8-digit code
                            crate::i18n::t("Code:"),
                            LIGHT_TEXT(),
                        );
                        let big = egui::FontId::new(26.0, winui::tahoma_bold().family);
                        winui::text_font(
                            &p,
                            big,
                            o.x + 80.0,
                            o.y + 304.0,
                            &crate::i18n::ltr(&format!("{} {}", &code[..4], &code[4..])),
                            LIGHT_TEXT(),
                        );
                        let secs = left.as_secs();
                        let line = crate::i18n::fmt(
                            // i18n: Options → Remote → Pair a device: how long the code stays valid; {time} = minutes:seconds ("9:58")
                            crate::i18n::t("Waiting for the device. The code works for {time}."),
                            &[("time", &format!("{}:{:02}", secs / 60, secs % 60))],
                        );
                        winui::paragraph(
                            &p,
                            o.x + 14.0,
                            o.y + 336.0,
                            &line,
                            LIGHT_TEXT(),
                            o.x + 346.0,
                            14.0,
                        );
                    }
                    None => {
                        winui::paragraph(
                            &p,
                            o.x + 14.0,
                            o.y + 300.0,
                            // i18n: Options → Remote → Pair a device: the code ran out (10 minutes, or 5 wrong tries)
                            crate::i18n::t("This code can't be used any more."),
                            crate::theme::colors().error_strong,
                            o.x + 346.0,
                            14.0,
                        );
                        if winui::button_light(
                            ui,
                            r(o, 14.0, 318.0, 100.0, 21.0),
                            // i18n: Options → Remote → Pair a device: button: show a new code
                            "New code",
                            ButtonKind::Normal,
                            false,
                        )
                        .clicked()
                        {
                            remote.start_pairing();
                        }
                    }
                }
                if close || winui::escape_pressed(ui) {
                    Outcome::Closed
                } else {
                    Outcome::Open
                }
            },
        );
        matches!(out, Outcome::Open)
    }

    fn sub_dialog(&mut self, ctx: &egui::Context) {
        let Some(sub) = self.sub.take() else { return };
        match sub {
            Sub::Pair => {
                if self.pair_window(ctx) {
                    self.sub = Some(Sub::Pair);
                } else if let Some(r) = &self.remote {
                    r.stop_pairing();
                }
            }
            Sub::Login(row, mut site, mut user, mut pass) => {
                let title = if row.is_some() {
                    crate::i18n::mark("Edit login")
                } else {
                    crate::i18n::mark("New login")
                };
                let modern_login = matches!(crate::theme::current().id.as_ref(), "win" | "fluent");
                let out = window(
                    ctx,
                    "opt-login",
                    title,
                    if modern_login {
                        [560.0, 344.0]
                    } else {
                        [300.0, 150.0]
                    },
                    false,
                    LIGHT_BG(),
                    |ui, o| {
                        if modern_login {
                            return modern_login_dialog(ui, o, &mut site, &mut user, &mut pass);
                        }
                        let p = ui.painter().clone();
                        // i18n: label before a web address (site and folder)
                        winui::text_right(
                            &p,
                            o.x + 70.0,
                            o.y + 26.0,
                            crate::i18n::t("Site/path:"),
                            LIGHT_TEXT(),
                        );
                        winui::edit_light(
                            ui,
                            r(o, 78.0, 12.0, 210.0, 23.0),
                            &mut site,
                            true,
                            false,
                        );
                        // i18n: noun: label before the user name
                        winui::text_right(
                            &p,
                            o.x + 70.0,
                            o.y + 57.0,
                            crate::i18n::t("User:"),
                            LIGHT_TEXT(),
                        );
                        winui::edit_light(
                            ui,
                            r(o, 78.0, 43.0, 210.0, 23.0),
                            &mut user,
                            true,
                            false,
                        );
                        winui::text_right(
                            &p,
                            o.x + 70.0,
                            o.y + 88.0,
                            crate::i18n::t("Password:"),
                            LIGHT_TEXT(),
                        );
                        password(ui, r(o, 78.0, 74.0, 210.0, 23.0), &mut pass, true);
                        ok_cancel(ui, o, 136.0, 117.0, !site.trim().is_empty())
                    },
                );
                match out {
                    Outcome::Open => self.sub = Some(Sub::Login(row, site, user, pass)),
                    Outcome::Done(()) => {
                        let entry = (site.trim().to_owned(), user, pass);
                        match row {
                            Some(i) => self.s.site_logins[i] = entry,
                            None => self.s.site_logins.push(entry),
                        }
                    }
                    Outcome::Closed => {}
                }
            }
            Sub::Exception(row, mut server, mut number) => {
                let limit = self.s.connection_limit();
                let title = if row.is_some() {
                    crate::i18n::mark("Edit exception")
                } else {
                    crate::i18n::mark("New exception")
                };
                let wide_exception =
                    matches!(crate::theme::current().id.as_ref(), "win" | "fluent");
                let out = window(
                    ctx,
                    "opt-exception",
                    title,
                    if wide_exception {
                        [560.0, 256.0]
                    } else {
                        [300.0, 118.0]
                    },
                    false,
                    LIGHT_BG(),
                    |ui, o| {
                        if wide_exception {
                            return modern_exception_dialog(ui, o, &mut server, &mut number, limit);
                        }
                        let p = ui.painter().clone();
                        // i18n: noun: label before a server name
                        winui::text_right(
                            &p,
                            o.x + 70.0,
                            o.y + 26.0,
                            crate::i18n::t("Server:"),
                            LIGHT_TEXT(),
                        );
                        winui::edit_light(
                            ui,
                            r(o, 78.0, 12.0, 210.0, 23.0),
                            &mut server,
                            true,
                            false,
                        );
                        // i18n: label before the number of connections allowed
                        winui::text_right(
                            &p,
                            o.x + 70.0,
                            o.y + 57.0,
                            crate::i18n::t("Connections:"),
                            LIGHT_TEXT(),
                        );
                        winui::spin_box(
                            ui,
                            r(o, 78.0, 43.0, 60.0, 23.0),
                            &mut number,
                            1,
                            u64::from(limit),
                            true,
                            "opt-exception-number",
                        );
                        let in_range = number
                            .parse::<u32>()
                            .is_ok_and(|n| (1..=limit).contains(&n));
                        ok_cancel(ui, o, 136.0, 85.0, !server.trim().is_empty() && in_range)
                    },
                );
                match out {
                    Outcome::Open => self.sub = Some(Sub::Exception(row, server, number)),
                    Outcome::Done(()) => {
                        let entry = (
                            server.trim().to_owned(),
                            number.parse().unwrap_or(1).clamp(1, limit),
                        );
                        match row {
                            Some(i) => self.s.conn_exceptions[i] = entry,
                            None => self.s.conn_exceptions.push(entry),
                        }
                    }
                    Outcome::Closed => {}
                }
            }
            Sub::Panels(mut on, mut sites, mut said) => {
                let out = window(
                    ctx,
                    "opt-panels",
                    // i18n: window title: the download button the extension shows over videos
                    "Download panels",
                    [380.0, 190.0],
                    false,
                    LIGHT_BG(),
                    |ui, o| {
                        let p = ui.painter().clone();
                        winui::checkbox_light(
                            ui,
                            o + vec2(14.0, 16.0),
                            &mut on,
                            "Show a download button over videos in browsers",
                            true,
                        );
                        winui::text_in_window(
                            ui,
                            &p,
                            o.x + 14.0,
                            o.y + 55.0,
                            crate::i18n::t("Never show it on these sites:"),
                            if on { LIGHT_TEXT() } else { LIGHT_DISABLED() },
                        );
                        multiline(
                            ui,
                            r(o, 14.0, 62.0, 352.0, 55.0),
                            &mut sites,
                            "panel-sites",
                            on,
                        );
                        winui::text_in_window(
                            ui,
                            &p,
                            o.x + 14.0,
                            o.y + 133.0,
                            crate::i18n::t("(separate names by spaces)"),
                            LIGHT_TEXT(),
                        );
                        // The list as a file, to keep or carry to a new installation (#424).
                        let [export_at, import_at] = winui::button_row(
                            &p,
                            &[("Export...", 212.0, 73.0), ("Import...", 293.0, 73.0)],
                            8.0,
                            160.0,
                            140.0,
                        )[..] else {
                            unreachable!("two buttons")
                        };
                        if winui::button_light(
                            ui,
                            r(o, export_at.0, 128.0, export_at.1, 21.0),
                            "Export...",
                            ButtonKind::Normal,
                            false,
                        )
                        .clicked()
                        {
                            let title = crate::i18n::t("Export").to_owned();
                            winui::start_pick(ui.ctx(), "opt-panels-export", move || {
                                rfd::FileDialog::new()
                                    .set_title(title)
                                    .add_filter("TXT", &["txt"])
                                    .set_file_name("sites.txt")
                                    .save_file()
                            });
                        }
                        if winui::button_light(
                            ui,
                            r(o, import_at.0, 128.0, import_at.1, 21.0),
                            "Import...",
                            ButtonKind::Normal,
                            false,
                        )
                        .clicked()
                        {
                            let title = crate::i18n::t("Import").to_owned();
                            winui::start_pick(ui.ctx(), "opt-panels-import", move || {
                                rfd::FileDialog::new()
                                    .set_title(title)
                                    .add_filter("TXT", &["txt"])
                                    .pick_file()
                            });
                        }
                        if let Some(path) = winui::picked(ui.ctx(), "opt-panels-export") {
                            said = match std::fs::write(
                                &path,
                                sdl_ipc::capture::sites_to_text(&sites),
                            ) {
                                Ok(()) => crate::i18n::tn(
                                    sites.split_whitespace().count() as u64,
                                    "{n} site saved to the file.",
                                    "{n} sites saved to the file.",
                                ),
                                Err(e) => crate::i18n::fmt(
                                    crate::i18n::t("The list could not be saved: {error}"),
                                    &[("error", &e.to_string())],
                                ),
                            };
                        }
                        if let Some(path) = winui::picked(ui.ctx(), "opt-panels-import") {
                            said = match read_sites_file(&path) {
                                Ok(text) => {
                                    let (merged, added) =
                                        sdl_ipc::capture::merge_sites(&sites, &text);
                                    sites = merged;
                                    crate::i18n::tn(
                                        added as u64,
                                        "{n} site added from the file.",
                                        "{n} sites added from the file.",
                                    )
                                }
                                Err(e) => crate::i18n::fmt(
                                    crate::i18n::t("The file could not be read: {error}"),
                                    &[("error", &e.to_string())],
                                ),
                            };
                        }
                        if !said.is_empty() {
                            let (rect, cut) = winui::text_fit(
                                &p,
                                o.x + 14.0,
                                o.y + 163.0,
                                &said,
                                LIGHT_TEXT(),
                                o.x + 205.0,
                            );
                            if cut {
                                winui::full_on_hover(ui, rect, "opt-panels-said", &said);
                            }
                        }
                        ok_cancel(ui, o, 212.0, 157.0, true)
                    },
                );
                match out {
                    Outcome::Open => self.sub = Some(Sub::Panels(on, sites, said)),
                    Outcome::Done(()) => {
                        self.s.video_panel = on;
                        self.s.video_panel_sites =
                            sites.split_whitespace().collect::<Vec<_>>().join(" ");
                    }
                    Outcome::Closed => {}
                }
            }
            Sub::Dns(mut e) => {
                let wide_dns = matches!(crate::theme::current().id.as_ref(), "win" | "fluent");
                let out = window(
                    ctx,
                    "opt-dns",
                    // i18n: window title: the settings of the DNS chosen in Options → Connection
                    "DNS settings",
                    if wide_dns {
                        [644.0, 588.0]
                    } else {
                        [400.0, 312.0]
                    },
                    false,
                    LIGHT_BG(),
                    |ui, o| {
                        if wide_dns {
                            dns_dialog_modern(ui, o, &mut e)
                        } else {
                            dns_dialog(ui, o, &mut e)
                        }
                    },
                );
                match out {
                    Outcome::Open => self.sub = Some(Sub::Dns(e)),
                    Outcome::Done(()) => e.save(&mut self.s),
                    Outcome::Closed => {}
                }
            }
            Sub::Account(mut e) => {
                let modern = matches!(crate::theme::current().id.as_ref(), "win" | "fluent");
                let out = window(
                    ctx,
                    "opt-account",
                    // i18n: window title: adding a debrid service account (Real-Debrid…)
                    "Add an account",
                    if modern {
                        [680.0, 610.0]
                    } else {
                        [420.0, 300.0]
                    },
                    false,
                    LIGHT_BG(),
                    |ui, o| {
                        if modern {
                            modern_account_dialog(ui, o, &mut e)
                        } else {
                            account_dialog(ui, o, &mut e)
                        }
                    },
                );
                match out {
                    Outcome::Open => self.sub = Some(Sub::Account(e)),
                    Outcome::Done(account) => {
                        let mut account = account;
                        account.id = self
                            .s
                            .debrid_accounts
                            .iter()
                            .map(|a| a.id)
                            .max()
                            .unwrap_or(0)
                            + 1;
                        self.s.debrid_accounts.push(account);
                    }
                    Outcome::Closed => e
                        .signin
                        .stop
                        .store(true, std::sync::atomic::Ordering::Relaxed),
                }
            }
            Sub::Addresses(mut list) => {
                let out = window(
                    ctx,
                    "opt-addresses",
                    // i18n: window title: the addresses whose downloads stay with the browser
                    "Addresses left to the browser",
                    [420.0, 230.0],
                    false,
                    LIGHT_BG(),
                    |ui, o| {
                        let p = ui.painter().clone();
                        winui::paragraph(
                            &p,
                            o.x + 14.0,
                            o.y + 20.0,
                            crate::i18n::t(
                                "One address per line; * stands for any text: example.com/private/*",
                            ),
                            LIGHT_TEXT(),
                            o.x + 406.0,
                            14.0,
                        );
                        multiline(
                            ui,
                            r(o, 14.0, 44.0, 392.0, 130.0),
                            &mut list,
                            "addresses",
                            true,
                        );
                        ok_cancel(ui, o, 252.0, 192.0, true)
                    },
                );
                match out {
                    Outcome::Open => self.sub = Some(Sub::Addresses(list)),
                    Outcome::Done(()) => {
                        self.s.excluded_addresses = list
                            .lines()
                            .map(str::trim)
                            .filter(|l| !l.is_empty())
                            .map(str::to_owned)
                            .collect();
                    }
                    Outcome::Closed => {}
                }
            }
            Sub::Keys(mut opened) => {
                let out = window(
                    ctx,
                    "opt-keys",
                    // i18n: window title: the keys that force or prevent downloading with SDL
                    "Keys",
                    [400.0, 160.0],
                    false,
                    LIGHT_BG(),
                    |ui, o| {
                        let p = ui.painter().clone();
                        winui::paragraph(
                            &p,
                            o.x + 14.0,
                            o.y + 22.0,
                            crate::i18n::t(
                                "Hold Alt while clicking a link to let the browser download it instead of Swoopdl.",
                            ),
                            LIGHT_TEXT(),
                            o.x + 386.0,
                            14.0,
                        );
                        winui::checkbox_light(
                            ui,
                            o + vec2(14.0, 70.0),
                            &mut opened,
                            "Also take files the browser would open (PDF, music, video)",
                            true,
                        );
                        ok_cancel(ui, o, 232.0, 127.0, true)
                    },
                );
                match out {
                    Outcome::Open => self.sub = Some(Sub::Keys(opened)),
                    Outcome::Done(()) => self.s.capture_opened = opened,
                    Outcome::Closed => {}
                }
            }
            Sub::Test(mut checks) => {
                let h = 120.0 + checks.len() as f32 * 22.0;
                let mut set_up = false;
                let out = window(
                    ctx,
                    "opt-test",
                    // i18n: window title: Options → General → Test browser integration
                    "Test browser integration",
                    [440.0, h],
                    false,
                    LIGHT_BG(),
                    |ui, o| {
                        let p = ui.painter().clone();
                        for (k, c) in checks.iter().enumerate() {
                            let y = o.y + 22.0 + k as f32 * 22.0;
                            let (mark, color) = if c.ok {
                                ("\u{2714}", winui::SUCCESS())
                            } else {
                                ("\u{2716}", winui::ERROR())
                            };
                            winui::text_in_window(ui, &p, o.x + 14.0, y, mark, color);
                            winui::text_in_window(ui, &p, o.x + 34.0, y, &c.text, LIGHT_TEXT());
                        }
                        let first = checks.iter().find(|c| !c.ok);
                        let y = h - 46.0;
                        // As wide as their labels in this language (#192), short of Close.
                        let [fix_at, set_up_at] = winui::button_row_from_left(
                            &p,
                            &[("How to fix it", 14.0, 110.0), ("Set it up", 132.0, 110.0)],
                            8.0,
                            440.0 - 14.0 - 73.0 - 8.0,
                            200.0,
                        )[..] else {
                            unreachable!("two buttons")
                        };
                        if let Some(c) = first {
                            if let Some(why) = c.why
                                && winui::button_light(
                                    ui,
                                    r(o, fix_at.0, y, fix_at.1, 25.0),
                                    // i18n: button in Test browser integration: open the help page's section for the problem
                                    "How to fix it",
                                    ButtonKind::Normal,
                                    false,
                                )
                                .clicked()
                            {
                                crate::help::open_page(&sdl_ipc::extension::help_doc(why));
                            }
                            if c.why == Some("not-installed")
                                && sdl_ipc::portable_dir().is_none()
                                && winui::button_light(
                                    ui,
                                    r(o, set_up_at.0, y, set_up_at.1, 25.0),
                                    // i18n: button in Test browser integration: register SDL's helper with the browsers
                                    "Set it up",
                                    ButtonKind::Normal,
                                    false,
                                )
                                .clicked()
                            {
                                set_up = true;
                            }
                        }
                        let close = winui::button_light(
                            ui,
                            r(o, 440.0 - 14.0 - 73.0, y, 73.0, 25.0),
                            "Close",
                            ButtonKind::Default,
                            false,
                        );
                        if close.clicked() || winui::enter_pressed(ui) || winui::escape_pressed(ui)
                        {
                            Outcome::<()>::Closed
                        } else {
                            Outcome::Open
                        }
                    },
                );
                // Set it up runs on a thread of its own; the checks are read again once it's done.
                if set_up {
                    crate::extension::set_up(ctx);
                }
                if crate::extension::set_up_finished(&mut self.set_up_seen) {
                    checks = crate::extension::checks(&self.s);
                }
                if matches!(out, Outcome::Open) {
                    self.sub = Some(Sub::Test(checks));
                }
            }
        }
    }
}

// ---- pieces -----------------------------------------------------------------------------

/// Proxy / Socks → Get System: Windows' own proxy (host, port), if one is set.
fn system_proxy() -> Option<(String, String)> {
    #[cfg(windows)]
    return crate::win32::system_proxy();
    #[cfg(not(windows))]
    None
}

pub fn play_sound(file: &str) {
    if crate::offscreen() {
        return;
    }
    #[cfg(windows)]
    crate::win32::play_sound(file);
    #[cfg(unix)]
    crate::unix::play_sound(file);
}

/// A folder as the dialogs show it: the full path, as IDM's Options shows it, ending in the
/// system's separator (a backslash on Windows).
fn display_dir(dir: &std::path::Path) -> String {
    let s = dir.display().to_string();
    if s.ends_with(std::path::MAIN_SEPARATOR) {
        s
    } else {
        format!("{s}{}", std::path::MAIN_SEPARATOR)
    }
}

/// OK / Cancel at (x, y), OK only when `valid`.
/// Connection → DNS → Settings...: how it's asked, its servers, the fallback, a test.
fn dns_dialog(ui: &mut Ui, o: Pos2, e: &mut DnsEdit) -> Outcome<()> {
    use sdl_engine::dns::{Mode, Preset};
    let p = ui.painter().clone();
    let preset = Preset::by_id(&e.provider);
    // i18n: Options → DNS settings: heading over the ways of asking the DNS
    text(
        &p,
        o.x + 14.0,
        o.y + 22.0,
        crate::i18n::t("Ask it with:"),
        LIGHT_TEXT(),
    );
    let modes = [
        (1u8, Mode::Plain, "plain DNS"),
        (2, Mode::Tls, "DNS over TLS"),
        (3, Mode::Https, "DNS over HTTPS"),
    ];
    let mut clicked = Vec::new();
    for (row, (n, m, caption)) in modes.iter().enumerate() {
        let offered = preset.is_none_or(|p| p.offers(*m));
        let r = winui::radio_light(
            ui,
            o + vec2(24.0, 32.0 + 20.0 * row as f32),
            e.mode == *n,
            caption,
            offered,
        );
        if r.clicked() && offered {
            e.mode = *n;
        }
        clicked.push(r);
    }
    if let Some(i) = winui::radio_keys(ui, &clicked) {
        let (n, m, _) = modes[i];
        if preset.is_none_or(|p| p.offers(m)) {
            e.mode = n;
        }
    }
    let field = |ui: &mut Ui, y: f32, label: &str, value: &mut String, enabled: bool| {
        winui::text_right(
            &p,
            o.x + 130.0,
            o.y + y + 14.0,
            crate::i18n::t(label),
            LIGHT_TEXT(),
        );
        winui::edit_light(ui, r(o, 138.0, y, 248.0, 21.0), value, enabled, false);
    };
    let mode = settings::dns_mode(e.mode).unwrap_or(Mode::Https);
    match preset {
        // A service: its servers as they are, NextDNS's configuration ID to type.
        Some(pr) => {
            let shown = match mode {
                Mode::Plain => pr.plain.join(", "),
                Mode::Tls => pr.tls_name.to_owned(),
                Mode::Https => pr.doh.to_owned(),
            };
            // i18n: Options → DNS settings: label before the chosen service's server (an address)
            winui::text_right(
                &p,
                o.x + 130.0,
                o.y + 112.0,
                crate::i18n::t("Server:"),
                LIGHT_TEXT(),
            );
            let (rect, cut) = winui::label_into(
                &p,
                o.x + 138.0,
                o.y + 112.0,
                &shown,
                LIGHT_DISABLED(),
                o.x + 386.0,
                o.x + 138.0,
            );
            if cut {
                winui::full_on_hover(ui, rect, "dns-server", &shown);
            }
            if pr.id == "nextdns" {
                // i18n: Options → DNS settings: NextDNS's own configuration ID (optional)
                field(
                    ui,
                    122.0,
                    "Configuration ID:",
                    &mut e.nextdns_id,
                    mode != Mode::Plain,
                );
            }
        }
        // The user's own DNS.
        None => match mode {
            // i18n: Options → DNS settings: label before the DNS servers' addresses, as IP addresses
            Mode::Plain => field(ui, 98.0, "Servers:", &mut e.servers, true),
            Mode::Tls => {
                field(ui, 98.0, "Servers:", &mut e.servers, true);
                // i18n: Options → DNS over TLS: label before the name the server's certificate shows
                field(ui, 124.0, "Certificate name:", &mut e.tls_name, true);
            }
            Mode::Https => {
                // i18n: Options → DNS over HTTPS: label before the server's https:// address
                field(ui, 98.0, "Address:", &mut e.doh_url, true);
                // i18n: Options → DNS over HTTPS: label before the server's IP addresses, which may be left empty
                field(ui, 124.0, "IP addresses (optional):", &mut e.servers, true);
            }
        },
    }
    winui::checkbox_light(
        ui,
        o + vec2(14.0, 158.0),
        &mut e.fallback,
        // i18n: Options → DNS settings: checkbox: when the chosen DNS can't be reached, use the system's instead
        "If it can't be reached, use the system's DNS",
        true,
    );
    winui::paragraph(
        &p,
        o.x + 14.0,
        o.y + 192.0,
        // i18n: Options → DNS settings: what the DNS choice covers and what it doesn't
        crate::i18n::t(
            "Names in your hosts file and local names (single words, .local, .lan) stay with the system's DNS. Video pages and torrent trackers use the system's DNS for now.",
        ),
        LIGHT_TEXT(),
        o.x + 386.0,
        14.0,
    );
    let choice = e.choice();
    // Test: asks github.com through the choice, without falling back, on a thread of its own.
    let running = e.test.as_ref().is_some_and(|t| t.lock().unwrap().is_none());
    let kind = if choice.is_ok() && !running {
        ButtonKind::Normal
    } else {
        ButtonKind::Disabled
    };
    // i18n: button: asks the chosen DNS for an address, to see it works
    if winui::button_light(ui, r(o, 14.0, 232.0, 73.0, 21.0), "Test", kind, false).clicked()
        && let Ok(c) = &choice
    {
        let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
        e.test = Some(slot.clone());
        let (c, ctx) = (c.clone(), ui.ctx().clone());
        // Through the network adapter downloads use (#35).
        let route = crate::settings::own_route();
        std::thread::spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())
                .and_then(|rt| {
                    rt.block_on(sdl_engine::dns::test(&c, "github.com", &[], &route))
                        .map_err(|e| e.to_string())
                });
            let line = match result {
                Ok((took, ips)) => (
                    crate::i18n::fmt(
                        // i18n: Options → DNS settings: the test's answer; {ms} milliseconds, {ip} the address found
                        crate::i18n::t("Answered in {ms} ms: {ip}"),
                        &[
                            ("ms", &crate::i18n::integer(took.as_millis() as u64)),
                            (
                                "ip",
                                &ips.first().map(|i| i.to_string()).unwrap_or_default(),
                            ),
                        ],
                    ),
                    true,
                ),
                Err(why) => (why, false),
            };
            *slot.lock().unwrap() = Some(line);
            ctx.request_repaint();
        });
    }
    let status = match (&choice, &e.test) {
        (Err(why), _) => Some((why.clone(), false)),
        (Ok(_), Some(t)) => Some(t.lock().unwrap().clone().unwrap_or_else(|| {
            // i18n: Options → DNS settings: the test is running
            (crate::i18n::t("Asking...").to_owned(), true)
        })),
        _ => None,
    };
    if let Some((line, ok)) = status {
        let color = if ok {
            LIGHT_TEXT()
        } else {
            crate::theme::colors().error_strong
        };
        let (rect, cut) = winui::label_into(
            &p,
            o.x + 95.0,
            o.y + 247.0,
            &line,
            color,
            o.x + 386.0,
            o.x + 95.0,
        );
        if cut {
            winui::full_on_hover(ui, rect, "dns-test", &line);
        }
    }
    ok_cancel(ui, o, 238.0, 278.0, choice.is_ok())
}

/// Accounts → Add...: the service, a token typed in, and the sign-in on its thread (#44).
#[derive(Default)]
struct AccountEdit {
    service: sdl_debrid::ServiceKind,
    token: String,
    signin: crate::debrid::SignIn,
}

/// Accounts → Add...: sign in by code in the browser (Real-Debrid's device flow, AllDebrid's PIN),
/// or paste the API token (Premiumize: its API key only); the account is checked before it is kept.
fn account_dialog(ui: &mut Ui, o: Pos2, e: &mut AccountEdit) -> Outcome<settings::DebridAccount> {
    use crate::debrid::SignInState as S;
    let p = ui.painter().clone();
    let ctx = ui.ctx().clone();
    let state = e.signin.get();
    let busy = matches!(state, S::Starting | S::Waiting { .. } | S::Checking);
    // i18n: Options → Add an account: label before the choice of debrid service
    text(
        &p,
        o.x + 14.0,
        o.y + 26.0,
        crate::i18n::t("Service:"),
        LIGHT_TEXT(),
    );
    let all = sdl_debrid::ServiceKind::ALL;
    let names: Vec<&str> = all.iter().map(|k| k.name()).collect();
    let at = all.iter().position(|k| *k == e.service).unwrap_or(0);
    if let Some(i) = winui::combo_light(
        ui,
        r(o, 90.0, 12.0, 200.0, 21.0),
        &names,
        at,
        !busy,
        "opt-account-service",
    ) && all[i] != e.service
    {
        // Another service: what was signed in to (or typed) belongs to the one before.
        e.service = all[i];
        e.token.clear();
        *e.signin.state.lock().unwrap() = S::Idle;
    }
    let by_code = e.service.signs_in_by_code();
    let kind = |on: bool| {
        if on {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        }
    };
    // i18n: button: sign in to the debrid service in the browser, by typing a code there
    if winui::button_light(
        ui,
        r(o, 14.0, 44.0, 220.0, 23.0),
        "Sign in in the browser",
        kind(!busy && by_code),
        false,
    )
    .clicked()
    {
        e.signin
            .stop
            .store(false, std::sync::atomic::Ordering::Relaxed);
        e.signin.by_code(e.service, &ctx);
    }
    if let S::Waiting { code, page, until } = &state {
        // The code, large, to type at the service's page.
        let code_font = egui::FontId::monospace(crate::theme::metrics().font_code);
        p.text(
            o + vec2(14.0, 78.0),
            egui::Align2::LEFT_TOP,
            code,
            code_font.clone(),
            LIGHT_TEXT(),
        );
        // The buttons as wide as their labels in this language (#192), short of the code.
        let code_end = 14.0 + winui::text_width_font(&p, code_font, code);
        let [copy_at, open_at] = winui::button_row(
            &p,
            &[("Copy code", 200.0, 90.0), ("Open the page", 296.0, 110.0)],
            6.0,
            (code_end + 12.0).min(200.0),
            200.0,
        )[..] else {
            unreachable!("two buttons")
        };
        // i18n: button: copy the sign-in code
        if winui::button_light(
            ui,
            r(o, copy_at.0, 80.0, copy_at.1, 21.0),
            "Copy code",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            crate::copy_text(&ctx, code.clone());
        }
        // i18n: button: open the service's page where the code is typed
        if winui::button_light(
            ui,
            r(o, open_at.0, 80.0, open_at.1, 21.0),
            "Open the page",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            open_page(page);
        }
        let left = until
            .saturating_duration_since(std::time::Instant::now())
            .as_secs();
        winui::paragraph(
            &p,
            o.x + 14.0,
            o.y + 120.0,
            &crate::i18n::fmt(
                // i18n: Options → Add an account: waiting while the user types the code at the service's page; {page} its address, {time} minutes:seconds left
                crate::i18n::t(
                    "Type this code at {page} and allow SDL; waiting for you (the code expires in {time}).",
                ),
                &[
                    ("page", &crate::i18n::ltr(page)),
                    ("time", &format!("{}:{:02}", left / 60, left % 60)),
                ],
            ),
            LIGHT_TEXT(),
            o.x + 406.0,
            14.0,
        );
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
    let label = if by_code {
        // i18n: Options → Add an account: label before the field for the service's API token (the other way to sign in)
        crate::i18n::t("Or paste your API token:")
    } else {
        // i18n: Options → Add an account: label before the field for the service's API key (Premiumize: the only way to sign in; the key is on the service's Account page)
        crate::i18n::t("Paste your API key (from the service's Account page):")
    };
    text(&p, o.x + 14.0, o.y + 178.0, label, LIGHT_TEXT());
    // As wide as its label in this language (#192); the box ends 6 px before it.
    let (use_x, use_w) =
        winui::buttons_to_right(&p, 406.0, 0.0, &[("Use the token", 106.0)], 200.0)[0];
    winui::password_light(
        ui,
        r(o, 14.0, 186.0, use_x - 6.0 - 14.0, 21.0),
        &mut e.token,
        !busy,
    );
    // i18n: button: sign in with the token typed in
    if winui::button_light(
        ui,
        r(o, use_x, 186.0, use_w, 21.0),
        "Use the token",
        kind(!busy && !e.token.trim().is_empty()),
        false,
    )
    .clicked()
    {
        e.signin.by_token(e.service, e.token.clone(), &ctx);
    }
    // i18n: button: open the service's page that shows the user's API token
    if winui::button_light(
        ui,
        r(
            o,
            14.0,
            212.0,
            winui::button_fit_width(&p, "Find my token", 160.0, 392.0),
            21.0,
        ),
        "Find my token",
        ButtonKind::Normal,
        false,
    )
    .clicked()
    {
        open_page(e.service.token_page());
    }
    let (line, ok) = match &state {
        // i18n: Options → Add an account: the sign-in is starting or being checked
        S::Starting | S::Checking => (crate::i18n::t("Asking the service...").to_owned(), true),
        S::Done { user, state, .. } => (
            crate::i18n::fmt(
                // i18n: Options → Add an account: signed in; {user} the account's name, {state} premium or not
                crate::i18n::t("Signed in as {user} ({state})"),
                &[("user", &crate::i18n::ltr(user)), ("state", state)],
            ),
            true,
        ),
        S::Failed(why) => (why.clone(), false),
        _ => (String::new(), true),
    };
    let color = if ok {
        LIGHT_TEXT()
    } else {
        crate::theme::colors().error_strong
    };
    let (rect, cut) = winui::label_into(
        &p,
        o.x + 14.0,
        o.y + 252.0,
        &line,
        color,
        o.x + 406.0,
        o.x + 14.0,
    );
    if cut {
        winui::full_on_hover(ui, rect, "opt-account-state", &line);
    }
    let done = matches!(state, S::Done { .. });
    match ok_cancel(ui, o, 252.0, 266.0, done) {
        Outcome::Done(()) => match state {
            S::Done {
                user,
                state,
                credential,
            } => Outcome::Done(settings::DebridAccount {
                id: 0,
                service: e.service,
                enabled: true,
                user,
                state,
                secret: credential,
            }),
            _ => Outcome::Open,
        },
        Outcome::Closed => Outcome::Closed,
        Outcome::Open => Outcome::Open,
    }
}

/// Win/Fluent Accounts Add dialog. The same sign-in state machine, timeout,
/// token flow, keyboard actions, and credential return type as the classic dialog.
fn modern_account_dialog(
    ui: &mut Ui,
    o: Pos2,
    e: &mut AccountEdit,
) -> Outcome<settings::DebridAccount> {
    use crate::debrid::SignInState as S;
    let _font = winui::ModernDialogFont::enter();
    let p = ui.painter().clone();
    let ctx = ui.ctx().clone();
    let state = e.signin.get();
    let busy = matches!(state, S::Starting | S::Waiting { .. } | S::Checking);
    settings_card(ui, o, 18.0, 508.0);
    modern_section_title(ui, o, 56.0, "Add an account");
    modern_field_label(ui, o, 87.0, "Service:");
    let all = sdl_debrid::ServiceKind::ALL;
    let names: Vec<&str> = all.iter().map(|k| k.name()).collect();
    let at = all.iter().position(|k| *k == e.service).unwrap_or(0);
    if let Some(i) = winui::combo_light(
        ui,
        r(o, 32.0, 101.0, 612.0, 36.0),
        &names,
        at,
        !busy,
        "opt-account-service",
    ) && all[i] != e.service
    {
        e.service = all[i];
        e.token.clear();
        *e.signin.state.lock().unwrap() = S::Idle;
    }
    let by_code = e.service.signs_in_by_code();
    let kind = |on: bool| {
        if on {
            ButtonKind::Normal
        } else {
            ButtonKind::Disabled
        }
    };
    if winui::button_light(
        ui,
        r(o, 32.0, 160.0, 252.0, 36.0),
        "Sign in in the browser",
        kind(!busy && by_code),
        false,
    )
    .clicked()
    {
        e.signin
            .stop
            .store(false, std::sync::atomic::Ordering::Relaxed);
        e.signin.by_code(e.service, &ctx);
    }
    if let S::Waiting { code, page, until } = &state {
        let code_font = egui::FontId::monospace(crate::theme::metrics().font_code);
        p.text(
            o + vec2(32.0, 218.0),
            egui::Align2::LEFT_TOP,
            code,
            code_font,
            LIGHT_TEXT(),
        );
        if winui::button_light(
            ui,
            r(o, 288.0, 217.0, 154.0, 35.0),
            "Copy code",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            crate::copy_text(&ctx, code.clone());
        }
        if winui::button_light(
            ui,
            r(o, 452.0, 217.0, 192.0, 35.0),
            "Open the page",
            ButtonKind::Normal,
            false,
        )
        .clicked()
        {
            open_page(page);
        }
        let left = until
            .saturating_duration_since(std::time::Instant::now())
            .as_secs();
        winui::paragraph(
            &p,
            o.x + 32.0,
            o.y + 289.0,
            &crate::i18n::fmt(
                crate::i18n::t(
                    "Type this code at {page} and allow SDL; waiting for you (the code expires in {time}).",
                ),
                &[
                    ("page", &crate::i18n::ltr(page)),
                    ("time", &format!("{}:{:02}", left / 60, left % 60)),
                ],
            ),
            LIGHT_TEXT(),
            o.x + 640.0,
            15.0,
        );
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
    modern_field_label(
        ui,
        o,
        344.0,
        if by_code {
            "Or paste your API token:"
        } else {
            "Paste your API key (from the service's Account page):"
        },
    );
    winui::password_light(ui, r(o, 32.0, 360.0, 430.0, 36.0), &mut e.token, !busy);
    if winui::button_light(
        ui,
        r(o, 472.0, 360.0, 172.0, 36.0),
        "Use the token",
        kind(!busy && !e.token.trim().is_empty()),
        false,
    )
    .clicked()
    {
        e.signin.by_token(e.service, e.token.clone(), &ctx);
    }
    if winui::button_light(
        ui,
        r(o, 32.0, 410.0, 178.0, 34.0),
        "Find my token",
        ButtonKind::Normal,
        false,
    )
    .clicked()
    {
        open_page(e.service.token_page());
    }
    let (line, ok) = match &state {
        S::Starting | S::Checking => (crate::i18n::t("Asking the service...").to_owned(), true),
        S::Done { user, state, .. } => (
            crate::i18n::fmt(
                crate::i18n::t("Signed in as {user} ({state})"),
                &[("user", &crate::i18n::ltr(user)), ("state", state)],
            ),
            true,
        ),
        S::Failed(why) => (why.clone(), false),
        _ => (String::new(), true),
    };
    let ink = if ok {
        LIGHT_TEXT()
    } else {
        crate::theme::colors().error_strong
    };
    let (rect, cut) = winui::label_into(
        &p,
        o.x + 32.0,
        o.y + 491.0,
        &line,
        ink,
        o.x + 644.0,
        o.x + 32.0,
    );
    if cut {
        winui::full_on_hover(ui, rect, "opt-account-state", &line);
    }
    let done = matches!(state, S::Done { .. });
    match modern_modal_actions(ui, o, 548.0, 644.0, done) {
        Outcome::Done(()) => match state {
            S::Done {
                user,
                state,
                credential,
            } => Outcome::Done(settings::DebridAccount {
                id: 0,
                service: e.service,
                enabled: true,
                user,
                state,
                secret: credential,
            }),
            _ => Outcome::Open,
        },
        Outcome::Closed => Outcome::Closed,
        Outcome::Open => Outcome::Open,
    }
}

/// Open a service's web page in the browser; never in an off-screen test run (SDL_OFFSCREEN),
/// whose clicks come from scripts, not from a person at the browser.
fn open_page(url: &str) {
    if !crate::offscreen() {
        crate::launch::open(url);
    }
}

/// A list of sites from a file (Download panels' Import, #424): text, and no more than a list
/// could be.
fn read_sites_file(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read as _;
    let mut text = Vec::new();
    std::fs::File::open(path)?
        .take(1 << 20)
        .read_to_end(&mut text)?;
    String::from_utf8(text).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

fn ok_cancel(ui: &mut Ui, o: Pos2, x: f32, y: f32, valid: bool) -> Outcome<()> {
    let kind = if valid {
        ButtonKind::Default
    } else {
        ButtonKind::Disabled
    };
    let ok = winui::button_light(ui, r(o, x, y, 73.0, 21.0), "OK", kind, false).clicked();
    let cancel = winui::button_light(
        ui,
        r(o, x + 81.0, y, 73.0, 21.0),
        "Cancel",
        ButtonKind::Normal,
        false,
    )
    .clicked();
    if cancel || winui::escape_pressed(ui) {
        Outcome::Closed
    } else if valid && (ok || winui::enter_pressed(ui)) {
        Outcome::Done(())
    } else {
        Outcome::Open
    }
}

/// Scroll content heights for each of the twelve Options pages. Earlier modern shells
/// allocated only 480 points for ten pages and clipped controls on tall pages.
fn modern_page_height(page: usize) -> f32 {
    match page {
        0 => 1110.0, // General: browser cards after seven switches
        1 => 735.0,  // File types
        2 if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") => 790.0,
        2 => 700.0,  // Other themes retain the previous scroll extent
        3 => 1050.0, // Downloads
        4 if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") => 895.0,
        4 => 700.0, // Other themes retain the previous scroll extent
        5 if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") => 1250.0,
        5 => 750.0, // Proxy / Socks classic layout
        6 if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") => 960.0,
        6 => 700.0,  // Sites Logins classic layout
        7 => 1035.0, // Accounts: table, routing, security guidance
        8 => 745.0,  // Sounds: full event list and actions
        9 => 855.0,  // Regional: separate local-format fields and preview
        10 => 900.0, // Archives: extraction and Link grabber cards
        REMOTE_PAGE if matches!(crate::theme::current().id.as_ref(), "win" | "fluent") => 1010.0,
        REMOTE_PAGE => 830.0, // Remote classic layout
        _ => 720.0,
    }
}

/// Modern Win/Fluent background for each legacy-coordinate settings page. The
/// page's existing controls are painted afterwards, with their current IDs,
/// callbacks, keyboard focus and settings-draft semantics unchanged. Keeping
/// this behind the page avoids swallowing pointer input from nested dialogs.
fn modern_legacy_page_surface(ui: &Ui, o: Pos2, page: usize) {
    let c = crate::theme::colors();
    let p = ui.painter();
    let branded = crate::theme::current().id.as_ref() == "fluent";
    // All nine retained pages receive the same full-width composition; the
    // varying bottom edge leaves room for explanatory text and error states.
    let bottom = match page {
        2 | 4 | 5 | 6 | 7 | 8 | 9 | 10 => 542.0,
        REMOTE_PAGE => 576.0,
        _ => 542.0,
    };
    let frame = r(o, 16.0, 94.0, 660.0, bottom - 94.0);
    let corner = if branded { 14.0 } else { 8.0 };
    p.rect_filled(frame, corner, c.tab_page);
    p.rect_stroke(
        frame,
        corner,
        egui::Stroke::new(1.0, c.group_line),
        egui::StrokeKind::Inside,
    );
    // Same content rectangle and hitboxes, but a stronger branded rail for
    // Swoopdl Fluent and a reserved Windows 11 rail for Win.
    p.rect_filled(
        r(o, 16.0, 108.0, if branded { 5.0 } else { 3.0 }, 24.0),
        if branded { 2.5 } else { 2.0 },
        c.accent,
    );
    p.line_segment(
        [pos2(o.x + 16.0, o.y + 89.0), pos2(o.x + 676.0, o.y + 89.0)],
        egui::Stroke::new(1.0, c.group_line),
    );
}

/// Page-local horizontal reflow for the retained compact-coordinate settings
/// controls. Maps their rectangles, labels and hit targets together so the
/// visual layout and interaction positions never diverge. Nonmodern themes
/// return their original pixel geometry unchanged.
fn settings_scaled_x(value: f32) -> f32 {
    16.0 + (value - 16.0) * 1.55
}

fn settings_form_x(x: f32, value: f32) -> f32 {
    if crate::theme::modern_layout() {
        x + settings_scaled_x(value)
    } else {
        x + value
    }
}

fn settings_form_rect(o: Pos2, x: f32, y: f32, w: f32, h: f32) -> Rect {
    if crate::theme::modern_layout() {
        let height = if (20.0..=24.0).contains(&h) {
            h + 3.0
        } else {
            h
        };
        r(o, settings_scaled_x(x), y, w * 1.55, height)
    } else {
        r(o, x, y, w, h)
    }
}

fn settings_form_pos(o: Pos2, x: f32, y: f32) -> Pos2 {
    pos2(settings_form_x(o.x, x), o.y + y)
}

fn settings_etched_h(p: &egui::Painter, x: f32, y: f32, width: f32) {
    let width = if crate::theme::modern_layout() {
        width * 1.55
    } else {
        width
    };
    winui::etched_h(p, x, y, width);
}

/// Report column edges are in the legacy page coordinate system. Scale the
/// edges with the containing table, rather than stretching text or changing
/// row-selection logic.
#[allow(clippy::too_many_arguments)]
fn settings_report(
    ui: &mut Ui,
    o: Pos2,
    rc: Rect,
    cols: &[(&str, f32)],
    rows: &[Vec<String>],
    st: &mut winui::RowList,
    grid: bool,
    id: &str,
) -> (bool, bool) {
    let columns: Vec<(&str, f32)> = if crate::theme::modern_layout() {
        cols.iter()
            .map(|(title, right)| (*title, settings_scaled_x(*right)))
            .collect()
    } else {
        cols.to_vec()
    };
    report(ui, o, rc, &columns, rows, st, grid, id)
}

/// Graphite Current Settings shell. Drawing is deliberately passive: the
/// twelve existing page painters, all their hitboxes and the common footer
/// continue to own every interaction. Win and the other presets never enter
/// this path.
fn graphite_settings_shell(ui: &Ui, o: Pos2, client: [f32; 2]) {
    let p = ui.painter();
    let c = crate::theme::colors();
    let content = r(
        o,
        229.0,
        0.0,
        (client[0] - 229.0).max(0.0),
        (client[1] - 64.0).max(0.0),
    );
    let navigation = r(o, 0.0, 0.0, 229.0, (client[1] - 64.0).max(0.0));
    let footer = r(o, 0.0, client[1] - 64.0, client[0], 64.0);
    p.rect_filled(content, 0.0, c.window);
    p.rect_filled(navigation, 0.0, c.menubar);
    p.rect_filled(footer, 0.0, c.egui_window);
    p.line_segment(
        [
            pos2(winui::mx(o.x), o.y + client[1] - 64.0),
            pos2(winui::mx(o.x + client[0]), o.y + client[1] - 64.0),
        ],
        egui::Stroke::new(1.0, c.group_line),
    );
    p.rect_filled(r(o, 245.0, client[1] - 45.0, 3.0, 25.0), 1.5, c.accent);
    winui::text_font(
        p,
        winui::tahoma(),
        o.x + 262.0,
        o.y + client[1] - 29.0,
        crate::i18n::t("Changes are saved when you select OK"),
        c.muted,
    );
}

/// Restrained semantic icon colors for the Fluent Settings sidebar only.
/// The main theme's blue is reserved for the selected page and key actions.
fn graphite_settings_icon_color(page: usize, selected: bool) -> egui::Color32 {
    if selected {
        return crate::theme::colors().accent;
    }
    let dark = crate::theme::dark_surface();
    match page {
        1 | 7 => {
            if dark {
                egui::Color32::from_rgb(190, 172, 234)
            } else {
                egui::Color32::from_rgb(112, 84, 165)
            }
        }
        2 | 8 => {
            if dark {
                egui::Color32::from_rgb(104, 204, 194)
            } else {
                egui::Color32::from_rgb(13, 118, 123)
            }
        }
        4 | 5 | REMOTE_PAGE => {
            if dark {
                egui::Color32::from_rgb(241, 189, 115)
            } else {
                egui::Color32::from_rgb(145, 94, 30)
            }
        }
        6 | 10 => {
            if dark {
                egui::Color32::from_rgb(241, 159, 174)
            } else {
                egui::Color32::from_rgb(173, 67, 90)
            }
        }
        _ => crate::theme::colors().muted,
    }
}

/// Full-width card shared by modern options page layouts.
fn settings_card(ui: &Ui, o: Pos2, y: f32, height: f32) {
    let c = crate::theme::colors();
    let branded = crate::theme::current().id.as_ref() == "fluent";
    let radius = if branded { 10.0 } else { 8.0 };
    let rc = r(o, 16.0, y, 660.0, height);
    ui.painter().rect_filled(rc, radius, c.tab_page);
    ui.painter().rect_stroke(
        rc,
        radius,
        egui::Stroke::new(1.0, c.group_line),
        egui::StrokeKind::Inside,
    );
    // Graphite Current uses restrained card borders instead of repeated accent rails.
}

/// Keep translated labels within the card; the shared text painter records accessibility captions.
fn settings_label(ui: &mut Ui, o: Pos2, y: f32, label: &str) {
    let full = crate::i18n::t(label);
    let shown = winui::ellipsize(ui.painter(), &winui::tahoma(), full, 628.0);
    let drawn = winui::text(ui.painter(), o.x + 32.0, o.y + y, &shown, LIGHT_TEXT());
    if shown != full {
        winui::full_on_hover(ui, drawn, ("settings-label", label), full);
    }
}

/// A section heading inside the wide Win/Fluent page cards. It does not create a
/// separate hit target or translate commands; original page IDs stay unchanged.
fn modern_section_title(ui: &mut Ui, o: Pos2, baseline: f32, label: &str) {
    let p = ui.painter().clone();
    let font = winui::tahoma_bold();
    let full = crate::i18n::t(label);
    let shown = winui::ellipsize(&p, &font, full, 622.0);
    let branded = crate::theme::current().id.as_ref() == "fluent";
    if branded {
        p.circle_filled(
            pos2(winui::mx(o.x + 24.0), o.y + baseline - 7.0),
            3.0,
            crate::theme::colors().accent,
        );
    }
    let rc = winui::text_font(&p, font, o.x + 32.0, o.y + baseline, &shown, LIGHT_TEXT());
    if shown != full {
        winui::full_on_hover(ui, rc, ("modern-section", label), full);
    }
}

/// A visible label above a field; long translations remain available on hover.
fn modern_field_label(ui: &mut Ui, o: Pos2, baseline: f32, label: &str) {
    let p = ui.painter().clone();
    let font = winui::tahoma();
    let full = crate::i18n::t(label);
    let shown = winui::ellipsize(&p, &font, full, 620.0);
    let rc = winui::text_font(
        &p,
        font,
        o.x + 32.0,
        o.y + baseline,
        &shown,
        crate::theme::colors().muted,
    );
    if shown != full {
        winui::full_on_hover(ui, rc, ("modern-field", label.to_owned()), full);
    }
}

/// A Fluent setting card. Draft values still use OptionsDialog's original commit path.
fn fluent_setting(
    ui: &mut Ui,
    o: Pos2,
    value: &mut bool,
    label: &str,
    description: &str,
    enabled: bool,
) -> egui::Response {
    let c = crate::theme::colors();
    let branded = crate::theme::current().id.as_ref() == "fluent";
    let card_radius = if branded { 10.0 } else { 8.0 };
    let rc = r(o, 0.0, 0.0, 660.0, 63.0);
    let response = ui.interact(
        rc,
        egui::Id::unique(("fluent-setting", label)),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let keyboard_activate =
        enabled && response.has_focus() && winui::take_key(ui, egui::Key::Space);
    if enabled && (response.clicked() || keyboard_activate) {
        *value = !*value;
    }
    winui::expose(
        &response,
        egui::Role::Switch,
        crate::i18n::t(label),
        enabled,
        Some(*value),
    );
    let p = ui.painter();
    p.rect_filled(
        rc,
        card_radius,
        if enabled && response.hovered() {
            c.subtle_hot
        } else {
            c.tab_page
        },
    );
    p.rect_stroke(
        rc,
        card_radius,
        egui::Stroke::new(1.0, c.group_line),
        egui::StrokeKind::Inside,
    );
    let title = crate::i18n::t(label);
    let shown = winui::ellipsize(p, &winui::segoe(), title, 548.0);
    winui::text_font(
        p,
        winui::segoe(),
        o.x + 16.0,
        o.y + 26.0,
        &shown,
        if enabled { c.text } else { c.text_disabled },
    );
    let detail = winui::ellipsize(p, &winui::tahoma(), crate::i18n::t(description), 548.0);
    winui::text_font(p, winui::tahoma(), o.x + 16.0, o.y + 45.0, &detail, c.muted);
    let track = r(o, 602.0, 20.0, 41.0, 23.0);
    p.rect_filled(
        track,
        12.0,
        if !enabled {
            c.button_disabled
        } else if *value {
            c.accent
        } else {
            c.field_border
        },
    );
    let dot = winui::rect(Rect::from_center_size(
        pos2(o.x + if *value { 632.0 } else { 614.0 }, o.y + 31.5),
        vec2(17.0, 17.0),
    ));
    p.circle_filled(
        dot.center(),
        8.5,
        if *value && enabled {
            c.button_accent_text
        } else {
            c.text
        },
    );
    if response.has_focus() {
        winui::focus_ring(p, rc);
    }
    response.on_hover_text(format!("{}\n{}", title, crate::i18n::t(description)))
}

/// Fluent-only settings navigation. Each entry is the existing OptionsDialog page index;
/// no setting value or save/cancel semantics are changed here.
fn fluent_options_nav(ui: &mut Ui, o: Pos2, selected: usize) -> Option<usize> {
    const NAV: [(usize, &str); 12] = [
        (0, "General"),
        (1, "File types"),
        (2, "Save to"),
        (3, "Downloads"),
        (4, "Connection"),
        (REMOTE_PAGE, "Remote"),
        (5, "Proxy / Socks"),
        (6, "Sites Logins"),
        (7, "Accounts"),
        (8, "Sounds"),
        (9, "Regional"),
        (10, "Archives"),
    ];
    let p = ui.painter().clone();
    let c = crate::theme::colors();
    let branded = crate::theme::current().id.as_ref() == "fluent";
    let font = winui::segoe();
    let panel = winui::rect(Rect::from_min_size(o, vec2(229.0, 656.0)));
    p.rect_filled(panel, 0.0, if branded { c.menubar } else { c.window });
    let divider_x = winui::mx(o.x + 228.0);
    p.line_segment(
        [pos2(divider_x, o.y + 12.0), pos2(divider_x, o.y + 656.0)],
        egui::Stroke::new(1.0, c.group_line),
    );
    if branded {
        // A legible Swoopdl download mark replaces the placeholder dot. It uses the
        // existing SVG icon cache: no new texture, font, animation or repaint timer.
        p.rect_filled(r(o, 15.0, 15.0, 38.0, 39.0), 10.0, c.highlight_hot);
        p.rect_filled(r(o, 15.0, 15.0, 3.0, 39.0), 1.5, c.accent);
        crate::icons::Icon::PageDownloads
            .image(true)
            .paint_at(ui, r(o, 23.0, 23.0, 22.0, 22.0));
        let sparkle = if crate::theme::dark_surface() {
            egui::Color32::from_rgb(101, 208, 202)
        } else {
            egui::Color32::from_rgb(13, 114, 138)
        };
        p.circle_filled(pos2(winui::mx(o.x + 46.0), o.y + 21.0), 3.0, sparkle);
        winui::text_font(
            &p,
            winui::tahoma_bold(),
            o.x + 64.0,
            o.y + 34.0,
            "Swoopdl",
            c.text,
        );
        winui::text_font(
            &p,
            font.clone(),
            o.x + 64.0,
            o.y + 53.0,
            crate::i18n::t("Preferences"),
            c.muted,
        );
    } else {
        winui::text_font(
            &p,
            winui::tahoma_title(),
            o.x + 18.0,
            o.y + 32.0,
            crate::i18n::t("Swoopdl Configuration"),
            c.text,
        );
        winui::text_font(
            &p,
            font.clone(),
            o.x + 18.0,
            o.y + 57.0,
            crate::i18n::t("Preferences"),
            c.muted,
        );
    }
    let mut selected_next = None;
    let mut focus_next = None;
    if ui.memory(|m| m.has_focus(egui::Id::unique(("opt-tab", selected)))) {
        let at = NAV
            .iter()
            .position(|(page, _)| *page == selected)
            .unwrap_or(0) as isize;
        let forward = if winui::rtl() { -1 } else { 1 };
        let to = if winui::take_key(ui, egui::Key::ArrowDown) {
            Some(at + 1)
        } else if winui::take_key(ui, egui::Key::ArrowUp) {
            Some(at - 1)
        } else if winui::take_key(ui, egui::Key::ArrowRight) {
            Some(at + forward)
        } else if winui::take_key(ui, egui::Key::ArrowLeft) {
            Some(at - forward)
        } else if winui::take_key(ui, egui::Key::Home) {
            Some(0)
        } else if winui::take_key(ui, egui::Key::End) {
            Some(NAV.len() as isize - 1)
        } else {
            None
        };
        if let Some(to) = to {
            let next = NAV[to.clamp(0, NAV.len() as isize - 1) as usize].0;
            selected_next = Some(next);
            focus_next = Some(next);
        }
    }
    let active = selected_next.unwrap_or(selected);
    for (index, (page, title)) in NAV.iter().enumerate() {
        let y = o.y + 70.0 + index as f32 * 38.0;
        let rc = winui::rect(Rect::from_min_size(pos2(o.x + 10.0, y), vec2(209.0, 37.0)));
        let response = ui.interact(
            rc,
            egui::Id::unique(("opt-tab", *page)),
            if *page == active {
                Sense::click()
            } else {
                winui::pointer_only()
            },
        );
        winui::expose(
            &response,
            egui::Role::Tab,
            crate::i18n::t(title),
            true,
            Some(*page == active),
        );
        let response = winui::enter_is_not_a_click(ui, response);
        if response.clicked() {
            selected_next = Some(*page);
        }
        if *page == active {
            winui::keep_keys(ui, &response, true, true, false);
        }
        if *page == active || response.hovered() {
            p.rect_filled(
                rc,
                if branded { 9.0 } else { 6.0 },
                if *page == active {
                    c.highlight_hot
                } else {
                    c.subtle_hot
                },
            );
            if branded && *page == active {
                p.rect_stroke(
                    rc,
                    9.0,
                    egui::Stroke::new(1.0, c.toolbar_hot_border),
                    egui::StrokeKind::Inside,
                );
            }
        }
        if *page == active {
            let bar = winui::rect(Rect::from_min_size(
                pos2(o.x + if branded { 12.0 } else { 10.0 }, y + 7.0),
                vec2(3.0, 20.0),
            ));
            p.rect_filled(bar, if branded { 1.5 } else { 2.0 }, c.accent);
        }
        if branded {
            p.rect_filled(
                r(o, 18.0, y - o.y + 5.0, 25.0, 27.0),
                7.0,
                if *page == active {
                    c.subtle_hot
                } else {
                    c.egui_panel
                },
            );
        }
        let icon = match *page {
            0 => Icon::PageGeneral,
            1 => Icon::PageFileTypes,
            2 => Icon::PageSaveTo,
            3 => Icon::PageDownloads,
            4 => Icon::PageConnection,
            5 => Icon::PageProxy,
            6 => Icon::PageLogins,
            8 => Icon::PageSounds,
            9 => Icon::PageRegional,
            10 => Icon::PageArchives,
            REMOTE_PAGE => Icon::PageRemote,
            _ => Icon::PageGeneral,
        };
        let page_icon = icon.image(true);
        if branded {
            page_icon
                .tint(graphite_settings_icon_color(*page, *page == active))
                .paint_at(ui, r(o, 22.0, y - o.y + 10.0, 17.0, 17.0));
        } else {
            page_icon.paint_at(ui, r(o, 20.0, y - o.y + 10.0, 17.0, 17.0));
        }
        let translated = crate::i18n::t(title);
        let clipped = winui::ellipsize(&p, &font, translated, 156.0);
        winui::text_font(
            &p,
            font.clone(),
            o.x + 48.0,
            y + 22.0,
            &clipped,
            if *page == active { c.text } else { c.muted },
        );
        winui::show_focus(ui, &response, rc.shrink(2.0));
    }
    if let Some(next) = focus_next {
        // Register the destination as focusable before transferring focus to it.
        ui.memory_mut(|m| m.request_focus(egui::Id::unique(("opt-tab", next))));
    }
    selected_next
}

/// A row of tabs as shown: IDM's measured tabs in English; a translation's sized by its labels
/// (each label centred, the room left over shared out), so the row keeps its span (x 8..419)
/// and no label is cut.
fn tab_row(
    p: &egui::Painter,
    row: &[(usize, &'static str, f32, f32, f32)],
    measured: bool,
) -> Vec<(usize, &'static str, f32, f32, f32)> {
    // `measured`: the row's English positions are IDM's (a row of ours is sized as a translation).
    if measured && row.iter().all(|t| crate::i18n::t(t.1) == t.1) {
        return row.to_vec();
    }
    let (left, right) = (row[0].2, row[row.len() - 1].3);
    let widths: Vec<f32> = row
        .iter()
        .map(|t| winui::text_width(p, crate::i18n::t(t.1)).ceil())
        .collect();
    let total: f32 = widths.iter().map(|w| w + 16.0).sum();
    let spare = ((right - left - total) / row.len() as f32).floor();
    let padded: Vec<f32> = widths.iter().map(|w| w + 16.0).collect();
    let cap = winui::width_cap(&padded, right - left);
    let mut x0 = left;
    row.iter()
        .zip(&widths)
        .enumerate()
        .map(|(k, (t, w))| {
            let x1 = if k + 1 == row.len() {
                right
            } else if spare < 0.0 {
                // Too wide for the strip: the narrow tabs keep their width, the wide ones share
                // the rest (their labels cut where drawn).
                x0 + (w + 16.0).min(cap)
            } else {
                x0 + w + 16.0 + spare
            };
            let tab = (t.0, t.1, x0, x1, x0 + ((x1 - x0 - w) / 2.0).floor());
            x0 = x1;
            tab
        })
        .collect()
}

/// The two-row tab strip and the page frame; returns the page clicked.
fn tabs(ui: &mut Ui, o: Pos2, page: usize) -> Option<usize> {
    // (page, label, x0, x1, label x): the rows never move sideways, only swap front and back.
    // Remote (#141, ours) joins the first row, which is sized by its labels since.
    const G: [(usize, &str, f32, f32, f32); 6] = [
        // i18n: tab title: general settings
        (0, crate::i18n::mark("General"), 8.0, 82.0, 26.0),
        // i18n: tab title
        (1, crate::i18n::mark("File types"), 82.0, 166.0, 101.0),
        // i18n: tab title: where downloads are saved
        (2, crate::i18n::mark("Save to"), 166.0, 241.0, 185.0),
        // i18n: tab title: download settings
        (3, crate::i18n::mark("Downloads"), 241.0, 330.0, 260.0),
        // i18n: tab title: connection settings
        (4, crate::i18n::mark("Connection"), 330.0, 419.0, 349.0),
        (
            REMOTE_PAGE,
            // i18n: tab title: control SDL from a phone or another computer on the local network
            crate::i18n::mark("Remote"),
            380.0,
            419.0,
            390.0,
        ),
    ];
    // IDM's Dial Up / VPN tab is gone (D-021: no modem support; it did nothing); ours, Regional
    // (how dates and times are written), Accounts (#44, debrid services) and Archives (#37) share
    // the row, sized by their labels.
    const P: [(usize, &str, f32, f32, f32); 6] = [
        // i18n: tab title
        (5, crate::i18n::mark("Proxy / Socks"), 8.0, 104.0, 20.0),
        // i18n: tab title: user names and passwords for sites
        (6, crate::i18n::mark("Sites Logins"), 104.0, 192.0, 116.0),
        // i18n: tab title: the user's debrid service accounts (Real-Debrid…)
        (7, crate::i18n::mark("Accounts"), 192.0, 265.0, 204.0),
        // i18n: tab title: sound settings
        (8, crate::i18n::mark("Sounds"), 265.0, 336.0, 279.0),
        // i18n: tab title: how dates and times are written (date order, 12- or 24-hour clock)
        (9, crate::i18n::mark("Regional"), 336.0, 380.0, 350.0),
        // i18n: tab title: extracting downloaded archives, and Link grabber
        (10, crate::i18n::mark("Archives"), 380.0, 419.0, 390.0),
    ];
    let p = ui.painter().clone();
    let border = winui::HEADER_LINE();
    let fluent = crate::theme::modern_layout();
    let (g, pr) = (tab_row(&p, &G, false), tab_row(&p, &P, false));
    let first_row = page < 5 || page == REMOTE_PAGE;
    let (front, back): (&[_], &[_]) = if first_row { (&g, &pr) } else { (&pr, &g) };
    let mut clicked = None;
    let mut tab = |ui: &mut Ui,
                   (i, label, x0, x1, lx): (usize, &str, f32, f32, f32),
                   top: f32,
                   base: f32,
                   sel: bool| {
        let (x0, x1) = if sel {
            (x0 - if x0 <= 8.0 { 2.0 } else { 1.0 }, x1 + 2.0)
        } else {
            (x0, x1)
        };
        let bottom = if sel { 45.0 } else { top + 17.0 };
        // From the leading edge: the first tab at the right in RTL.
        let rc = winui::rect(Rect::from_min_max(
            o + vec2(x0, top),
            o + vec2(x1 + 1.0, bottom + 1.0),
        ));
        // The strip is one Tab stop, the chosen tab (Windows' tab control); the arrows move.
        let sense = if sel {
            Sense::click()
        } else {
            winui::pointer_only()
        };
        let resp = ui.interact(rc, egui::Id::unique(("opt-tab", i)), sense);
        winui::expose(
            &resp,
            egui::Role::Tab,
            crate::i18n::t(label),
            true,
            Some(sel),
        );
        let resp = winui::enter_is_not_a_click(ui, resp);
        if resp.clicked() && !sel {
            clicked = Some(i);
        }
        if sel {
            winui::keep_keys(ui, &resp, true, false, false);
            if resp.has_focus() {
                // Left / Right (mirrored in RTL), Home / End through the pages in order.
                const ORDER: [usize; 12] = [0, 1, 2, 3, 4, 11, 5, 6, 7, 8, 9, 10];
                let at = ORDER.iter().position(|p| *p == i).unwrap_or(0) as isize;
                let fwd = if winui::rtl() { -1 } else { 1 };
                let to = if winui::take_key(ui, egui::Key::ArrowRight) {
                    Some(at + fwd)
                } else if winui::take_key(ui, egui::Key::ArrowLeft) {
                    Some(at - fwd)
                } else if winui::take_key(ui, egui::Key::Home) {
                    Some(0)
                } else if winui::take_key(ui, egui::Key::End) {
                    Some(ORDER.len() as isize - 1)
                } else {
                    None
                };
                if let Some(to) = to {
                    let next = ORDER[to.clamp(0, ORDER.len() as isize - 1) as usize];
                    clicked = Some(next);
                    // The keys stay on the strip: the new tab takes the focus.
                    ui.memory_mut(|m| m.request_focus(egui::Id::unique(("opt-tab", next))));
                }
            }
        }
        let face = if sel {
            LIGHT_PAGE()
        } else if resp.hovered() {
            winui::TAB_HOT()
        } else {
            winui::TAB_UNSELECTED()
        };
        if fluent {
            // Segmented Fluent navigation: painting only; original tab geometry, ids,
            // keyboard traversal, RTL mapping and page switching are not modified.
            let chip = winui::rect(Rect::from_min_max(
                o + vec2(x0 + 2.0, top + 1.0),
                o + vec2(x1 - 1.0, bottom),
            ));
            p.rect_filled(chip, 5.0, face);
            if sel {
                p.rect_filled(
                    Rect::from_min_max(
                        pos2(chip.left() + 5.0, chip.bottom() - 2.0),
                        pos2(chip.right() - 5.0, chip.bottom()),
                    ),
                    1.0,
                    crate::theme::colors().progress_bar,
                );
            }
        } else {
            fill(&p, o.x + x0, o.y + top, o.x + x1, o.y + bottom, face);
            fill(
                &p,
                o.x + x0 + 1.0,
                o.y + top,
                o.x + x1 - 1.0,
                o.y + top,
                border,
            );
            fill(
                &p,
                o.x + x0,
                o.y + top + 1.0,
                o.x + x0,
                o.y + bottom,
                border,
            );
            fill(
                &p,
                o.x + x1,
                o.y + top + 1.0,
                o.x + x1,
                o.y + bottom,
                border,
            );
        }
        // A translated row too wide for the strip has narrower tabs (`tab_row`): their labels are
        // cut with "..." and shown whole on hover.
        let full = crate::i18n::t(label);
        let shown = winui::ellipsize(&p, &winui::tahoma(), full, x1 - x0 - 6.0);
        if shown == full {
            text(&p, o.x + lx, o.y + base, full, LIGHT_TEXT());
        } else {
            let w = winui::text_width(&p, &shown);
            let lx = x0 + ((x1 - x0 - w) / 2.0).floor();
            let rc = text(&p, o.x + lx, o.y + base, &shown, LIGHT_TEXT());
            winui::full_on_hover(ui, rc, ("opt-tab-label", i), full);
        }
        winui::show_focus(ui, &resp, rc.shrink(2.0));
    };
    for t in back {
        tab(ui, *t, 9.0, 22.0, false);
    }
    for t in front.iter().filter(|t| t.0 != page) {
        tab(ui, *t, 27.0, 40.0, false);
    }
    // The Fluent page is a raised settings surface; legacy themes retain their frames.
    if fluent {
        let card = winui::rect(Rect::from_min_max(
            o + vec2(6.0, 45.0),
            o + vec2(422.0, 469.0),
        ));
        p.rect_filled(card, 9.0, LIGHT_PAGE());
        p.rect_stroke(
            card,
            9.0,
            egui::Stroke::new(1.0, border),
            egui::StrokeKind::Inside,
        );
    } else {
        fill(
            &p,
            o.x + 6.0,
            o.y + 45.0,
            o.x + 421.0,
            o.y + 468.0,
            LIGHT_PAGE(),
        );
        fill(&p, o.x + 6.0, o.y + 45.0, o.x + 421.0, o.y + 45.0, border);
        fill(&p, o.x + 6.0, o.y + 468.0, o.x + 421.0, o.y + 468.0, border);
        fill(&p, o.x + 6.0, o.y + 45.0, o.x + 6.0, o.y + 468.0, border);
        fill(
            &p,
            o.x + 421.0,
            o.y + 45.0,
            o.x + 421.0,
            o.y + 468.0,
            border,
        );
        let shadow = winui::TAB_UNSELECTED();
        fill(
            &p,
            o.x + 422.0,
            o.y + 45.0,
            o.x + 423.0,
            o.y + 469.0,
            shadow,
        );
        fill(&p, o.x + 6.0, o.y + 469.0, o.x + 423.0, o.y + 469.0, shadow);
    }
    if let Some(t) = front.iter().find(|t| t.0 == page) {
        tab(ui, *t, 25.0, 38.0, true);
    }
    clicked
}

/// Page header: the icon, the title (bold, right-aligned as IDM's) and the etched rule.
#[allow(clippy::too_many_arguments)]
fn header(
    ui: &mut Ui,
    o: Pos2,
    icon: Icon,
    title: &str,
    bold: bool,
    ink_x: f32,
    rule_x0: f32,
    rule_x1: f32,
) {
    let p = ui.painter().clone();
    let fluent = crate::theme::modern_layout();
    if crate::theme::current().id.as_ref() == "fluent" {
        let c = crate::theme::colors();
        let panel = r(o, 16.0, 42.0, 660.0, 59.0);
        p.rect_filled(panel, 10.0, c.header);
        p.rect_stroke(
            panel,
            10.0,
            egui::Stroke::new(1.0, c.group_line),
            egui::StrokeKind::Inside,
        );
        p.rect_filled(r(o, 27.0, 52.0, 42.0, 39.0), 9.0, c.highlight_hot);
        icon.image(true)
            .tint(c.accent)
            .paint_at(ui, r(o, 32.0, 55.0, 32.0, 32.0));
        let font = egui::FontId::proportional(22.0);
        let shown = winui::ellipsize(&p, &font, crate::i18n::t(title), 576.0);
        winui::text_font(&p, font, o.x + 80.0, o.y + 77.0, &shown, c.text);
        return;
    }
    if fluent {
        p.rect_filled(
            r(o, 16.0, 51.0, 42.0, 40.0),
            9.0,
            crate::theme::colors().menu_hot,
        );
    }
    icon.image(true).paint_at(ui, r(o, 21.0, 54.0, 32.0, 32.0));
    if fluent {
        let font = egui::FontId::proportional(24.0);
        let shown = winui::ellipsize(&p, &font, crate::i18n::t(title), 610.0);
        winui::text_font(&p, font, o.x + 67.0, o.y + 76.0, &shown, LIGHT_TEXT());
        return;
    }
    let font = if bold {
        winui::tahoma_bold()
    } else if fluent {
        winui::segoe()
    } else {
        winui::tahoma()
    };
    if fluent && !winui::rtl() {
        let caption = crate::i18n::t(title);
        let shown = winui::ellipsize(&p, &font, caption, 338.0);
        winui::text_font(&p, font, o.x + 67.0, o.y + 70.0, &shown, LIGHT_TEXT());
        winui::fill_l(
            &p,
            o.x + 67.0,
            o.y + 83.0,
            o.x + 402.0,
            o.y + 83.0,
            winui::HEADER_LINE(),
        );
    } else {
        page_title(&p, font, o.x + ink_x, o.y + 69.0, title);
        winui::etched_h(&p, o.x + rule_x0, o.y + 78.0, rule_x1 - rule_x0 + 2.0);
    }
}

/// A page title, right-aligned as IDM's: English at its measured pen `x`, a translation ending
/// where the English ends (in RTL: starting there, at the mirrored edge).
fn page_title(p: &egui::Painter, font: egui::FontId, x: f32, baseline: f32, english: &str) {
    let shown = crate::i18n::t(english);
    let x = if shown == english {
        x
    } else {
        let end = x + winui::text_width_font(p, font.clone(), english);
        end - winui::text_width_font(p, font.clone(), shown)
    };
    winui::text_font(p, font, x, baseline, shown, LIGHT_TEXT());
}

/// `english` translated, as [`page_title_ending`] draws it in `room` px: one line where it fits,
/// else two ([`winui::fit_lines`]); and how wide it is drawn.
fn page_title_lines(
    p: &egui::Painter,
    font: egui::FontId,
    english: &str,
    room: f32,
) -> (Vec<String>, f32) {
    let lines = winui::fit_lines(p, font.clone(), crate::i18n::t(english), room);
    let w = lines
        .iter()
        .map(|l| winui::text_width_font(p, font.clone(), l))
        .fold(0.0, f32::max);
    (lines, w)
}

/// A page title's `lines` ([`page_title_lines`]) ending at English x `end` (a control after it
/// needs the room up to there): one line on `baseline`, two around it (beside the page's icon),
/// each starting where the wider one does.
fn page_title_ending(
    p: &egui::Painter,
    font: egui::FontId,
    end: f32,
    baseline: f32,
    lines: &[String],
) {
    let w = lines
        .iter()
        .map(|l| winui::text_width_font(p, font.clone(), l))
        .fold(0.0, f32::max);
    // Bold Tahoma's rows are 15 px apart; two lines share the one line's middle.
    let first = baseline - 7.5 * (lines.len() as f32 - 1.0);
    for (i, line) in lines.iter().enumerate() {
        let y = (first + 15.0 * i as f32).round();
        winui::text_font(p, font.clone(), end - w, y, line, LIGHT_TEXT());
    }
}

/// Group box (`#dcdcdc` frame) with its caption (translated here) at x + 9.
fn group(ui: &Ui, o: Pos2, x: f32, y: f32, w: f32, h: f32, caption: &str) {
    let p = ui.painter();
    let full = crate::i18n::t(caption);
    // A caption longer than the box is cut with "..." before its right edge (whole on hover).
    let caption_width = if crate::theme::modern_layout() {
        w * 1.55 - 18.0
    } else {
        w - 18.0
    };
    let caption = winui::ellipsize(p, &winui::tahoma(), full, caption_width);
    let cw = winui::text_width(p, &caption);
    if crate::theme::modern_layout() {
        let rc = settings_form_rect(o, x, y, w, h);
        p.rect_filled(rc, 8.0, crate::theme::colors().tab_page);
        p.rect_stroke(
            rc,
            7.0,
            egui::Stroke::new(1.0, winui::HEADER_LINE()),
            egui::StrokeKind::Inside,
        );
    } else {
        winui::group_box(
            p,
            r(o, x, y, w, h),
            Some((o.x + x + 7.0, o.x + x + 9.0 + cw + 2.0)),
        );
    }
    let rc = text(
        p,
        settings_form_x(o.x, x + 9.0),
        o.y + y + 4.0,
        &caption,
        LIGHT_TEXT(),
    );
    if caption != full {
        winui::full_on_hover(ui, rc, ("group", caption.as_str()), full);
    }
}

/// Shared modern modal footer with the same Enter, Escape and disabled-OK behavior
/// as `ok_cancel`. Dialogs retain their settings/validation commit paths.
fn modern_modal_actions(ui: &mut Ui, o: Pos2, y: f32, right: f32, valid: bool) -> Outcome<()> {
    let ok = winui::button_light(
        ui,
        r(o, right - 206.0, y, 96.0, 36.0),
        "OK",
        if valid {
            ButtonKind::Accent
        } else {
            ButtonKind::Disabled
        },
        false,
    )
    .clicked();
    let cancel = winui::button_light(
        ui,
        r(o, right - 100.0, y, 96.0, 36.0),
        "Cancel",
        ButtonKind::Normal,
        false,
    )
    .clicked();
    if cancel || winui::escape_pressed(ui) {
        Outcome::Closed
    } else if valid && (ok || winui::enter_pressed(ui)) {
        Outcome::Done(())
    } else {
        Outcome::Open
    }
}

/// Connection → New/Edit exception. Visual layout only: server/number drafts,
/// ranges, and save semantics belong to the original `Sub::Exception` branch.
/// Sites Logins: New/Edit site credential fields. Original validation and commit
/// remain in the existing Sub::Login branch; this changes the modal geometry only.
fn modern_login_dialog(
    ui: &mut Ui,
    o: Pos2,
    site: &mut String,
    user: &mut String,
    pass: &mut String,
) -> Outcome<()> {
    let _font = winui::ModernDialogFont::enter();
    settings_card(ui, o, 18.0, 250.0);
    modern_field_label(ui, o, 60.0, "Site/path:");
    winui::edit_light(ui, r(o, 32.0, 74.0, 496.0, 35.0), site, true, false);
    modern_field_label(ui, o, 128.0, "User:");
    winui::edit_light(ui, r(o, 32.0, 142.0, 496.0, 35.0), user, true, false);
    modern_field_label(ui, o, 197.0, "Password:");
    password(ui, r(o, 32.0, 211.0, 496.0, 35.0), pass, true);
    modern_modal_actions(ui, o, 293.0, 528.0, !site.trim().is_empty())
}

fn modern_exception_dialog(
    ui: &mut Ui,
    o: Pos2,
    server: &mut String,
    number: &mut String,
    limit: u32,
) -> Outcome<()> {
    let _font = winui::ModernDialogFont::enter();
    let c = crate::theme::colors();
    ui.painter()
        .rect_filled(r(o, 16.0, 18.0, 528.0, 178.0), 8.0, c.tab_page);
    modern_field_label(ui, o, 51.0, "Server:");
    winui::edit_light(ui, r(o, 32.0, 65.0, 496.0, 35.0), server, true, false);
    modern_field_label(ui, o, 132.0, "Connections:");
    winui::spin_box(
        ui,
        r(o, 32.0, 146.0, 166.0, 35.0),
        number,
        1,
        u64::from(limit),
        true,
        "opt-exception-number",
    );
    let in_range = number
        .parse::<u32>()
        .is_ok_and(|n| (1..=limit).contains(&n));
    modern_modal_actions(ui, o, 209.0, 532.0, !server.trim().is_empty() && in_range)
}

/// DNS settings modal for Win and Fluent. All DNS mode selection, field edits,
/// fallback, asynchronous connectivity test and validation match the classic form.
fn dns_dialog_modern(ui: &mut Ui, o: Pos2, e: &mut DnsEdit) -> Outcome<()> {
    use sdl_engine::dns::{Mode, Preset};
    let _font = winui::ModernDialogFont::enter();
    let p = ui.painter().clone();
    let c = crate::theme::colors();
    let preset = Preset::by_id(&e.provider);
    p.rect_filled(r(o, 16.0, 16.0, 612.0, 129.0), 8.0, c.tab_page);
    modern_section_title(ui, o, 44.0, "Ask it with:");
    let modes = [
        (1u8, Mode::Plain, "plain DNS"),
        (2, Mode::Tls, "DNS over TLS"),
        (3, Mode::Https, "DNS over HTTPS"),
    ];
    let mut clicked = Vec::new();
    for (row, (n, m, caption)) in modes.iter().enumerate() {
        let offered = preset.is_none_or(|p| p.offers(*m));
        let response = winui::radio_light(
            ui,
            o + vec2(32.0, 56.0 + 27.0 * row as f32),
            e.mode == *n,
            caption,
            offered,
        );
        if response.clicked() && offered {
            e.mode = *n;
        }
        clicked.push(response);
    }
    if let Some(i) = winui::radio_keys(ui, &clicked) {
        let (n, m, _) = modes[i];
        if preset.is_none_or(|p| p.offers(m)) {
            e.mode = n;
        }
    }

    p.rect_filled(r(o, 16.0, 158.0, 612.0, 166.0), 8.0, c.tab_page);
    let field = |ui: &mut Ui, y: f32, label: &str, value: &mut String, enabled: bool| {
        modern_field_label(ui, o, y, label);
        winui::edit_light(ui, r(o, 32.0, y + 12.0, 580.0, 35.0), value, enabled, false);
    };
    let mode = settings::dns_mode(e.mode).unwrap_or(Mode::Https);
    match preset {
        Some(pr) => {
            let shown = match mode {
                Mode::Plain => pr.plain.join(", "),
                Mode::Tls => pr.tls_name.to_owned(),
                Mode::Https => pr.doh.to_owned(),
            };
            modern_field_label(ui, o, 188.0, "Server:");
            readonly(ui, r(o, 32.0, 200.0, 580.0, 35.0), &shown);
            if pr.id == "nextdns" {
                field(
                    ui,
                    265.0,
                    "Configuration ID:",
                    &mut e.nextdns_id,
                    mode != Mode::Plain,
                );
            }
        }
        None => match mode {
            Mode::Plain => field(ui, 188.0, "Servers:", &mut e.servers, true),
            Mode::Tls => {
                field(ui, 188.0, "Servers:", &mut e.servers, true);
                field(ui, 265.0, "Certificate name:", &mut e.tls_name, true);
            }
            Mode::Https => {
                field(ui, 188.0, "Address:", &mut e.doh_url, true);
                field(ui, 265.0, "IP addresses (optional):", &mut e.servers, true);
            }
        },
    }
    p.rect_filled(r(o, 16.0, 336.0, 612.0, 181.0), 8.0, c.tab_page);
    winui::checkbox_light(
        ui,
        o + vec2(32.0, 353.0),
        &mut e.fallback,
        "If it can't be reached, use the system's DNS",
        true,
    );
    winui::paragraph(
        &p,
        o.x + 32.0,
        o.y + 391.0,
        crate::i18n::t(
            "Names in your hosts file and local names (single words, .local, .lan) stay with the system's DNS. Video pages and torrent trackers use the system's DNS for now.",
        ),
        c.muted,
        o.x + 602.0,
        18.0,
    );

    let choice = e.choice();
    let running = e.test.as_ref().is_some_and(|t| t.lock().unwrap().is_none());
    let kind = if choice.is_ok() && !running {
        ButtonKind::Normal
    } else {
        ButtonKind::Disabled
    };
    if winui::button_light(ui, r(o, 32.0, 465.0, 96.0, 35.0), "Test", kind, false).clicked()
        && let Ok(c) = &choice
    {
        let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
        e.test = Some(slot.clone());
        let (c, ctx) = (c.clone(), ui.ctx().clone());
        let route = crate::settings::own_route();
        std::thread::spawn(move || {
            let result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())
                .and_then(|rt| {
                    rt.block_on(sdl_engine::dns::test(&c, "github.com", &[], &route))
                        .map_err(|e| e.to_string())
                });
            let line = match result {
                Ok((took, ips)) => (
                    crate::i18n::fmt(
                        crate::i18n::t("Answered in {ms} ms: {ip}"),
                        &[
                            ("ms", &crate::i18n::integer(took.as_millis() as u64)),
                            (
                                "ip",
                                &ips.first().map(|i| i.to_string()).unwrap_or_default(),
                            ),
                        ],
                    ),
                    true,
                ),
                Err(why) => (why, false),
            };
            *slot.lock().unwrap() = Some(line);
            ctx.request_repaint();
        });
    }
    let status = match (&choice, &e.test) {
        (Err(why), _) => Some((why.clone(), false)),
        (Ok(_), Some(t)) => Some(
            t.lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| (crate::i18n::t("Asking...").to_owned(), true)),
        ),
        _ => None,
    };
    if let Some((line, ok)) = status {
        let color = if ok { LIGHT_TEXT() } else { c.error_strong };
        let (rect, cut) = winui::label_into(
            &p,
            o.x + 143.0,
            o.y + 487.0,
            &line,
            color,
            o.x + 604.0,
            o.x + 143.0,
        );
        if cut {
            winui::full_on_hover(ui, rect, "dns-test", &line);
        }
    }
    modern_modal_actions(ui, o, 537.0, 615.0, choice.is_ok())
}

/// A label (translated here) that IDM greys out with the classic embossed look when its
/// controls are off.
fn disabled_label(p: &egui::Painter, x: f32, base: f32, s: &str, enabled: bool, right: bool) {
    let s = crate::i18n::t(s);
    let x = if right {
        x - winui::text_width(p, s)
    } else {
        x
    };
    winui::label(p, x, base, s, enabled);
}

pub(crate) fn list_frame(p: &egui::Painter, rc: Rect) {
    p.rect_filled(rc, 0.0, winui::FIELD());
    p.rect_stroke(
        rc,
        0.0,
        egui::Stroke::new(1.0, winui::FIELD_EDGE()),
        egui::StrokeKind::Inside,
    );
}

/// A report list (header 25 px, rows 17 px): `cols` are (title, right edge in client x).
/// Returns true on a double click of a row.
#[allow(clippy::too_many_arguments)]
fn report(
    ui: &mut Ui,
    o: Pos2,
    rc: Rect,
    cols: &[(&str, f32)],
    rows: &[Vec<String>],
    st: &mut winui::RowList,
    grid: bool,
    id: &str,
) -> (bool, bool) {
    let p = ui.painter().clone();
    list_frame(&p, rc);
    // Laid out in English, as `cols` are; each part made on screen.
    let rc = winui::rect(rc);
    let left = rc.left();
    let head = rc.top() + 1.0;
    let modern = crate::theme::modern_layout();
    let header_height = if modern { 30.0 } else { 23.0 };
    let row_height = if modern { 28.0 } else { 17.0 };
    let divider = winui::HEADER_LINE();
    fill(
        &p,
        left + 1.0,
        head,
        rc.right() - 2.0,
        head + header_height,
        winui::LIST_HEADER(),
    );
    let mut x0 = left;
    for (i, (title, right)) in cols.iter().enumerate() {
        let title = crate::i18n::t(title);
        let right = o.x + right;
        let tx = if i == 0 { x0 + 8.0 } else { x0 + 7.0 };
        let (rect, cut) = winui::text_fit(
            &p,
            tx,
            head + if modern { 20.0 } else { 16.0 },
            title,
            LIGHT_TEXT(),
            right - 3.0,
        );
        if cut {
            winui::full_on_hover(ui, rect, (id, "head", i), title);
        }
        fill(&p, right, head + 1.0, right, head + header_height, divider);
        x0 = right;
    }
    // The rows: the shared row list under the header (its frame on the list's).
    let body = Rect::from_min_max(
        pos2(rc.left() + 1.0, head + header_height + 2.0),
        pos2(rc.right() - 1.0, rc.bottom() - 1.0),
    );
    let gridc = winui::GRID();
    let ev = winui::row_list(
        ui,
        (id, "rows"),
        winui::rect(body.expand(1.0)),
        rows.len(),
        row_height,
        st,
        |ui, clip, k, line, look| {
            let top = line.top();
            // Read as its cells, joined.
            look.name = rows[k]
                .iter()
                .filter(|c| !c.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
            if grid {
                fill(
                    clip,
                    line.left(),
                    top + row_height - 1.0,
                    line.right(),
                    top + row_height - 1.0,
                    gridc,
                );
            }
            let mut cx = left;
            for (i, (_, right)) in cols.iter().enumerate() {
                let right = o.x + right;
                if grid {
                    fill(
                        clip,
                        right + 1.0,
                        top,
                        right + 1.0,
                        top + row_height - 1.0,
                        gridc,
                    );
                }
                if let Some(cell) = rows[k].get(i) {
                    winui::fit(
                        ui,
                        clip,
                        cx + 6.0,
                        top + if modern { 19.0 } else { 12.0 },
                        cell,
                        look.ink,
                        right - 3.0,
                        (id, "cell", k, i),
                    );
                }
                cx = right;
            }
        },
    );
    if ev.focused {
        st.type_to(ui, id, rows.len(), |i| {
            rows[i].first().cloned().unwrap_or_default()
        });
    }
    // A double-click or Enter edits (Enter taken: not the dialog's OK too); Delete removes.
    if ev.entered.is_some() {
        winui::take_key(ui, egui::Key::Enter);
    }
    (
        ev.double_clicked.or(ev.entered).is_some(),
        ev.delete.is_some(),
    )
}

/// A multi-line edit that wraps its text (the shared one).
fn multiline(ui: &mut Ui, rc: Rect, value: &mut String, id: &str, enabled: bool) {
    winui::multiline_light(ui, rc, value, id, enabled);
}

/// A read-only field (the page colour, as IDM's): the shared one, so it can be copied.
fn readonly(ui: &mut Ui, rc: Rect, value: &str) {
    winui::readonly_field(
        ui,
        rc,
        value,
        LIGHT_PAGE(),
        false,
        ("options", rc.left() as i32, rc.top() as i32),
    );
}

/// [`readonly`] for an address: left to right in every language.
fn readonly_ltr(ui: &mut Ui, rc: Rect, value: &str) {
    winui::readonly_field(
        ui,
        rc,
        value,
        LIGHT_PAGE(),
        true,
        ("options", rc.left() as i32, rc.top() as i32),
    );
}

/// A password edit (characters shown as bullets): the shared one, on the tab page.
fn password(ui: &mut Ui, rc: Rect, value: &mut String, enabled: bool) {
    winui::password_light_on(ui, rc, value, enabled, LIGHT_PAGE());
}

/// Options → Accounts: where the sign-ins (and Sites Logins' passwords, the proxy's) are kept,
/// as `secrets::Vault::kept` found it. None while it is still looking.
fn kept_text(kept: crate::secrets::Kept) -> Option<&'static str> {
    use crate::secrets::Kept;
    Some(match kept {
        Kept::Looking => return None,
        #[cfg(windows)]
        Kept::Keychain => {
            // i18n: Options → Accounts: where the debrid accounts' sign-ins and the other passwords are kept (Windows' own Credential Manager, in Control Panel)
            crate::i18n::mark("Sign-ins and passwords are kept in Windows Credential Manager.")
        }
        #[cfg(target_os = "macos")]
        Kept::Keychain => {
            // i18n: Options → Accounts: where the debrid accounts' sign-ins and the other passwords are kept (the macOS login keychain)
            crate::i18n::mark("Sign-ins and passwords are kept in the macOS keychain.")
        }
        #[cfg(target_os = "linux")]
        // i18n: Options → Accounts: where the debrid accounts' sign-ins and the other passwords are kept (Linux: GNOME Keyring, KeePassXC... through the Secret Service)
        Kept::Keychain => crate::i18n::mark(
            "Sign-ins and passwords are kept in the system keyring (Secret Service).",
        ),
        #[cfg(windows)]
        // i18n: Options → Accounts: a portable copy (it writes nothing outside its folder) keeps the sign-ins in its own settings, encrypted so that only this Windows account can read them
        Kept::Portable => crate::i18n::mark(
            "This portable copy keeps sign-ins and passwords in its own settings, encrypted for your Windows account.",
        ),
        #[cfg(not(windows))]
        // i18n: Options → Accounts: a portable copy (it writes nothing outside its folder) keeps the sign-ins in its own settings folder, which only the user's account may read
        Kept::Portable => crate::i18n::mark(
            "This portable copy keeps sign-ins and passwords in its own settings, which only you can read.",
        ),
        // i18n: Options → Accounts (Linux): the system keyring is locked (SDL never unlocks it: that would ask for its password), so the sign-ins stay in SDL's settings folder, which only the user's account may read
        Kept::Locked => crate::i18n::mark(
            "The system keyring is locked and SDL doesn't unlock it, so sign-ins and passwords are kept in SDL's settings, which only you can read.",
        ),
        #[cfg(windows)]
        // i18n: Options → Accounts: Windows Credential Manager didn't answer, so the sign-ins stay in SDL's settings, encrypted so that only this Windows account can read them
        Kept::Missing => crate::i18n::mark(
            "Windows Credential Manager didn't answer, so sign-ins and passwords are kept in SDL's settings, encrypted for your Windows account.",
        ),
        #[cfg(not(windows))]
        // i18n: Options → Accounts: no system keyring/keychain answered (none runs, e.g. a desktop without one), so the sign-ins stay in SDL's settings folder, which only the user's account may read
        Kept::Missing => crate::i18n::mark(
            "No system keyring answered, so sign-ins and passwords are kept in SDL's settings, which only you can read.",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_settings_pages_have_scroll_room() {
        assert_eq!(modern_page_height(3), 1050.0);
        assert_eq!(modern_page_height(1), 735.0);
        for page in 0..=REMOTE_PAGE {
            assert!(modern_page_height(page) >= 700.0);
        }
    }

    #[test]
    fn reflowed_settings_grid_uses_full_content_width() {
        // The legacy content fits in ~400 points; mapped controls fill the
        // 720-point modern surface while retaining their ordering/hitboxes.
        assert_eq!(settings_scaled_x(16.0), 16.0);
        assert!(settings_scaled_x(21.0) > 20.0);
        assert!(settings_scaled_x(404.0) < 676.0);
        assert!(settings_scaled_x(404.0) > 600.0);
        assert!((settings_scaled_x(400.0) - settings_scaled_x(30.0) - 370.0 * 1.55).abs() < 0.01);
        assert!(modern_page_height(0) > 1020.0);
    }

    #[test]
    fn fluent_navigation_keeps_arrow_keys_and_focus() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        winui::install_fonts(&mut fonts, None);
        ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&ctx);
        let mut selected = 0;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            fluent_options_nav(ui, pos2(0.0, 0.0), selected);
            ui.memory_mut(|m| m.request_focus(egui::Id::unique(("opt-tab", selected))));
        });
        output.textures_delta.clear();
        for (key, expected) in [
            (egui::Key::ArrowDown, 1),
            (egui::Key::End, 10),
            (egui::Key::Home, 0),
        ] {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
            let mut output = ctx.run_ui(input, |ui| {
                if let Some(page) = fluent_options_nav(ui, pos2(0.0, 0.0), selected) {
                    selected = page;
                }
            });
            output.textures_delta.clear();
            assert_eq!(selected, expected);
            assert_eq!(
                ctx.memory(|m| m.focused()),
                Some(egui::Id::unique(("opt-tab", expected)))
            );
        }
    }

    /// #424: the list written and read back through a real file: nothing lost, nothing doubled.
    #[test]
    fn sites_round_trip_through_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sites.txt");
        let sites = "a.test *.b.test";
        std::fs::write(&file, sdl_ipc::capture::sites_to_text(sites)).unwrap();
        let text = read_sites_file(&file).unwrap();
        assert_eq!(text, "a.test\n*.b.test\n");
        let (back, added) = sdl_ipc::capture::merge_sites("", &text);
        assert_eq!((back.as_str(), added), (sites, 2));
        assert_eq!(sdl_ipc::capture::merge_sites(sites, &text).1, 0);
    }
}

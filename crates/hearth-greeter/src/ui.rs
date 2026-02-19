//! GTK4 fullscreen greeter UI.
//!
//! The UI is a single fullscreen window with a centered login form, branding
//! area, and a progress/status section. Communication between the async
//! orchestration layer and the GTK main loop uses `async_channel` bridged
//! through `glib::MainContext::spawn_local`.

use gtk4::gdk;
use gtk4::prelude::*;
use hearth_common::config::BrandingConfig;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

/// Messages sent from GTK callbacks to the async orchestration layer.
#[derive(Debug, Clone)]
pub enum UiAction {
    /// User clicked the login button.
    LoginClicked { username: String, password: String },
    /// User clicked the fallback session button.
    FallbackClicked,
}

/// Messages sent from the async orchestration layer to update the UI.
#[derive(Debug, Clone)]
pub enum UiUpdate {
    /// Authentication succeeded — move to environment prep phase.
    AuthSuccess,
    /// Authentication failed with an error message.
    AuthFailed(String),
    /// Agent environment preparation progress.
    PrepProgress { percent: u8, message: String },
    /// Environment is ready; session is being started.
    PrepReady,
    /// Environment preparation failed.
    PrepError(String),
}

// ---------------------------------------------------------------------------
// Widget handles — lets the orchestrator push updates into the UI
// ---------------------------------------------------------------------------

/// Holds references to the widgets we need to update from async code.
struct Widgets {
    username_entry: gtk4::Entry,
    password_entry: gtk4::PasswordEntry,
    login_button: gtk4::Button,
    status_label: gtk4::Label,
    progress_bar: gtk4::ProgressBar,
    progress_box: gtk4::Box,
    fallback_button: gtk4::Button,
    error_label: gtk4::Label,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load custom CSS from a file into the default display.
pub fn load_css(css_path: &str) {
    let path = Path::new(css_path);
    if !path.exists() {
        tracing::warn!(path = %css_path, "CSS file not found, skipping");
        return;
    }

    let provider = gtk4::CssProvider::new();
    provider.load_from_path(css_path);

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Load the built-in base CSS (dark theme defaults).
fn load_base_css() {
    let css = r#"
        window {
            background-color: #141726;
        }
        .branding-name {
            font-size: 28px;
            font-weight: 700;
            color: #e2e8f0;
            margin-bottom: 8px;
        }
        .login-card {
            background-color: #1a1f36;
            border-radius: 16px;
            padding: 40px;
            margin: 16px;
        }
        .login-title {
            font-size: 18px;
            font-weight: 600;
            color: #e2e8f0;
            margin-bottom: 24px;
        }
        entry, .password-entry {
            background-color: #232946;
            color: #e2e8f0;
            border: 1px solid #394068;
            border-radius: 8px;
            padding: 10px 14px;
            font-size: 15px;
            min-height: 24px;
        }
        entry:focus, .password-entry:focus-within {
            border-color: #e94560;
            outline-color: #e94560;
        }
        .login-button {
            background-color: #e94560;
            color: #ffffff;
            border-radius: 8px;
            padding: 10px 24px;
            font-size: 15px;
            font-weight: 600;
            min-height: 24px;
        }
        .login-button:hover {
            background-color: #d63851;
        }
        .login-button:disabled {
            opacity: 0.5;
        }
        .status-label {
            color: #94a3b8;
            font-size: 14px;
        }
        .error-label {
            color: #f87171;
            font-size: 14px;
        }
        .fallback-button {
            background-color: transparent;
            color: #94a3b8;
            border: 1px solid #394068;
            border-radius: 8px;
            padding: 8px 16px;
            font-size: 13px;
        }
        .fallback-button:hover {
            border-color: #e94560;
            color: #e2e8f0;
        }
        progressbar trough {
            background-color: #232946;
            border-radius: 4px;
            min-height: 8px;
        }
        progressbar progress {
            background-color: #e94560;
            border-radius: 4px;
            min-height: 8px;
        }
    "#;

    let provider = gtk4::CssProvider::new();
    provider.load_from_string(css);

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION - 1,
        );
    }
}

/// Build the greeter UI and return:
/// - An `async_channel::Sender<UiUpdate>` that the async layer uses to push state changes.
/// - A `tokio::sync::mpsc::Receiver<UiAction>` for the async layer to receive user actions.
///
/// This function must be called from the GTK main thread inside `Application::connect_activate`.
pub fn build_ui(
    app: &gtk4::Application,
    branding: &BrandingConfig,
) -> (
    async_channel::Sender<UiUpdate>,
    tokio::sync::mpsc::Receiver<UiAction>,
) {
    // Load CSS.
    load_base_css();
    if let Some(css_path) = &branding.css_path {
        load_css(css_path);
    }

    // Channel from GTK callbacks -> async orchestrator.
    let (action_tx, action_rx) = tokio::sync::mpsc::channel::<UiAction>(16);

    // Channel from async orchestrator -> GTK main loop.
    // Using async-channel since glib 0.20 removed MainContext::channel().
    let (update_tx, update_rx) = async_channel::unbounded::<UiUpdate>();

    // --- Window ---
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("Hearth Login")
        .fullscreened(true)
        .decorated(false)
        .build();

    // Force dark color scheme.
    if let Some(display) = gdk::Display::default() {
        let settings = gtk4::Settings::for_display(&display);
        settings.set_gtk_application_prefer_dark_theme(true);
    }

    // --- Outer layout: center everything vertically and horizontally ---
    let outer_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    outer_box.set_halign(gtk4::Align::Center);
    outer_box.set_valign(gtk4::Align::Center);
    outer_box.set_width_request(420);

    // --- Branding ---
    let branding_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    branding_box.set_halign(gtk4::Align::Center);
    branding_box.set_margin_bottom(32);

    // Optional logo.
    if let Some(logo_path) = &branding.logo_path
        && Path::new(logo_path).exists()
    {
        let logo = gtk4::Picture::for_filename(logo_path);
        logo.set_content_fit(gtk4::ContentFit::Contain);
        logo.set_width_request(96);
        logo.set_height_request(96);
        logo.set_halign(gtk4::Align::Center);
        branding_box.append(&logo);
    }

    let org_label = gtk4::Label::new(Some(&branding.organization_name));
    org_label.add_css_class("branding-name");
    branding_box.append(&org_label);

    outer_box.append(&branding_box);

    // --- Login card ---
    let card = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
    card.add_css_class("login-card");

    let title = gtk4::Label::new(Some("Sign in"));
    title.add_css_class("login-title");
    title.set_halign(gtk4::Align::Start);
    card.append(&title);

    // Username entry.
    let username_entry = gtk4::Entry::new();
    username_entry.set_placeholder_text(Some("Username"));
    username_entry.set_hexpand(true);
    card.append(&username_entry);

    // Password entry.
    let password_entry = gtk4::PasswordEntry::new();
    password_entry.set_placeholder_text(Some("Password"));
    password_entry.set_show_peek_icon(true);
    password_entry.set_hexpand(true);
    password_entry.add_css_class("password-entry");
    card.append(&password_entry);

    // Login button.
    let login_button = gtk4::Button::with_label("Sign in");
    login_button.add_css_class("login-button");
    card.append(&login_button);

    // Error label (hidden initially).
    let error_label = gtk4::Label::new(None);
    error_label.add_css_class("error-label");
    error_label.set_wrap(true);
    error_label.set_halign(gtk4::Align::Start);
    error_label.set_visible(false);
    card.append(&error_label);

    // --- Progress section (hidden initially) ---
    let progress_box = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
    progress_box.set_visible(false);

    let status_label = gtk4::Label::new(Some("Preparing environment..."));
    status_label.add_css_class("status-label");
    status_label.set_halign(gtk4::Align::Start);
    progress_box.append(&status_label);

    let progress_bar = gtk4::ProgressBar::new();
    progress_bar.set_fraction(0.0);
    progress_box.append(&progress_bar);

    card.append(&progress_box);

    // Fallback button (hidden initially).
    let fallback_button = gtk4::Button::with_label("Use fallback session");
    fallback_button.add_css_class("fallback-button");
    fallback_button.set_visible(false);
    card.append(&fallback_button);

    outer_box.append(&card);
    window.set_child(Some(&outer_box));

    // --- Widget refs for update handler ---
    let widgets = Rc::new(RefCell::new(Widgets {
        username_entry: username_entry.clone(),
        password_entry: password_entry.clone(),
        login_button: login_button.clone(),
        status_label,
        progress_bar,
        progress_box,
        fallback_button: fallback_button.clone(),
        error_label,
    }));

    // --- Login button click ---
    {
        let tx = action_tx.clone();
        let uentry = username_entry.clone();
        let pentry = password_entry.clone();
        login_button.connect_clicked(move |_| {
            let username = uentry.text().to_string();
            let password = pentry.text().to_string();
            if username.is_empty() {
                return;
            }
            let _ = tx.try_send(UiAction::LoginClicked { username, password });
        });
    }

    // Allow Enter in password field to trigger login.
    {
        let btn = login_button.clone();
        password_entry.connect_activate(move |_| {
            btn.emit_clicked();
        });
    }

    // Allow Enter in username field to move focus to password.
    {
        let pw = password_entry.clone();
        username_entry.connect_activate(move |_| {
            pw.grab_focus();
        });
    }

    // --- Fallback button click ---
    {
        let tx = action_tx;
        fallback_button.connect_clicked(move |_| {
            let _ = tx.try_send(UiAction::FallbackClicked);
        });
    }

    // --- Handle updates from async layer via glib::spawn_local ---
    {
        let w = widgets;
        glib::spawn_future_local(async move {
            while let Ok(msg) = update_rx.recv().await {
                let w = w.borrow();
                match msg {
                    UiUpdate::AuthSuccess => {
                        // Disable inputs, show progress area.
                        w.login_button.set_sensitive(false);
                        w.username_entry.set_sensitive(false);
                        w.password_entry.set_sensitive(false);
                        w.error_label.set_visible(false);
                        w.progress_box.set_visible(true);
                        w.status_label
                            .set_text("Authenticating... preparing your environment.");
                        w.progress_bar.set_fraction(0.0);
                    }
                    UiUpdate::AuthFailed(ref msg) => {
                        w.login_button.set_sensitive(true);
                        w.username_entry.set_sensitive(true);
                        w.password_entry.set_sensitive(true);
                        w.password_entry.set_text("");
                        w.error_label.set_text(msg);
                        w.error_label.set_visible(true);
                        w.progress_box.set_visible(false);
                        w.fallback_button.set_visible(false);
                    }
                    UiUpdate::PrepProgress {
                        percent,
                        ref message,
                    } => {
                        w.progress_box.set_visible(true);
                        w.progress_bar.set_fraction(f64::from(percent) / 100.0);
                        w.status_label.set_text(message);
                    }
                    UiUpdate::PrepReady => {
                        w.progress_bar.set_fraction(1.0);
                        w.status_label
                            .set_text("Environment ready. Starting session...");
                    }
                    UiUpdate::PrepError(ref msg) => {
                        w.status_label.set_text(&format!("Error: {msg}"));
                        w.error_label.set_text(msg);
                        w.error_label.set_visible(true);
                        w.fallback_button.set_visible(true);
                        w.progress_bar.set_fraction(0.0);
                    }
                }
            }
        });
    }

    window.present();

    // Focus the username entry on startup.
    username_entry.grab_focus();

    (update_tx, action_rx)
}

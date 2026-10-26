//! Globaler Scout-Mode-Hotkey über das XDG-Portal "GlobalShortcuts" (KDE Plasma, GNOME 48+).
//! Die Taste wird nicht von uns abgegriffen, sondern in den Systemeinstellungen festgelegt;
//! der Desktop meldet dort selbst Kollisionen mit anderen Kürzeln.

use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
use ashpd::desktop::CreateSessionOptions;
use futures_lite::{future::block_on, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const SHORTCUT_ID: &str = "scout-mode-toggle";

/// Startet den Portal-Thread. `enabled` schaltet die Reaktion ein/aus, ohne die Sitzung neu aufzubauen.
pub fn spawn(enabled: Arc<AtomicBool>, on_activated: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        if let Err(e) = block_on(run(enabled, on_activated)) {
            eprintln!("Globaler Hotkey nicht verfügbar: {e}");
        }
    });
}

async fn run(enabled: Arc<AtomicBool>, on_activated: impl Fn()) -> ashpd::Result<()> {
    let proxy = GlobalShortcuts::new().await?;
    let session = proxy.create_session(CreateSessionOptions::default()).await?;
    // KDE ignoriert Tastenvorschläge; die Taste vergibt der Nutzer in den Systemeinstellungen (und bestätigt mit "Anwenden").
    let shortcut = NewShortcut::new(SHORTCUT_ID, "Scout Mode umschalten");
    let bound = proxy
        .bind_shortcuts(&session, &[shortcut], None, BindShortcutsOptions::default())
        .await?
        .response()?;
    for s in bound.shortcuts() {
        eprintln!("Globaler Hotkey registriert: {} ({})", s.description(), s.trigger_description());
    }
    let mut activations = proxy.receive_activated().await?;
    while let Some(activation) = activations.next().await {
        if activation.shortcut_id() == SHORTCUT_ID && enabled.load(Ordering::Relaxed) {
            on_activated();
        }
    }
    drop(session);
    Ok(())
}

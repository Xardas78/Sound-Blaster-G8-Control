fn main() {
    // Übersetzungen (.po) werden beim Bauen eingebettet: ohne diese Zeile erkennt Cargo geänderte
    // Sprachdateien nicht und baut mit veralteten Texten.
    println!("cargo:rerun-if-changed=lang");
    // Übersetzungsdomäne ist automatisch CARGO_PKG_NAME ("g8-control"),
    // siehe slint-build-Quelltext. Erwartet rohe .po-Dateien (kein
    // separates msgfmt/.mo nötig) unter lang/<sprache>/LC_MESSAGES/
    // g8-control.po, relativ zu CARGO_MANIFEST_DIR.
    let config = slint_build::CompilerConfiguration::new().with_bundled_translations("lang");
    slint_build::compile_with_config("ui/app-window.slint", config)
        .expect("Slint-Build fehlgeschlagen");
}

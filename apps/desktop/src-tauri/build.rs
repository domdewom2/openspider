fn main() {
    // `tauri::generate_context!` verlangt, dass `frontendDist` existiert. Damit
    // `cargo check/clippy/test --workspace` auch ohne vorherigen Frontend-Build
    // funktionieren, wird das (gitignorierte) Verzeichnis bei Bedarf angelegt.
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist");
    let _ = std::fs::create_dir_all(dist);
    tauri_build::build()
}

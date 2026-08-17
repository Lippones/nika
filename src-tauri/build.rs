fn main() {
    // O sidecar do Tor é instalado com sufixo de target triple durante `tauri dev`
    // (`tor-x86_64-pc-windows-msvc.exe`). Precisamos do triple em runtime para
    // localizar o binário — ver `paths::tor_binary`.
    let target = std::env::var("TARGET").expect("cargo define TARGET");
    println!("cargo:rustc-env=TARGET_TRIPLE={target}");

    tauri_build::build();
}

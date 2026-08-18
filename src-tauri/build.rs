fn main() {
    // O sidecar do Tor é instalado com sufixo de target triple durante `tauri dev`
    // (`tor-x86_64-pc-windows-msvc.exe`). Precisamos do triple em runtime para
    // localizar o binário — ver `paths::tor_binary`.
    let target = std::env::var("TARGET").expect("cargo define TARGET");
    println!("cargo:rustc-env=TARGET_TRIPLE={target}");

    // O `tauri_build` embute `icons/icon.ico` no executável, mas só pede rebuild
    // quando `tauri.conf.json` muda — trocar a arte do ícone deixava o binário
    // com o ícone antigo até um `cargo clean`. Aqui a dependência fica explícita.
    println!("cargo:rerun-if-changed=icons/icon.ico");

    tauri_build::build();
}

fn main() {
    // O `tauri_build` embute `icons/icon.ico` no executável, mas só pede rebuild
    // quando `tauri.conf.json` muda — trocar a arte do ícone deixava o binário
    // com o ícone antigo até um `cargo clean`. Aqui a dependência fica explícita.
    println!("cargo:rerun-if-changed=icons/icon.ico");

    tauri_build::build();
}

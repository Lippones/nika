# nika-discord-shim (`version.dll`)

A `version.dll` que o Nika instala ao lado do `Discord.exe` para fazer o tráfego
TCP do Discord passar pelo proxy Tor. Spec: [../../docs/discord-dll.md](../../docs/discord-dll.md).

Reexporta as 17 funções do `version.dll` do sistema e instala dois hooks inline
(`GetCommandLineW`, `GetEnvironmentVariableW`) que injetam `--proxy-server` e
`http_proxy` a partir de um `nika-proxy.ini` gravado ao lado da DLL.

## Compilar (no Windows, MSVC)

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

O artefato sai como `target/x86_64-pc-windows-msvc/release/version.dll`.
`scripts/build-shim.ps1` (na raiz) compila e copia para
`src-tauri/resources/discord/version.dll`, de onde o app o empacota e instala.

## MSRV

**1.88** — usa `#[unsafe(naked)]` + `naked_asm!`, estáveis desde 1.88.0. O app
principal segue em 1.77; só este crate sobe.

## Validação

`cargo check --target x86_64-pc-windows-msvc` type-checa tudo. As funções puras
(`parse_proxy`, `patch_command_line`) têm testes. **O comportamento real — hookar
dentro do Discord sem derrubar o processo — só o teste no Windows prova**; ver os
gates em [../../docs/discord-dll.md](../../docs/discord-dll.md) §12. Este crate é
Windows-only e propositalmente fora do workspace do app (o `[workspace]` vazio no
`Cargo.toml` o isola), para o `cargo` de `src-tauri` nunca tentar compilá-lo junto.

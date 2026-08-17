/**
 * Sobe a versão nos quatro lugares que precisam concordar. Errar um deles gera
 * um instalador com número diferente do que o app relata — e ninguém percebe.
 *
 *   npm run bump 0.2.0
 */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = process.argv[2];

if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error("uso: npm run bump <maior.menor.correção>   (ex.: npm run bump 0.2.0)");
  process.exit(1);
}

/** Substitui exatamente uma ocorrência, ou falha — nunca em silêncio. */
function patch(relativePath, pattern, replacement) {
  const path = join(root, relativePath);
  const before = readFileSync(path, "utf8");
  const matches = before.match(pattern);

  if (!matches || matches.length !== 1) {
    console.error(`${relativePath}: esperava 1 ocorrência de ${pattern}, achei ${matches?.length ?? 0}`);
    process.exit(1);
  }

  writeFileSync(path, before.replace(pattern, replacement));
  console.log(`  ${relativePath}`);
}

console.log(`versão ${version} em:`);

patch("package.json", /(?<=^  "version": ")[^"]+(?=")/m, version);
patch("src-tauri/tauri.conf.json", /(?<=^  "version": ")[^"]+(?=")/m, version);
patch("src-tauri/Cargo.toml", /(?<=^version = ")[^"]+(?=")/m, version);
// O Cargo.lock também registra a versão do próprio crate.
patch("src-tauri/Cargo.lock", /(?<=name = "nika"\nversion = ")[^"]+(?=")/, version);

console.log(`\nagora:  git commit -am "v${version}" && git tag v${version} && git push --follow-tags`);

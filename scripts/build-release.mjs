// Build de release assinado + latest.json do atualizador automático.
//
//   npm run release:build -- v0.4.0
//
// A chave privada de atualização NUNCA fica no repositório: é lida de
// TORII_UPDATER_KEY (caminho do arquivo) ou de ~/.tauri/torii-updater.key.
// Sem ela, quem já tem o Torii instalado não aceita a atualização.
//
// Saída em src-tauri/target/release/bundle/nsis/: o instalador, o .sig e o
// latest.json — os dois primeiros e o latest.json vão como assets da release
// no GitHub (release normal, não pré-lançamento: o link
// releases/latest/download/latest.json ignora pré-lançamentos).

import { execSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const REPO = "Fokezato/torii";

const tag = process.argv[2];
if (!tag || !/^v\d+\.\d+\.\d+/.test(tag)) {
  console.error("uso: npm run release:build -- v0.4.0");
  process.exit(1);
}

const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const version = conf.version;
if (!tag.startsWith(`v${version}`)) {
  console.error(`a tag ${tag} não bate com a versão ${version} do tauri.conf.json`);
  process.exit(1);
}

const keyPath = process.env.TORII_UPDATER_KEY ?? join(homedir(), ".tauri", "torii-updater.key");
if (!existsSync(keyPath)) {
  console.error(`chave de atualização não encontrada em ${keyPath}`);
  process.exit(1);
}

execSync("npx tauri build --bundles nsis", {
  stdio: "inherit",
  env: {
    ...process.env,
    TAURI_SIGNING_PRIVATE_KEY: readFileSync(keyPath, "utf8"),
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "",
  },
});

const dir = "src-tauri/target/release/bundle/nsis";
const installer = `${conf.productName}_${version}_x64-setup.exe`;
const signature = readFileSync(join(dir, `${installer}.sig`), "utf8").trim();

const latest = {
  version,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature,
      url: `https://github.com/${REPO}/releases/download/${tag}/${installer}`,
    },
  },
};
writeFileSync(join(dir, "latest.json"), JSON.stringify(latest, null, 2) + "\n");
console.log(`\nPronto: ${join(dir, installer)} + .sig + latest.json`);

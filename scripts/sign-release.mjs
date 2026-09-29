// Publica um release compilado pelo GitHub Actions (.github/workflows/build.yml):
//
//   npm run release:sign -- v0.5.0
//
// Baixa os instaladores do release RASCUNHO da tag, assina localmente os
// que o atualizador usa (instalador do Windows e AppImage do Linux), gera o
// latest.json com as duas plataformas, sobe as assinaturas e publica o
// release como normal (não pré-lançamento — o link
// releases/latest/download/latest.json ignora pré-lançamentos).
//
// A chave privada fica só nesta máquina: TORII_UPDATER_KEY (caminho) ou
// ~/.tauri/torii-updater.key. Senha em TAURI_SIGNING_PRIVATE_KEY_PASSWORD.

import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";

const REPO = "Fokezato/torii";

const tag = process.argv[2];
if (!tag || !/^v\d+\.\d+\.\d+/.test(tag)) {
  console.error("uso: npm run release:sign -- v0.5.0");
  process.exit(1);
}
const version = tag.slice(1);

const keyPath = process.env.TORII_UPDATER_KEY ?? join(homedir(), ".tauri", "torii-updater.key");
if (!existsSync(keyPath)) {
  console.error(`chave de atualização não encontrada em ${keyPath}`);
  process.exit(1);
}

const gh = (...args) => execFileSync("gh", args, { stdio: ["ignore", "pipe", "inherit"] }).toString();
const shell = process.platform === "win32";

const dir = mkdtempSync(join(tmpdir(), "torii-release-"));
console.log(`baixando os instaladores de ${tag} em ${dir}…`);
gh("release", "download", tag, "-R", REPO, "-D", dir, "-p", "*.exe", "-p", "*.AppImage");

const files = readdirSync(dir);
const find = (suffix) => {
  const name = files.find((f) => f.endsWith(suffix));
  if (!name) {
    console.error(`nenhum *${suffix} no release ${tag} — o build do GitHub Actions terminou?`);
    process.exit(1);
  }
  return name;
};
const targets = {
  "windows-x86_64": find("-setup.exe"),
  "linux-x86_64": find(".AppImage"),
};

const platforms = {};
for (const [platform, name] of Object.entries(targets)) {
  const file = join(dir, name);
  // Chave e senha por variável de ambiente: no Windows (shell) um argumento
  // vazio — senha em branco — some da linha de comando.
  execFileSync("npx", ["tauri", "signer", "sign", file], {
    stdio: "inherit",
    shell,
    env: {
      ...process.env,
      TAURI_SIGNING_PRIVATE_KEY_PATH: keyPath,
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "",
    },
  });
  platforms[platform] = {
    signature: readFileSync(`${file}.sig`, "utf8").trim(),
    url: `https://github.com/${REPO}/releases/download/${tag}/${encodeURIComponent(name)}`,
  };
}

const latest = { version, pub_date: new Date().toISOString(), platforms };
const latestPath = join(dir, "latest.json");
writeFileSync(latestPath, JSON.stringify(latest, null, 2) + "\n");

const uploads = [...Object.values(targets).map((n) => join(dir, `${n}.sig`)), latestPath];
gh("release", "upload", tag, "-R", REPO, "--clobber", ...uploads);
gh("release", "edit", tag, "-R", REPO, "--draft=false", "--prerelease=false", "--latest");
console.log(`\nPublicado: https://github.com/${REPO}/releases/tag/${tag}`);

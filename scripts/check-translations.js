import { readFileSync, readdirSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const LOCALES_DIR = resolve(__dirname, "../src/locales");

function flattenKeys(obj, prefix = "") {
  let keys = [];
  for (const [key, value] of Object.entries(obj)) {
    const fullKey = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      keys = keys.concat(flattenKeys(value, fullKey));
    } else {
      keys.push({ key: fullKey, value });
    }
  }
  return keys;
}

function readJSON(filePath) {
  return JSON.parse(readFileSync(filePath, "utf-8"));
}

function main() {
  const files = readdirSync(LOCALES_DIR).filter((f) => f.endsWith(".json"));
  const enPath = resolve(LOCALES_DIR, "en.json");
  const en = readJSON(enPath);
  const enKeys = flattenKeys(en);

  const localeKeys = {};
  for (const file of files) {
    const path = resolve(LOCALES_DIR, file);
    if (file === "en.json") continue;
    const locale = file.replace(".json", "");
    const data = readJSON(path);
    localeKeys[locale] = flattenKeys(data);
  }

  let hasErrors = false;

  for (const [locale, keys] of Object.entries(localeKeys)) {
    const keyMap = new Map(keys.map((k) => [k.key, k.value]));
    const missing = [];

    for (const { key, value: enValue } of enKeys) {
      if (!keyMap.has(key)) {
        missing.push(key);
      } else {
        const val = keyMap.get(key);
        if (val === "" || val === null || val === undefined) {
          missing.push(`${key} (empty/null value)`);
        }
      }
    }

    const extra = keys
      .filter((k) => !enKeys.some((ek) => ek.key === k.key))
      .map((k) => k.key);

    if (missing.length > 0) {
      hasErrors = true;
      console.error(`\n\x1b[31m✗ ${locale}.json is missing ${missing.length} translation(s):\x1b[0m`);
      for (const k of missing) {
        console.error(`   - ${k}`);
      }
    }

    if (extra.length > 0) {
      console.warn(`\n\x1b[33m⚠ ${locale}.json has ${extra.length} unused key(s) (not in en.json):\x1b[0m`);
      for (const k of extra) {
        console.warn(`   - ${k}`);
      }
    }

    if (missing.length === 0 && extra.length === 0) {
      console.log(`\x1b[32m✓ ${locale}.json is complete\x1b[0m`);
    }
  }

  if (hasErrors) {
    console.error("\n\x1b[31mERROR: Some locale files are missing translations. Add them before releasing.\x1b[0m");
    process.exit(1);
  }

  console.log("\n\x1b[32mAll locale files are complete!\x1b[0m");
}

main();

/**
 * Siegu landing — translation completeness checker.
 * Ensures every other locale has all (and non-empty) keys present in en.json.
 * Usage: node scripts/check-translations.js
 */
import { readFile } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const LOCALES = join(__dirname, '..', 'public', 'locales');
const SUPPORTED = ['en', 'nl', 'fr', 'es', 'pap', 'de', 'it', 'pt'];

function flatten(obj, prefix = '', out = {}) {
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) flatten(v, key, out);
    else out[key] = v;
  }
  return out;
}

const enRaw = JSON.parse(await readFile(join(LOCALES, 'en.json'), 'utf8'));
const en = flatten(enRaw);
let failed = false;

for (const code of SUPPORTED) {
  if (code === 'en') continue;
  const raw = JSON.parse(await readFile(join(LOCALES, `${code}.json`), 'utf8'));
  const dict = flatten(raw);
  for (const [key, val] of Object.entries(en)) {
    const other = dict[key];
    if (other === undefined) {
      console.error(`✗ [${code}] missing key "${key}"`);
      failed = true;
    } else if (typeof val === 'string' && String(other).trim() === '') {
      console.error(`✗ [${code}] empty value for "${key}"`);
      failed = true;
    }
  }
}

if (failed) {
  console.error('\nTranslation check FAILED.');
  process.exit(1);
}
console.log(`✓ Translations OK (${SUPPORTED.length} locales, ${Object.keys(en).length} keys).`);

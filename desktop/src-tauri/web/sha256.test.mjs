// Checks the browser companion's SHA-256 against Node's: `node desktop/src-tauri/web/sha256.test.mjs`
// (also run by the Rust test `web_sha256_matches_node` when Node is available).
import { createHash, randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const source = readFileSync(fileURLToPath(new URL("./sha256.js", import.meta.url)), "utf8");
const sandbox = { Uint8Array, Uint32Array, Math };
sandbox.globalThis = sandbox;
vm.runInNewContext(source, sandbox);
const { Sha256 } = sandbox;

let checked = 0;
for (const size of [0, 1, 55, 56, 63, 64, 65, 119, 120, 1000, 192 * 1024 + 7, 3 * 1024 * 1024]) {
  const data = randomBytes(size);
  const expected = createHash("sha256").update(data).digest("hex");
  // Feed in uneven pieces, like file chunks.
  const hasher = new Sha256();
  for (let i = 0; i < data.length; ) {
    const n = Math.min(data.length - i, 1 + ((i * 7919) % 70001));
    hasher.update(new Uint8Array(data.buffer, data.byteOffset + i, n));
    i += n;
  }
  const got = hasher.hex();
  if (got !== expected) {
    console.error(`✗ size ${size}: ${got} != ${expected}`);
    process.exit(1);
  }
  checked++;
}
console.log(`✓ sha256.js matches Node for ${checked} sizes`);

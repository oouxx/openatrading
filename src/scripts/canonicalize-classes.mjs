/**
 * canonicalize-classes.mjs
 *
 * 扫描 src/ 下所有 .tsx / .ts 文件，提取 className="..." 中的类名，
 * 通过 `tailwindcss canonicalize` 转换为 canonical 格式并写回文件。
 *
 * 用法: bun run canonicalize
 */

import { execSync } from "child_process";
import { readFileSync, writeFileSync, readdirSync } from "fs";
import { join } from "path";

const SRC_DIR = new URL("../src", import.meta.url).pathname;
const CWD = new URL("..", import.meta.url).pathname;

function findFiles(dir) {
  const results = [];
  const entries = readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory() && !entry.name.startsWith(".") && entry.name !== "node_modules" && entry.name !== "dist") {
      results.push(...findFiles(fullPath));
    } else if (entry.isFile() && (entry.name.endsWith(".tsx") || entry.name.endsWith(".ts")) && !entry.name.endsWith(".d.ts")) {
      results.push(fullPath);
    }
  }
  return results;
}

// Collect all unique className strings
const allClassStrings = new Map();

for (const file of findFiles(SRC_DIR)) {
  const content = readFileSync(file, "utf-8");
  const regex = /className="([^"]*)"/g;
  let match;
  while ((match = regex.exec(content)) !== null) {
    const s = match[1].trim();
    if (!s) continue;
    if (!allClassStrings.has(s)) allClassStrings.set(s, []);
    allClassStrings.get(s).push({ file, fullMatch: match[0] });
  }
}

const entries = [...allClassStrings.keys()];
console.log(`Found ${entries.length} unique className strings across ${allClassStrings.size} groups`);

// Batch canonicalize
const canonicalMap = new Map();
const BATCH = 20;

for (let i = 0; i < entries.length; i += BATCH) {
  const batch = entries.slice(i, i + BATCH);
  const args = batch.map((s) => JSON.stringify(s)).join(" ");
  try {
    const result = execSync(
      `npx @tailwindcss/cli canonicalize ${args} 2>/dev/null`,
      { cwd: CWD, encoding: "utf-8", timeout: 10000 },
    ).trim();
    const lines = result.split("\n");
    for (let j = 0; j < batch.length && j < lines.length; j++) {
      const canon = lines[j].trim();
      if (canon && canon !== batch[j]) canonicalMap.set(batch[j], canon);
    }
  } catch {
    for (const s of batch) {
      try {
        const result = execSync(
          `npx @tailwindcss/cli canonicalize ${JSON.stringify(s)} 2>/dev/null`,
          { cwd: CWD, encoding: "utf-8", timeout: 5000 },
        ).trim();
        if (result && result !== s) canonicalMap.set(s, result);
      } catch { /* skip */ }
    }
  }
}

if (canonicalMap.size === 0) {
  console.log("✅ All className strings are already canonical.");
  process.exit(0);
}

console.log(`\nChanges (${canonicalMap.size}):`);
for (const [orig, canon] of canonicalMap) {
  console.log(`  "${orig}" → "${canon}"`);
}

// Apply
const fileChanges = new Map();
for (const [orig, canon] of canonicalMap) {
  for (const { file, fullMatch } of allClassStrings.get(orig)) {
    if (!fileChanges.has(file)) fileChanges.set(file, readFileSync(file, "utf-8"));
    fileChanges.set(file, fileChanges.get(file).replace(fullMatch, `className="${canon}"`));
  }
}

for (const [file, content] of fileChanges) {
  writeFileSync(file, content, "utf-8");
}

console.log(`\n✅ Applied to ${fileChanges.size} files (${canonicalMap.size} changes).`);

// vendor 期索引生成：扫描 design-systems/*，产出一份静态 index.json。
// 运行期只读这个文件（169 次文件读取 → 1 次），封面色从 tokens.css 现场解析。
// 用法：node scripts/gen-ds-index.mjs <design-systems 目录> <输出文件>
import { readdirSync, readFileSync, writeFileSync, existsSync, statSync } from "node:fs";
import { join } from "node:path";

const [dir, out] = process.argv.slice(2);
if (!dir || !out) {
  console.error("用法: node gen-ds-index.mjs <design-systems目录> <输出json>");
  process.exit(1);
}

// 去掉 C 风格块注释
function stripComments(css) {
  return css.replace(/\/\*[\s\S]*?\*\//g, "");
}

/** 从 tokens.css 解析封面需要的令牌值（跳过 var() 引用，取字面值） */
function coverFromTokens(css) {
  const body = stripComments(css);
  const map = {};
  for (const m of body.matchAll(/--([a-z0-9-]+)\s*:\s*([^;]+);/gi)) {
    const v = m[2].trim();
    if (!v.startsWith("var(") && !(m[1] in map)) map[m[1]] = v;
  }
  return {
    bg: map.bg ?? map.surface ?? "#1e1e1e",
    fg: map.fg ?? map["fg-2"] ?? "#e8e8e8",
    mut: map.muted ?? map.meta ?? "#8a8f98",
    bd: map.border ?? map["border-soft"] ?? "rgba(255,255,255,.14)",
    btn: map.accent ?? map.fg ?? "#4b9eff",
    r: map["radius-sm"] ?? map.radius ?? "6px",
  };
}

const packs = [];
for (const entry of readdirSync(dir).sort()) {
  const pdir = join(dir, entry);
  if (!statSync(pdir).isDirectory()) continue;
  const manifestPath = join(pdir, "manifest.json");
  const tokensPath = join(pdir, "tokens.css");
  if (!existsSync(manifestPath) || !existsSync(tokensPath)) continue;
  try {
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    const tokens = readFileSync(tokensPath, "utf8");
    packs.push({
      id: entry,
      name: manifest.name ?? entry,
      category: manifest.category ?? "",
      description: manifest.description ?? "",
      cover: coverFromTokens(tokens),
      hasComponents: existsSync(join(pdir, "components.html")),
    });
  } catch (e) {
    console.error(`跳过 ${entry}: ${e.message}`);
  }
}

writeFileSync(out, JSON.stringify({ total: packs.length, packs }, null, 1));
console.log(`索引就绪: ${packs.length} 个包 → ${out} (${Math.round(readFileSync(out).length / 1024)}KB)`);

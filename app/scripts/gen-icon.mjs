// 生成 1024x1024 纯色应用图标 PNG（零依赖，Node 内置 zlib）
// 用途：仅作为 tauri icon 的源图，后续可换成正式设计稿
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const SIZE = 1024;
const RGB = [108, 108, 245]; // #6C6CF5 indigo

const crcTable = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 2; // color type: truecolor RGB

// 每行前加 filter byte 0；居中画一个白色圆角方块增加辨识度
const raw = Buffer.alloc(SIZE * (SIZE * 3 + 1));
const m = SIZE * 0.2, M = SIZE * 0.8, r = SIZE * 0.06;
for (let y = 0; y < SIZE; y++) {
  const rowStart = y * (SIZE * 3 + 1);
  raw[rowStart] = 0;
  for (let x = 0; x < SIZE; x++) {
    const inBox = x >= m && x <= M && y >= m && y <= M;
    const near = Math.max(Math.abs(x - SIZE / 2), Math.abs(y - SIZE / 2)) - (M - m) / 2;
    const inRound = inBox && near > -r;
    const isBar = inRound && y >= SIZE * 0.42 && y <= SIZE * 0.58;
    const [R, G, B] = isBar ? [255, 255, 255] : RGB;
    const p = rowStart + 1 + x * 3;
    raw[p] = R;
    raw[p + 1] = G;
    raw[p + 2] = B;
  }
}

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

const outPath = join(dirname(fileURLToPath(import.meta.url)), "..", "app-icon.png");
writeFileSync(outPath, png);
console.log(`written: ${outPath}`);

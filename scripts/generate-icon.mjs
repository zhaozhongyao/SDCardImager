import { deflateSync } from "node:zlib";
import { writeFileSync, mkdirSync } from "node:fs";

const W = 1024;
const H = 1024;
const c1 = [0x25, 0x63, 0xeb]; // blue-500
const c2 = [0x4f, 0x46, 0xe5]; // indigo-600
const R = 185;

const stride = 1 + W * 4;
const raw = Buffer.alloc(H * stride);

function inRoundedRect(x, y) {
  if (x < R && y < R) {
    const dx = R - x, dy = R - y;
    return dx * dx + dy * dy <= R * R;
  }
  if (x > W - R && y < R) {
    const dx = x - (W - R), dy = R - y;
    return dx * dx + dy * dy <= R * R;
  }
  if (x < R && y > H - R) {
    const dx = R - x, dy = y - (H - R);
    return dx * dx + dy * dy <= R * R;
  }
  if (x > W - R && y > H - R) {
    const dx = x - (W - R), dy = y - (H - R);
    return dx * dx + dy * dy <= R * R;
  }
  return true;
}

// 白色 "SD" 文字（矩形像素字）
function inLetter(x, y) {
  // S
  if (
    (y >= 320 && y <= 410 && x >= 300 && x <= 480) ||
    (x >= 300 && x <= 390 && y >= 410 && y <= 530) ||
    (y >= 520 && y <= 610 && x >= 300 && x <= 480) ||
    (x >= 390 && x <= 480 && y >= 610 && y <= 720) ||
    (y >= 710 && y <= 800 && x >= 300 && x <= 480)
  ) {
    return true;
  }
  // D
  if (
    (x >= 560 && x <= 650 && y >= 320 && y <= 800) ||
    (y >= 320 && y <= 410 && x >= 560 && x <= 750) ||
    (x >= 660 && x <= 750 && y >= 410 && y <= 710) ||
    (y >= 710 && y <= 800 && x >= 560 && x <= 750)
  ) {
    return true;
  }
  return false;
}

for (let y = 0; y < H; y++) {
  raw[y * stride] = 0;
  for (let x = 0; x < W; x++) {
    const off = y * stride + 1 + x * 4;
    if (!inRoundedRect(x, y)) {
      raw[off + 3] = 0;
      continue;
    }
    if (inLetter(x, y)) {
      raw[off] = 255;
      raw[off + 1] = 255;
      raw[off + 2] = 255;
      raw[off + 3] = 255;
      continue;
    }
    const t = (x + y) / (W + H);
    raw[off] = Math.round(c1[0] + (c2[0] - c1[0]) * t);
    raw[off + 1] = Math.round(c1[1] + (c2[1] - c1[1]) * t);
    raw[off + 2] = Math.round(c1[2] + (c2[2] - c1[2]) * t);
    raw[off + 3] = 255;
  }
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeBuf = Buffer.from(type, "ascii");
  const crcInput = Buffer.concat([typeBuf, data]);
  let crc = 0xffffffff;
  for (const byte of crcInput) {
    crc ^= byte;
    for (let k = 0; k < 8; k++) {
      crc = crc & 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
    }
  }
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE((crc ^ 0xffffffff) >>> 0);
  return Buffer.concat([len, typeBuf, data, crcBuf]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(W, 0);
ihdr.writeUInt32BE(H, 4);
ihdr[8] = 8;
ihdr[9] = 6;

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", Buffer.alloc(0)),
]);

mkdirSync("src-tauri/icons", { recursive: true });
writeFileSync("src-tauri/icons/app-icon.png", png);
console.log("icon written:", png.length, "bytes");

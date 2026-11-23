import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Resvg } from "@resvg/resvg-js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, "..");
const svgPath = resolve(rootDir, "public", "luair-icon.svg");
const iconsDir = resolve(rootDir, "src-tauri", "icons");
const svg = readFileSync(svgPath, "utf8");

function renderSvg(size) {
  const resvg = new Resvg(svg, {
    fitTo: {
      mode: "width",
      value: size,
    },
  });
  return resvg.render();
}

function encodeDibFrame(size, rgbaPixels) {
  const xorBytes = size * size * 4;
  const andRowStride = Math.ceil(size / 32) * 4;
  const andBytes = andRowStride * size;
  const buf = Buffer.alloc(40 + xorBytes + andBytes);

  // BITMAPINFOHEADER (40 bytes)
  buf.writeUInt32LE(40, 0); // biSize
  buf.writeInt32LE(size, 4); // biWidth
  buf.writeInt32LE(size * 2, 8); // biHeight (XOR + AND)
  buf.writeUInt16LE(1, 12); // biPlanes
  buf.writeUInt16LE(32, 14); // biBitCount
  buf.writeUInt32LE(0, 16); // biCompression (BI_RGB)
  buf.writeUInt32LE(xorBytes + andBytes, 20); // biSizeImage

  const xorOffset = 40;
  const andOffset = 40 + xorBytes;

  // Bottom-up 32-bit BGRA XOR bitmap + 1bpp AND mask
  for (let dstRow = 0; dstRow < size; dstRow++) {
    const srcY = size - 1 - dstRow;
    for (let x = 0; x < size; x++) {
      const srcIdx = (srcY * size + x) * 4;
      const r = rgbaPixels[srcIdx];
      const g = rgbaPixels[srcIdx + 1];
      const b = rgbaPixels[srcIdx + 2];
      const a = rgbaPixels[srcIdx + 3];

      const dstIdx = xorOffset + (dstRow * size + x) * 4;
      buf[dstIdx] = b;
      buf[dstIdx + 1] = g;
      buf[dstIdx + 2] = r;
      buf[dstIdx + 3] = a;

      if (a < 128) {
        const maskByteIdx = andOffset + dstRow * andRowStride + (x >> 3);
        buf[maskByteIdx] |= 0x80 >> (x & 7);
      }
    }
  }

  return buf;
}

function buildIco(frames) {
  const count = frames.length;
  const headerSize = 6 + count * 16;
  const header = Buffer.alloc(headerSize);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type = ICO
  header.writeUInt16LE(count, 4); // count

  let offset = headerSize;
  const chunks = [header];

  for (let i = 0; i < count; i++) {
    const { size, data } = frames[i];
    const entryOff = 6 + i * 16;
    header[entryOff] = size >= 256 ? 0 : size; // width
    header[entryOff + 1] = size >= 256 ? 0 : size; // height
    header[entryOff + 2] = 0; // color count
    header[entryOff + 3] = 0; // reserved
    header.writeUInt16LE(1, entryOff + 4); // planes
    header.writeUInt16LE(32, entryOff + 6); // bpp
    header.writeUInt32LE(data.length, entryOff + 8); // bytesInRes
    header.writeUInt32LE(offset, entryOff + 12); // imageOffset
    offset += data.length;
    chunks.push(data);
  }

  return Buffer.concat(chunks);
}

const pngTargets = [
  [1024, "source-1024.png"],
  [512, "icon.png"],
  [256, "128x128@2x.png"],
  [128, "128x128.png"],
  [64, "64x64.png"],
  [32, "32x32.png"],
];

for (const [size, filename] of pngTargets) {
  const rendered = renderSvg(size);
  const outPath = resolve(iconsDir, filename);
  writeFileSync(outPath, rendered.asPng());
  console.log(`Generated ${filename} (${rendered.width}x${rendered.height})`);
}

const icoSizes = [16, 24, 32, 48, 64, 256];
const icoFrames = icoSizes.map((size) => {
  const rendered = renderSvg(size);
  const data = size === 256 ? rendered.asPng() : encodeDibFrame(size, rendered.pixels);
  return { size, data };
});

const icoPath = resolve(iconsDir, "icon.ico");
writeFileSync(icoPath, buildIco(icoFrames));
console.log(`Generated icon.ico with frames: ${icoSizes.join(", ")}`);

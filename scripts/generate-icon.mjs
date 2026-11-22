import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Resvg } from "@resvg/resvg-js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(__dirname, "..");
const svgPath = resolve(rootDir, "public", "luair-icon.svg");
const outPath = resolve(rootDir, "src-tauri", "icons", "source-1024.png");

const svg = readFileSync(svgPath, "utf8");
const resvg = new Resvg(svg, {
  fitTo: {
    mode: "width",
    value: 1024,
  },
});

const rendered = resvg.render();
const pngData = rendered.asPng();
writeFileSync(outPath, pngData);

console.log(`Generated ${outPath} (${rendered.width}x${rendered.height})`);

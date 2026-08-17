/**
 * Gera os ícones do app e da bandeja.
 *
 * Nada de PNG binário versionado: a arte é código, e mudar a paleta é mudar as
 * constantes abaixo. Roda com `npm run icons`.
 */
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ICONS_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "icons");

/** Camadas concêntricas (raios normalizados) que desenham a "cebola". */
const RING = [0.78, 1.0];
const MIDDLE = [0.48, 0.64];
const CORE = [0.0, 0.3];

const STATES = {
  stopped: { color: [0x6b, 0x72, 0x80], layers: [RING] },
  connecting: { color: [0xf5, 0x9e, 0x0b], layers: [RING, MIDDLE] },
  connected: { color: [0x34, 0xd3, 0x99], layers: [RING, MIDDLE, CORE] },
  error: { color: [0xef, 0x44, 0x44], layers: [RING, CORE], slash: true },
};

/** Roxo do Tor, usado no ícone do app. */
const BRAND = { color: [0xa8, 0x76, 0xd8], layers: [RING, MIDDLE, CORE] };

// --- desenho -----------------------------------------------------------------

const SUPERSAMPLE = 3;

function render(size, { color, layers, slash = false }) {
  const pixels = Buffer.alloc(size * size * 4);
  const center = size / 2;
  const radius = center * 0.94;
  const step = 1 / SUPERSAMPLE;

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      let hits = 0;
      let samples = 0;

      for (let sy = 0; sy < SUPERSAMPLE; sy++) {
        for (let sx = 0; sx < SUPERSAMPLE; sx++) {
          const px = x + (sx + 0.5) * step - center;
          const py = y + (sy + 0.5) * step - center;
          const distance = Math.hypot(px, py) / radius;
          samples++;

          const inLayer = layers.some(([inner, outer]) => distance >= inner && distance <= outer);
          // A barra diagonal do estado de erro corta a arte.
          const inSlash = slash && Math.abs(px + py) < radius * 0.16;
          if (inLayer && !inSlash) hits++;
        }
      }

      const offset = (y * size + x) * 4;
      pixels[offset] = color[0];
      pixels[offset + 1] = color[1];
      pixels[offset + 2] = color[2];
      pixels[offset + 3] = Math.round((hits / samples) * 255);
    }
  }

  return pixels;
}

// --- PNG ---------------------------------------------------------------------

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) crc = CRC_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

function encodePng(size, pixels) {
  const stride = size * 4;
  const raw = Buffer.alloc((stride + 1) * size);
  for (let y = 0; y < size; y++) {
    raw[y * (stride + 1)] = 0; // filtro "none"
    pixels.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }

  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header[8] = 8; // bits por canal
  header[9] = 6; // RGBA
  header[10] = 0;
  header[11] = 0;
  header[12] = 0;

  return Buffer.concat([
    PNG_SIGNATURE,
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// --- ICO ---------------------------------------------------------------------

/**
 * O `.ico` do Windows usa entradas DIB (não PNG) para máxima compatibilidade
 * com o embutidor de recursos do build.
 */
function encodeDib(size, pixels) {
  const header = Buffer.alloc(40);
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(size, 4);
  header.writeInt32LE(size * 2, 8); // XOR + máscara AND
  header.writeUInt16LE(1, 12);
  header.writeUInt16LE(32, 14);

  const xor = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const from = (y * size + x) * 4;
      const to = ((size - 1 - y) * size + x) * 4; // DIB é bottom-up
      xor[to] = pixels[from + 2];
      xor[to + 1] = pixels[from + 1];
      xor[to + 2] = pixels[from];
      xor[to + 3] = pixels[from + 3];
    }
  }

  // Máscara AND zerada: a transparência real vem do canal alfa.
  const maskStride = Math.ceil(size / 32) * 4;
  const mask = Buffer.alloc(maskStride * size);

  header.writeUInt32LE(xor.length + mask.length, 20);
  return Buffer.concat([header, xor, mask]);
}

function encodeIco(entries) {
  const directory = Buffer.alloc(6 + entries.length * 16);
  directory.writeUInt16LE(0, 0);
  directory.writeUInt16LE(1, 2);
  directory.writeUInt16LE(entries.length, 4);

  let offset = directory.length;
  const images = [];

  entries.forEach(({ size, data }, index) => {
    const at = 6 + index * 16;
    directory[at] = size >= 256 ? 0 : size;
    directory[at + 1] = size >= 256 ? 0 : size;
    directory.writeUInt16LE(1, at + 4);
    directory.writeUInt16LE(32, at + 6);
    directory.writeUInt32LE(data.length, at + 8);
    directory.writeUInt32LE(offset, at + 12);
    offset += data.length;
    images.push(data);
  });

  return Buffer.concat([directory, ...images]);
}

// --- saída -------------------------------------------------------------------

mkdirSync(ICONS_DIR, { recursive: true });

const written = [];
function write(name, data) {
  writeFileSync(join(ICONS_DIR, name), data);
  written.push(`${name} (${data.length} B)`);
}

for (const [state, spec] of Object.entries(STATES)) {
  write(`tray-${state}.png`, encodePng(32, render(32, spec)));
}

for (const size of [32, 128, 256, 512]) {
  const name = { 32: "32x32.png", 128: "128x128.png", 256: "128x128@2x.png", 512: "icon.png" }[size];
  write(name, encodePng(size, render(size, BRAND)));
}

write(
  "icon.ico",
  encodeIco([16, 32, 48, 256].map((size) => ({ size, data: encodeDib(size, render(size, BRAND)) }))),
);

console.log(`ícones gerados em ${ICONS_DIR}:\n  ${written.join("\n  ")}`);

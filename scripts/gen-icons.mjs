/**
 * Gera os ícones do app e da bandeja. Roda com `npm run icons`.
 *
 * As duas artes são nuvens em meio-tom e vivem como PNG em `icons/source/`: um
 * binário versionado é o preço de ter retícula, porque nenhuma constante deste
 * arquivo desenha aquilo. O script decodifica, recorta, reamostra por média de
 * caixa e escreve os tamanhos que o Windows pede — inclusive o `.ico`.
 *
 * Da arte da bandeja saem os quatro estados: muda o tom do papel, a força e, no
 * erro, um corte diagonal. Trocar qualquer ícone é trocar o PNG de origem e
 * rodar `npm run icons`.
 */
import { deflateSync, inflateSync } from "node:zlib";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ICONS_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "icons");

/** Tons neutros do sistema — nenhum matiz, o estado é dito por força e corte. */
const INK = {
  base: [0xf5, 0xf5, 0xf5],
  mid: [0xc4, 0xc4, 0xc4],
  dim: [0x9a, 0x9a, 0x9a],
  rim: [0x56, 0x56, 0x56],
  dot: [0x19, 0x19, 0x19],
};

/**
 * Estados da bandeja. A silhueta é a da mesma nuvem do ícone do app, mas chapada:
 * a 16px a retícula vira borrão cinza, e o que sobrevive nesse tamanho é contorno
 * e área. O estado é dito pela forma — contorno, cheia, cheia com corte — nunca
 * por matiz.
 */
const TRAY_STATES = {
  stopped: { mode: "outline", tone: INK.dim },
  connecting: { mode: "fill", tone: INK.mid, rim: INK.dot },
  connected: { mode: "fill", tone: INK.base, rim: INK.rim },
  error: { mode: "fill", tone: INK.base, rim: INK.rim, slash: true },
};

// --- arte, vinda de PNG ------------------------------------------------------

/**
 * As duas artes são meio-tom: nenhuma constante deste arquivo desenha aquilo, e
 * por isso os PNGs de origem são versionados. Trocar o ícone é trocar o arquivo
 * em `icons/source/` e rodar `npm run icons`.
 */
const SOURCES = {
  app: join(ICONS_DIR, "source", "app-icon.png"),
  tray: join(ICONS_DIR, "source", "tray-cloud.png"),
};

/** Decodificador mínimo: 8 bits, RGB ou RGBA, sem entrelaçamento. */
function decodePng(file) {
  const bytes = readFileSync(file);
  let at = 8;
  let width = 0;
  let height = 0;
  let channels = 0;
  const parts = [];

  while (at < bytes.length) {
    const length = bytes.readUInt32BE(at);
    const type = bytes.toString("ascii", at + 4, at + 8);
    const body = bytes.subarray(at + 8, at + 8 + length);

    if (type === "IHDR") {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      const depth = body[8];
      const color = body[9];
      if (depth !== 8 || (color !== 2 && color !== 6) || body[12] !== 0) {
        throw new Error(`${file}: só 8 bits RGB/RGBA sem entrelaçamento`);
      }
      channels = color === 6 ? 4 : 3;
    } else if (type === "IDAT") {
      parts.push(body);
    }

    at += 12 + length;
  }

  const data = inflateSync(Buffer.concat(parts));
  const stride = width * channels;
  const pixels = Buffer.alloc(width * height * 4, 255);
  let previous = Buffer.alloc(stride);
  let offset = 0;

  for (let y = 0; y < height; y++) {
    const filter = data[offset];
    const line = Buffer.from(data.subarray(offset + 1, offset + 1 + stride));
    offset += 1 + stride;

    for (let i = 0; i < stride; i++) {
      const left = i >= channels ? line[i - channels] : 0;
      const up = previous[i];
      const corner = i >= channels ? previous[i - channels] : 0;

      if (filter === 1) line[i] = (line[i] + left) & 0xff;
      else if (filter === 2) line[i] = (line[i] + up) & 0xff;
      else if (filter === 3) line[i] = (line[i] + ((left + up) >> 1)) & 0xff;
      else if (filter === 4) {
        const p = left + up - corner;
        const dl = Math.abs(p - left);
        const du = Math.abs(p - up);
        const dc = Math.abs(p - corner);
        line[i] = (line[i] + (dl <= du && dl <= dc ? left : du <= dc ? up : corner)) & 0xff;
      }
    }

    for (let x = 0; x < width; x++) {
      const from = x * channels;
      const to = (y * width + x) * 4;
      line.copy(pixels, to, from, from + channels);
    }

    previous = line;
  }

  return { width, height, pixels };
}

/**
 * A arte vem num quadrado opaco: a margem preta em volta do cartão precisa virar
 * transparência, senão o ícone fica um retângulo preto no Explorer. Inundação a
 * partir das bordas, só por pixels quase pretos — nada de adivinhar o raio do
 * canto arredondado.
 */
function clearMargin({ width, height, pixels }) {
  const queue = [];
  const seen = new Uint8Array(width * height);

  const visit = (x, y) => {
    if (x < 0 || y < 0 || x >= width || y >= height) return;
    const index = y * width + x;
    if (seen[index]) return;
    const at = index * 4;
    if (pixels[at] > 8 || pixels[at + 1] > 8 || pixels[at + 2] > 8) return;
    seen[index] = 1;
    pixels[at + 3] = 0;
    queue.push(index);
  };

  for (let x = 0; x < width; x++) {
    visit(x, 0);
    visit(x, height - 1);
  }
  for (let y = 0; y < height; y++) {
    visit(0, y);
    visit(width - 1, y);
  }

  while (queue.length) {
    const index = queue.pop();
    const x = index % width;
    const y = (index - x) / width;
    visit(x + 1, y);
    visit(x - 1, y);
    visit(x, y + 1);
    visit(x, y - 1);
  }

  return { width, height, pixels };
}

/** Média de caixa: com meio-tom, qualquer outra reamostragem cintila. */
function resample(source, size) {
  const out = Buffer.alloc(size * size * 4);
  const ratio = source.width / size;

  for (let y = 0; y < size; y++) {
    const y0 = Math.floor(y * ratio);
    const y1 = Math.max(y0 + 1, Math.floor((y + 1) * ratio));

    for (let x = 0; x < size; x++) {
      const x0 = Math.floor(x * ratio);
      const x1 = Math.max(x0 + 1, Math.floor((x + 1) * ratio));
      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;
      let n = 0;

      for (let sy = y0; sy < y1; sy++) {
        for (let sx = x0; sx < x1; sx++) {
          const at = (sy * source.width + sx) * 4;
          const alpha = source.pixels[at + 3] / 255;
          r += source.pixels[at] * alpha;
          g += source.pixels[at + 1] * alpha;
          b += source.pixels[at + 2] * alpha;
          a += alpha;
          n++;
        }
      }

      const to = (y * size + x) * 4;
      // Desfaz a pré-multiplicação para o PNG, que guarda cor e alfa separados.
      out[to] = a > 0 ? Math.round(r / a) : 0;
      out[to + 1] = a > 0 ? Math.round(g / a) : 0;
      out[to + 2] = a > 0 ? Math.round(b / a) : 0;
      out[to + 3] = Math.round((a / n) * 255);
    }
  }

  return out;
}

/** Caixa da tinta: a arte vem com folga em volta, e folga não é ícone. */
function inkBounds({ width, height, pixels }) {
  let x0 = width;
  let y0 = height;
  let x1 = -1;
  let y1 = -1;

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (pixels[(y * width + x) * 4 + 3] <= 16) continue;
      if (x < x0) x0 = x;
      if (y < y0) y0 = y;
      if (x > x1) x1 = x;
      if (y > y1) y1 = y;
    }
  }

  if (x1 < 0) throw new Error("a arte está vazia");
  return { x0, y0, x1, y1 };
}

/**
 * Encaixa a arte recortada num quadrado, medindo *cobertura* em vez de cor: para
 * cada pixel do destino sai quanto dele é nuvem (`alpha`) e quanto é ponto da
 * retícula (`ink`). É a média que faz a retícula virar tom quando o ícone
 * encolhe para 16px.
 */
function coverage(source, size, margin = 0.06) {
  const box = inkBounds(source);
  const artWidth = box.x1 - box.x0 + 1;
  const artHeight = box.y1 - box.y0 + 1;
  const inner = size * (1 - margin * 2);
  const scale = Math.min(inner / artWidth, inner / artHeight);
  const drawWidth = artWidth * scale;
  const drawHeight = artHeight * scale;
  const left = (size - drawWidth) / 2;
  const top = (size - drawHeight) / 2;

  const alpha = new Float64Array(size * size);
  const ink = new Float64Array(size * size);

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      // Volta ao espaço da arte e integra o retângulo inteiro do pixel.
      const sx0 = box.x0 + ((x - left) / scale);
      const sx1 = box.x0 + ((x + 1 - left) / scale);
      const sy0 = box.y0 + ((y - top) / scale);
      const sy1 = box.y0 + ((y + 1 - top) / scale);

      let sum = 0;
      let inked = 0;
      let samples = 0;

      for (let sy = Math.floor(sy0); sy < Math.ceil(sy1); sy++) {
        for (let sx = Math.floor(sx0); sx < Math.ceil(sx1); sx++) {
          samples++;
          if (sx < 0 || sy < 0 || sx >= source.width || sy >= source.height) continue;
          const at = (sy * source.width + sx) * 4;
          const a = source.pixels[at + 3] / 255;
          if (a === 0) continue;
          const luma =
            (source.pixels[at] * 0.299 + source.pixels[at + 1] * 0.587 + source.pixels[at + 2] * 0.114) /
            255;
          sum += a;
          inked += a * (1 - luma);
        }
      }

      const index = y * size + x;
      alpha[index] = samples > 0 ? sum / samples : 0;
      ink[index] = sum > 0 ? inked / sum : 0;
    }
  }

  return { alpha, ink };
}

const trayCoverage = new Map();

/** Onde a nuvem cobre o pixel o bastante para virar tinta cheia. */
const SOLID = 0.42;

function trayIcon(size, { mode, tone, rim, slash = false }) {
  if (!trayCoverage.has(size)) trayCoverage.set(size, coverage(decodePng(SOURCES.tray), size, 0.04));
  const { alpha } = trayCoverage.get(size);
  const pixels = Buffer.alloc(size * size * 4);

  const filled = (x, y) =>
    x >= 0 && y >= 0 && x < size && y < size && alpha[y * size + x] >= SOLID;

  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      if (!filled(x, y)) continue;

      // O corte do erro abre um vão na diagonal: funciona em barra clara e
      // escura, ao contrário de uma barra de cor.
      if (slash && Math.abs(x + y - size + 1) / size < 0.08) continue;

      const edge =
        !filled(x - 1, y) || !filled(x + 1, y) || !filled(x, y - 1) || !filled(x, y + 1);

      // Só contorno: o estado parado é a nuvem vazia.
      if (mode === "outline" && !edge) continue;

      // Aro escuro na borda da nuvem cheia — é ele que a segura numa barra de
      // tarefas clara, onde branco sobre branco desaparece.
      const color = mode === "fill" && edge && rim ? rim : tone;
      const at = (y * size + x) * 4;
      pixels[at] = color[0];
      pixels[at + 1] = color[1];
      pixels[at + 2] = color[2];
      pixels[at + 3] = 255;
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

for (const [file, path] of Object.entries(SOURCES)) {
  if (!existsSync(path)) throw new Error(`falta a arte de origem: ${path} (${file})`);
}

// 32px é o que a bandeja do Windows pede em 200%; em 100% ele reduz para 16 e a
// redução de 2:1 preserva a retícula melhor que qualquer outra.
for (const [state, spec] of Object.entries(TRAY_STATES)) {
  write(`tray-${state}.png`, encodePng(32, trayIcon(32, spec)));
}

const app = clearMargin(decodePng(SOURCES.app));
const appIcon = (size) => resample(app, size);

for (const size of [32, 128, 256, 512]) {
  const name = { 32: "32x32.png", 128: "128x128.png", 256: "128x128@2x.png", 512: "icon.png" }[size];
  write(name, encodePng(size, appIcon(size)));
}

write(
  "icon.ico",
  encodeIco([16, 32, 48, 256].map((size) => ({ size, data: encodeDib(size, appIcon(size)) }))),
);

console.log(`ícones gerados em ${ICONS_DIR}:\n  ${written.join("\n  ")}`);

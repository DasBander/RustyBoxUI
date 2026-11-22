import {
  canyonCenter,
  riverCenter,
  riverWidth,
  waterLevel,
  groundHeight,
  COURSE_LENGTH,
  GATE_RADIUS,
} from './course.js';

const tau = Math.PI * 2;
const sand = [0.82, 0.5, 0.3],
  rock = [0.66, 0.31, 0.17];
const ivory = [0.98, 0.91, 0.72],
  ember = [0.98, 0.26, 0.075],
  graphite = [0.13, 0.15, 0.15];

function random(seed) {
  let n = seed >>> 0;
  return () => {
    n = (Math.imul(n, 1664525) + 1013904223) >>> 0;
    return n / 4294967296;
  };
}

class Mesh {
  vertices = [];
  triangle(a, b, c, color) {
    const u = b.map((v, i) => v - a[i]),
      v = c.map((x, i) => x - a[i]);
    const normal = [
      u[1] * v[2] - u[2] * v[1],
      u[2] * v[0] - u[0] * v[2],
      u[0] * v[1] - u[1] * v[0],
    ];
    const length = Math.hypot(...normal) || 1;
    const unit = normal.map((n) => n / length);
    for (const point of [a, b, c]) this.vertices.push(...point, ...unit, ...color);
  }
  quad(a, b, c, d, color) {
    this.triangle(a, b, c, color);
    this.triangle(a, c, d, color);
  }
  data() {
    return new Float32Array(this.vertices);
  }
}

// Rings of uneven rock form the vertical faces and cap of each freestanding sandstone tower.
function butte(mesh, x, z, height, width, rng) {
  const base = groundHeight(x, z) - width * 0.48;
  const sides = 9;
  const outline = Array.from({ length: sides }, (_, i) => {
    const a = (tau * (i + 0.3 * rng())) / sides;
    const r = width * (0.8 + 0.3 * rng());
    return [Math.cos(a) * r, Math.sin(a) * r * 0.79, rng()];
  });
  const ledges = [
    [0, 1.2],
    [0.19, 0.91],
    [0.22, 0.93],
    [0.45, 0.75],
    [0.48, 0.78],
    [0.74, 0.69],
    [0.78, 0.63],
    [0.97, 0.59],
    [1, 0.52],
  ];
  let previous;
  for (const [level, scale] of ledges) {
    const ring = outline.map(([dx, dz, n]) => [
      x + dx * scale + level * width * 0.09,
      base + (level + n * 0.022) * height,
      z + dz * scale,
    ]);
    if (previous)
      for (let i = 0; i < sides; i++) {
        const j = (i + 1) % sides;
        mesh.quad(previous[i], ring[i], ring[j], previous[j], level > 0.95 ? sand : rock);
      }
    previous = ring;
  }
  const cap = [x + width * 0.09, base + height * 1.02, z];
  for (let i = 0; i < sides; i++) mesh.triangle(cap, previous[(i + 1) % sides], previous[i], sand);
}

// Faceted ellipsoids work for small details whose silhouettes matter at flight speed.
function ovoid(mesh, center, radii, color, sides = 7, rotation = 0) {
  const point = (i, lat) => {
    const a = (tau * i) / sides + rotation;
    return [
      center[0] + radii[0] * Math.cos(a) * Math.cos(lat),
      center[1] + radii[1] * Math.sin(lat),
      center[2] + radii[2] * Math.sin(a) * Math.cos(lat),
    ];
  };
  for (let i = 0; i < sides; i++) {
    const low = -0.39,
      high = 0.4;
    mesh.triangle(point(i, low), point(i + 1, low), point(i, -Math.PI / 2), color);
    mesh.quad(point(i, low), point(i, high), point(i + 1, high), point(i + 1, low), color);
    mesh.triangle(point(i, high), point(i, Math.PI / 2), point(i + 1, high), color);
  }
}

function strut(mesh, a, b, radius, color) {
  const d = b.map((v, i) => v - a[i]);
  const horizontal = Math.hypot(d[0], d[2]);
  const x = horizontal > 0 ? [-d[2] / horizontal, 0, d[0] / horizontal] : [1, 0, 0];
  const length = Math.hypot(...d);
  const y = [
    (d[1] * x[2] - d[2] * x[1]) / length,
    (d[2] * x[0] - d[0] * x[2]) / length,
    (d[0] * x[1] - d[1] * x[0]) / length,
  ];
  const ring = (p, j) =>
    p.map((v, i) => v + radius * (x[i] * Math.cos((j * tau) / 5) + y[i] * Math.sin((j * tau) / 5)));
  for (let j = 0; j < 5; j++)
    mesh.quad(ring(a, j), ring(b, j), ring(b, j + 1), ring(a, j + 1), color);
}

function juniper(mesh, x, z, h, rng) {
  const base = groundHeight(x, z) - 0.18;
  const lean = 0.22 * h * (rng() - 0.5);
  const leaf = [0.26 + 0.07 * rng(), 0.33 + 0.08 * rng(), 0.23 + 0.04 * rng()];
  strut(mesh, [x, base, z], [x + lean, base + h * 0.68, z], h * 0.055, [0.33, 0.24, 0.17]);
  ovoid(mesh, [x + lean, base + h * 0.63, z], [h * 0.36, h * 0.37, h * 0.34], leaf, 6, rng() * tau);
  ovoid(
    mesh,
    [x - h * 0.22, base + h * 0.49, z + h * 0.08],
    [h * 0.27, h * 0.23, h * 0.24],
    leaf,
    5,
    rng() * tau
  );
}

const sectionDepth = 168;
function landscapeSection(start, index) {
  const end = start + sectionDepth;
  const land = new Mesh(),
    water = new Mesh(),
    foliage = new Mesh();
  const ground = (lateral, z) => {
    const x = canyonCenter(z) + lateral;
    return [x, groundHeight(x, z), z];
  };
  const xSamples = [-390, -270, -190, -150];
  for (let x = -138; x <= 138; x += 4.6) xSamples.push(x);
  xSamples.push(150, 190, 270, 390);
  let row = xSamples.map((x) => ground(x, start));
  for (let z = start + 7; z <= end; z += 7) {
    const next = xSamples.map((x) => ground(x, z));
    for (let i = 0; i + 1 < row.length; i++) {
      const color = Math.abs(xSamples[i]) > 44 ? rock : sand;
      land.quad(row[i], next[i], next[i + 1], row[i + 1], color);
    }
    row = next;
    const shore = (z, side) => [riverCenter(z) + side * (riverWidth(z) + 5), waterLevel(z), z];
    water.quad(shore(z - 7, -1), shore(z, -1), shore(z, 1), shore(z - 7, 1), [0.09, 0.47, 0.43]);
  }
  const rng = random(12173 + 577 * index);
  for (const side of [-1, 1]) {
    const z = start + rng() * sectionDepth;
    const x = canyonCenter(z) + side * (74 + 96 * rng());
    butte(land, x, z, 27 + 62 * rng(), 16 + 20 * rng(), rng);
    for (let i = 0; i < 4; i++) {
      const rz = start + rng() * sectionDepth;
      const rx = riverCenter(rz) + side * (13 + 21 * rng());
      butte(land, rx, rz, 1.2 + rng() * 3, 1.2 + rng() * 3, rng);
    }
    for (let i = 0; i < 20; i++) {
      const rz = start + rng() * sectionDepth;
      juniper(
        foliage,
        riverCenter(rz) + side * (riverWidth(rz) + 4 + rng() * 23),
        rz,
        1.0 + 3.8 * rng(),
        rng
      );
    }
  }
  return { start, end, land: land.data(), water: water.data(), foliage: foliage.data() };
}

export function buildLandscape() {
  const sections = [];
  for (let start = -sectionDepth; start < COURSE_LENGTH + 1100; start += sectionDepth) {
    sections.push(landscapeSection(start, sections.length));
  }
  return sections;
}

export function buildGate() {
  const ring = new Mesh(),
    segments = 56,
    tubeSides = 6;
  const point = (i, j) => {
    const a = (tau * i) / segments,
      b = (tau * j) / tubeSides;
    const r = GATE_RADIUS + 0.46 * Math.cos(b);
    return [r * Math.cos(a), r * Math.sin(a), 0.55 * Math.sin(b)];
  };
  for (let i = 0; i < segments; i++) {
    const color = i % 14 < 2 ? graphite : i % 14 < 5 ? ivory : [0.53, 0.96, 0.77];
    for (let j = 0; j < tubeSides; j++)
      ring.quad(point(i, j), point(i + 1, j), point(i + 1, j + 1), point(i, j + 1), color);
  }
  return ring.data();
}

export function buildGlider() {
  const wing = new Mesh();
  const nose = [0, 0.13, 3.5],
    spine = [0, 0.48, 0.6],
    tail = [0, 0.2, -2.3];
  for (const s of [-1, 1]) {
    const point = (x, y, z) => [s * x, y, z];
    const root = point(0.42, 0.17, 1.18),
      aft = point(0.42, 0.05, -1.92);
    const tip = point(5.3, 0.19, -2.45),
      tipFront = point(5.25, 0.25, -1.88);
    const flash = point(3.12, 0.22, -0.66),
      back = point(2.88, 0.1, -1.72);
    wing.triangle(nose, tipFront, flash, ivory);
    wing.triangle(nose, flash, root, ivory);
    wing.quad(root, flash, back, aft, s < 0 ? ivory : ember);
    wing.quad(flash, tipFront, tip, back, s < 0 ? ember : graphite);
    wing.triangle(nose, root, spine, ivory);
    wing.quad(spine, root, aft, tail, s < 0 ? [0.84, 0.75, 0.56] : [0.79, 0.23, 0.095]);
    // Dark sail battens and the triangular control frame are visible in a bank.
    strut(wing, point(0.42, 0.23, 1.18), point(2.9, 0.16, -1.72), 0.028, graphite);
    strut(wing, [0, 0.18, 0.6], point(0.78, -1.35, 0.35), 0.045, graphite);
    strut(wing, point(0.78, -1.35, 0.35), point(4.7, 0.13, -1.65), 0.019, graphite);
  }
  strut(wing, [-0.78, -1.35, 0.35], [0.78, -1.35, 0.35], 0.046, graphite);
  strut(wing, [0, 0.16, -0.65], [0, -1.05, -0.65], 0.05, graphite);
  ovoid(wing, [0, -1.15, -1.22], [0.35, 0.3, 1.35], [0.18, 0.21, 0.2]);
  ovoid(wing, [0, -1.12, -0.14], [0.36, 0.3, 0.51], ember);
  ovoid(wing, [0, -0.92, 0.43], [0.25, 0.26, 0.28], ivory);
  return wing.data();
}

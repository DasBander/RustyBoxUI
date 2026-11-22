// World space is measured in metres. Positive Z travels downstream.
export let COURSE_NAME = 'Copper Narrows';
export let COURSE_LENGTH = 2220;
let bend = 1;
export const GATE_RADIUS = 10;
export const MISS_PENALTY = 4;
export const clamp = (value, low, high) => Math.max(low, Math.min(high, value));
export const approach = (value, target, rate, dt) =>
  value + (target - value) * (1 - Math.exp(-rate * dt));
export function smoothstep(low, high, value) {
  const t = clamp((value - low) / (high - low), 0, 1);
  return t * t * (3 - 2 * t);
}

export function canyonCenter(z) {
  return 32 * Math.sin((z * bend) / 183) + 13 * Math.sin((z * bend) / 76);
}
export function riverCenter(z) {
  return canyonCenter(z) + 6 * Math.sin(z / 49 + 0.7);
}
export function riverWidth(z) {
  return 7.1 + 1.4 * Math.sin(z / 53);
}
export function waterLevel(z) {
  return 1.4 + 0.7 * Math.sin(z / 260);
}

export function groundHeight(x, z) {
  const side = x - canyonCenter(z);
  const distance = Math.abs(side);
  const wall = distance + 8 * Math.sin(z / 72 + Math.sign(side) * 2) + 4 * Math.sin(z / 33);
  const shore = smoothstep(-2, 8, Math.abs(x - riverCenter(z)) - riverWidth(z));
  const shelf = 10 * smoothstep(29, 47, wall) + 24 * smoothstep(50, 58, wall);
  const upper = (24 + 13 * Math.sin(z / 110 + side / 75)) * smoothstep(67, 81, wall);
  const rim = (26 + 17 * Math.sin(z / 139 - side / 68)) * smoothstep(95, 120, wall);
  const erosion = Math.sin(x * 0.19 + z * 0.07) + 1.8 * Math.sin(z * 0.087 - x * 0.1);
  return (
    waterLevel(z) -
    2.4 +
    shore * (4 + erosion * 0.35) +
    shelf +
    upper +
    rim +
    smoothstep(22, 58, distance) * erosion
  );
}

const lines = [6, -11, 13, -8, 10, -12, 9, -7, 0];
const heights = [22, 27, 34, 25, 31, 22, 33, 25, 27];
const builtIn = {
  name: 'Copper Narrows',
  length: 2220,
  bend: 1,
  gates: [125, 362, 602, 840, 1082, 1320, 1558, 1798, 2040].map((z, i) => ({
    z,
    offset: lines[i],
    height: heights[i],
  })),
};
export let GATES;
export function useCourse(definition = builtIn) {
  const within = (n, low, high) => Number.isFinite(n) && n >= low && n <= high;
  const { name, length, bend: turns, gates } = definition;
  if (
    typeof name !== 'string' ||
    !name.trim() ||
    name.length > 64 ||
    !within(length, 400, 6000) ||
    !within(turns, 0.5, 2) ||
    !Array.isArray(gates) ||
    gates.length < 1 ||
    gates.length > 50 ||
    !gates.every(
      (g, i) =>
        g &&
        within(g.z, (gates[i - 1]?.z ?? 0) + 60, length - 30) &&
        within(g.offset, -26, 26) &&
        within(g.height, 15, 55)
    )
  )
    throw new Error('Invalid flight course.');
  COURSE_NAME = name;
  COURSE_LENGTH = length;
  bend = turns;
  GATES = gates.map(({ z, offset, height }) => ({ x: canyonCenter(z) + offset, y: height, z }));
}
useCourse();

export function newFlight() {
  return {
    x: canyonCenter(0),
    y: 23,
    z: 0,
    vx: 0,
    vy: 0,
    speed: 54,
    bank: 0,
    pitch: 0,
    charge: 1,
    boosting: false,
    depleted: false,
    elapsed: 0,
    penalty: 0,
    next: 0,
    hits: 0,
    impactCooldown: 0,
    lastEvent: '',
    finished: false,
  };
}

// A plane crossing scores the gate even when a slow frame spans its centre.
export function stepFlight(flight, input, dt) {
  if (flight.finished) return;
  const previous = { x: flight.x, y: flight.y, z: flight.z };
  flight.lastEvent = '';
  flight.elapsed += dt;
  flight.impactCooldown = Math.max(0, flight.impactCooldown - dt);
  if (!input.boost && flight.charge >= 0.16) flight.depleted = false;
  flight.boosting = input.boost && !flight.depleted;
  flight.charge = clamp(flight.charge + (flight.boosting ? -0.24 : 0.105) * dt, 0, 1);
  if (flight.charge === 0) flight.depleted = true;
  flight.speed = approach(flight.speed, flight.boosting ? 87 : 54, 1.9, dt);
  flight.vx = approach(flight.vx, input.x * 34, 4.2, dt);
  flight.vy = approach(flight.vy, input.y * 24, 4.0, dt);
  flight.bank = approach(flight.bank, -input.x * 0.67, 5, dt);
  flight.pitch = approach(flight.pitch, input.y * 0.16, 4, dt);
  flight.x += flight.vx * dt;
  flight.y = Math.min(66, flight.y + flight.vy * dt);
  flight.z += flight.speed * dt;
  const center = canyonCenter(flight.z);
  flight.x = clamp(flight.x, center - 62, center + 62);
  const floor = groundHeight(flight.x, flight.z) + 2.1;
  if (flight.y < floor) {
    flight.y = floor + 0.25;
    flight.vy = Math.max(5, flight.vy);
    if (!flight.impactCooldown) {
      flight.penalty += 2;
      flight.impactCooldown = 2;
      flight.lastEvent = 'scrape';
    }
  }
  const gate = GATES[flight.next];
  if (gate && flight.z >= gate.z) {
    const t = clamp((gate.z - previous.z) / (flight.z - previous.z), 0, 1);
    const dx = previous.x + t * (flight.x - previous.x) - gate.x;
    const dy = previous.y + t * (flight.y - previous.y) - gate.y;
    if (Math.hypot(dx, dy) < GATE_RADIUS) {
      flight.hits++;
      flight.charge = Math.min(1, flight.charge + 0.12);
      flight.lastEvent = 'gate';
    } else {
      flight.penalty += MISS_PENALTY;
      flight.lastEvent = 'miss';
    }
    flight.next++;
  }
  flight.finished = flight.z >= COURSE_LENGTH;
}

export function clock(seconds, precision = 2) {
  const ticks = Math.floor(Math.max(0, seconds) * 10 ** precision);
  const whole = Math.floor(ticks / 10 ** precision);
  return (
    String(Math.floor(whole / 60)).padStart(2, '0') +
    ':' +
    String(whole % 60).padStart(2, '0') +
    '.' +
    String(ticks % 10 ** precision).padStart(precision, '0')
  );
}

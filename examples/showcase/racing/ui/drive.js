export const lapLength = 1800;
export function newLap() {
  return { distance: 0, speed: 0, lane: 0, steer: 0, time: 0 };
}
export function curvature(distance) {
  const angle = (2 * Math.PI * distance) / lapLength;
  return 0.65 * Math.sin(angle * 3 + 0.7) + 0.27 * Math.sin(angle * 5 - 0.32);
}
export function advance(car, keys, assist, dt) {
  const throttle = keys.has('KeyW') || keys.has('ArrowUp');
  const brake = keys.has('KeyS') || keys.has('ArrowDown');
  const steering =
    Number(keys.has('KeyD') || keys.has('ArrowRight')) -
    Number(keys.has('KeyA') || keys.has('ArrowLeft'));
  const offRoad = Math.max(0, Math.abs(car.lane) - 0.82);
  car.speed = Math.max(
    0,
    Math.min(
      70,
      car.speed +
        ((throttle ? 23 : 0) - (brake ? 42 : 0) - 3.3 - car.speed * 0.12 - offRoad * 51) * dt
    )
  );
  car.steer += (steering - car.steer) * Math.min(1, dt * 6);
  const force = curvature(car.distance) * (car.speed / 70) ** 2;
  const centering = assist && !steering ? car.lane * 0.85 : 0;
  car.lane +=
    (car.steer * (0.5 + car.speed / 47) - force * (assist ? 0.63 : 1.37) - centering) *
    Math.min(1, car.speed / 4) *
    dt;
  car.lane = Math.max(-1.55, Math.min(1.55, car.lane));
  const remaining = lapLength - car.distance;
  car.time += car.speed * dt > remaining ? remaining / car.speed : dt;
  car.distance = Math.min(lapLength, car.distance + car.speed * dt);
}

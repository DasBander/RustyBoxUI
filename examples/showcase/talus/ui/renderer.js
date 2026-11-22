import { buildLandscape, buildGate, buildGlider } from './geometry.js';
import { canyonCenter, GATES } from './course.js';

const identity = new Float32Array([1, 0, 0, 0, 1, 0, 0, 0, 1]);
const tau = Math.PI * 2;
const skyVertex = `attribute vec2 aPoint;
varying vec2 vScreen;
void main() { vScreen = aPoint; gl_Position = vec4(aPoint, 0.999, 1.0); }`;
const skyFragment = `precision highp float;
varying vec2 vScreen;
uniform mat3 uBasis;
uniform vec2 uScale;
float ridge(float x) { return sin(x * 9.0) * 0.017 + sin(x * 22.0 + 0.7) * 0.013 + sin(x * 48.0) * 0.006; }
void main() {
  vec3 ray = normalize(uBasis * vec3(vScreen * uScale, 1.0));
  vec3 color = mix(vec3(0.99,0.79,0.58), vec3(0.42,0.67,0.74), smoothstep(-0.045,0.51,ray.y));
  vec3 sun = normalize(vec3(0.52,0.26,1.0));
  float disc = dot(ray,sun);
  color += vec3(0.21,0.11,0.025)*pow(max(disc,0.0),28.0);
  color = mix(color,vec3(1.0,0.94,0.73),smoothstep(0.99935,0.99965,disc));
  float a = atan(ray.x,ray.z);
  float farRidge = 0.075+ridge(a);
  color = mix(vec3(0.73,0.58,0.53),color,smoothstep(farRidge-0.002,farRidge,ray.y));
  float nearRidge = 0.038+ridge(a+1.8)*1.3;
  color = mix(vec3(0.64,0.46,0.42),color,smoothstep(nearRidge-0.002,nearRidge,ray.y));
  gl_FragColor = vec4(color,1.0);
}`;
const worldVertex = `attribute vec3 aPosition, aNormal, aColor;
uniform mat3 uView, uModel;
uniform vec3 uEye, uOrigin;
uniform vec2 uScale;
varying vec3 vPosition, vNormal, vColor;
varying float vDepth;
void main() {
  vPosition = uOrigin + uModel * aPosition;
  vec3 view = uView * (vPosition-uEye);
  vDepth = view.z; vNormal = uModel * aNormal; vColor = aColor;
  // Perspective depth for near = 0.6 m, far = 1350 m.
  gl_Position = vec4(view.xy/uScale, view.z*1.0008893-1.2005336, view.z);
}`;
const worldFragment = `precision highp float;
varying vec3 vPosition, vNormal, vColor;
varying float vDepth;
uniform int uMaterial;
uniform float uTime;
uniform vec3 uPilot;
float hash(vec2 p) {
  vec3 q=fract(vec3(p.xyx)*vec3(0.1031,0.1030,0.0973));
  q+=dot(q,q.yzx+33.33);
  return fract((q.x+q.y)*q.z);
}
float noise(vec2 p) {
  vec2 i=floor(p), f=fract(p); f=f*f*(3.0-2.0*f);
  return mix(mix(hash(i),hash(i+vec2(1,0)),f.x),mix(hash(i+vec2(0,1)),hash(i+1.0),f.x),f.y);
}
float centre(float z) { return 32.0*sin(z/183.0)+13.0*sin(z/76.0); }
void main() {
  vec3 n = normalize(vNormal);
  if (uMaterial==3 && n.y<0.0) n=-n;
  vec3 base = vColor;
  float light = max(dot(n,normalize(vec3(-0.64,0.79,0.32))),0.0);
  vec3 color = base * (vec3(0.50,0.52,0.57)+light*vec3(0.62,0.55,0.41));
  if (uMaterial==0) {
    vec2 uv=vec2(vPosition.x*0.71+vPosition.z*0.77,vPosition.y);
    float broad=noise(uv*0.068);
    float grit=noise(uv*0.9);
    float y=vPosition.y+2.1*sin(vPosition.z*0.021)+1.7*broad;
    float strata=noise(vec2(y*0.40,0.5+broad*0.8));
    float seam=1.0-smoothstep(0.01,0.12,abs(fract(y*0.19+0.3*broad)-0.5));
    color=mix(color*vec3(0.91,0.85,0.88),color*vec3(1.09,1.06,1.01),strata);
    color*=0.93+0.14*broad+0.055*grit-0.063*seam;
    color+=vec3(0.077,0.058,0.032)*smoothstep(0.67,0.94,n.y);
    float shade=1.0-smoothstep(-26.0,-10.0,vPosition.x-centre(vPosition.z)+sin(vPosition.z*0.045)*7.0);
    color*=mix(vec3(1.0),vec3(0.77,0.78,0.81),shade*(1.0-smoothstep(6.0,28.0,vPosition.y)));
  } else if (uMaterial==1) {
    float river=centre(vPosition.z)+6.0*sin(vPosition.z/49.0+0.7);
    float shore=smoothstep(0.65,1.16,abs(vPosition.x-river)/(7.1+1.4*sin(vPosition.z/53.0)));
    vec2 flow=vec2(vPosition.x-river,vPosition.z-uTime*2.2);
    float current=noise(flow*vec2(0.42,0.102));
    float ripples=noise(vec2(flow.x*1.55+current*2.1,flow.y*0.32));
    color=mix(vec3(0.075,0.32,0.32),vec3(0.30,0.50,0.44),0.22+0.64*current);
    color=mix(color,vec3(0.47,0.52,0.37),shore*0.60);
    color+=vec3(0.38,0.32,0.19)*smoothstep(0.70,0.93,ripples)*(0.25+0.75*current);
  } else if (uMaterial==2) {
    color=base*(0.85+0.18*light);
  } else if (uMaterial==3) {
    color=base*(0.69+0.39*light);
  }
  if (uMaterial<=1) {
    vec2 d=(vPosition.xz-uPilot.xz-vec2(2.0,-1.1))/vec2(4.9,2.2);
    color *= 0.76+0.24*smoothstep(0.25,1.4,dot(d,d));
  }
  float fog=1.0-exp(-max(vDepth-45.0,0.0)*0.00255);
  gl_FragColor=vec4(mix(color,vec3(0.87,0.68,0.55),fog),1.0);
}`;

function program(gl, vertex, fragment, uniforms, attributes) {
  const handle = gl.createProgram();
  for (const [type, source] of [
    [gl.VERTEX_SHADER, vertex],
    [gl.FRAGMENT_SHADER, fragment],
  ]) {
    const shader = gl.createShader(type);
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS))
      throw new Error(gl.getShaderInfoLog(shader));
    gl.attachShader(handle, shader);
    gl.deleteShader(shader);
  }
  gl.linkProgram(handle);
  if (!gl.getProgramParameter(handle, gl.LINK_STATUS))
    throw new Error(gl.getProgramInfoLog(handle));
  const locations = { handle };
  for (const name of uniforms) locations[name] = gl.getUniformLocation(handle, name);
  for (const name of attributes) locations[name] = gl.getAttribLocation(handle, name);
  return locations;
}
function meshBuffer(gl, vertices, stride = 9) {
  const buffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
  return { buffer, count: vertices.length / stride };
}
const unit = (v) => {
  const length = Math.hypot(...v);
  return v.map((x) => x / length);
};
const cross = (a, b) => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

export class CanyonRenderer {
  constructor(canvas) {
    this.canvas = canvas;
    const gl = canvas.getContext('webgl', {
      alpha: false,
      antialias: true,
      powerPreference: 'high-performance',
    });
    if (!gl)
      throw new Error('WebGL is unavailable. Enable graphics acceleration to fly this course.');
    this.gl = gl;
    this.sky = program(gl, skyVertex, skyFragment, ['uBasis', 'uScale'], ['aPoint']);
    this.world = program(
      gl,
      worldVertex,
      worldFragment,
      ['uView', 'uModel', 'uEye', 'uOrigin', 'uScale', 'uMaterial', 'uPilot', 'uTime'],
      ['aPosition', 'aNormal', 'aColor']
    );
    this.background = meshBuffer(gl, new Float32Array([-1, -1, 3, -1, -1, 3]), 2);
    this.gate = meshBuffer(gl, buildGate());
    this.glider = meshBuffer(gl, buildGlider());
    this.quality = 'balanced';
    this.sections = [];
    this.loadLandscape();
  }

  loadLandscape() {
    const gl = this.gl;
    for (const { land, water, foliage } of this.sections) {
      for (const mesh of [land, water, foliage]) gl.deleteBuffer(mesh.buffer);
    }
    this.sections = buildLandscape().map(({ start, end, land, water, foliage }) => ({
      start,
      end,
      land: meshBuffer(gl, land),
      water: meshBuffer(gl, water),
      foliage: meshBuffer(gl, foliage),
    }));
  }

  draw(flight) {
    const { gl, world, sky, canvas } = this;
    if (gl.isContextLost()) return;
    const pixels = this.quality === 'high' ? 2600 : 1600;
    const scale = Math.min(devicePixelRatio || 1, pixels / Math.max(1, canvas.clientWidth));
    const w = Math.max(1, Math.round(canvas.clientWidth * scale)),
      h = Math.max(1, Math.round(canvas.clientHeight * scale));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
    }
    const { x, y, z, vx, vy, bank, pitch } = flight;
    const eye = [x - vx * 0.115, y + 4.7, z - 17.4];
    const aim = [
      x + vx * 0.21 + (canyonCenter(z + 55) - canyonCenter(z)) * 0.24,
      y + 3.5 + vy * 0.075,
      z + 43,
    ];
    const forward = unit(aim.map((v, i) => v - eye[i]));
    const right = unit(cross([bank * 0.1, 1, 0], forward));
    const up = cross(forward, right);
    const tangent = Math.tan(((58 + (6 * (flight.speed - 54)) / 33) * tau) / 720);
    const projection = [(tangent * w) / h, tangent];
    this.camera = { eye, right, up, forward, projection };
    gl.viewport(0, 0, w, h);
    gl.clear(gl.DEPTH_BUFFER_BIT);
    gl.disable(gl.DEPTH_TEST);
    gl.useProgram(sky.handle);
    // The previous frame used three attributes. The sky triangle has a single short buffer.
    for (const loc of [world.aPosition, world.aNormal, world.aColor])
      gl.disableVertexAttribArray(loc);
    gl.bindBuffer(gl.ARRAY_BUFFER, this.background.buffer);
    gl.enableVertexAttribArray(sky.aPoint);
    gl.vertexAttribPointer(sky.aPoint, 2, gl.FLOAT, false, 0, 0);
    gl.uniformMatrix3fv(sky.uBasis, false, [...right, ...up, ...forward]);
    gl.uniform2fv(sky.uScale, projection);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.disableVertexAttribArray(sky.aPoint);
    gl.enable(gl.DEPTH_TEST);
    gl.useProgram(world.handle);
    gl.uniformMatrix3fv(world.uView, false, [
      right[0],
      up[0],
      forward[0],
      right[1],
      up[1],
      forward[1],
      right[2],
      up[2],
      forward[2],
    ]);
    gl.uniform2fv(world.uScale, projection);
    gl.uniform3fv(world.uEye, eye);
    gl.uniform3f(world.uPilot, x, y, z);
    gl.uniform1f(world.uTime, flight.elapsed);
    gl.uniformMatrix3fv(world.uModel, false, identity);
    gl.uniform3f(world.uOrigin, 0, 0, 0);
    for (const loc of [world.aPosition, world.aNormal, world.aColor])
      gl.enableVertexAttribArray(loc);
    for (const section of this.sections) {
      if (section.end < z - 46 || section.start > z + 1000) continue;
      this.drawMesh(section.land, 0);
      this.drawMesh(section.water, 1);
      this.drawMesh(section.foliage, 4);
    }
    for (const gate of GATES.slice(flight.next)) {
      if (gate.z - z > 520) break;
      gl.uniform3f(world.uOrigin, gate.x, gate.y, gate.z);
      this.drawMesh(this.gate, 2);
    }
    const cb = Math.cos(bank),
      sb = Math.sin(bank),
      cp = Math.cos(pitch),
      sp = Math.sin(pitch);
    gl.uniformMatrix3fv(world.uModel, false, [
      cb,
      sb * cp,
      -sb * sp,
      -sb,
      cb * cp,
      -cb * sp,
      0,
      sp,
      cp,
    ]);
    gl.uniform3f(world.uOrigin, x, y, z);
    this.drawMesh(this.glider, 3);
  }

  drawMesh(mesh, material) {
    const { gl, world } = this;
    gl.uniform1i(world.uMaterial, material);
    gl.bindBuffer(gl.ARRAY_BUFFER, mesh.buffer);
    gl.vertexAttribPointer(world.aPosition, 3, gl.FLOAT, false, 36, 0);
    gl.vertexAttribPointer(world.aNormal, 3, gl.FLOAT, false, 36, 12);
    gl.vertexAttribPointer(world.aColor, 3, gl.FLOAT, false, 36, 24);
    gl.drawArrays(gl.TRIANGLES, 0, mesh.count);
  }

  // Normalized screen coordinates anchor the active gate's distance label to the actual 3D hoop.
  project(x, y, z) {
    const { eye, right, up, forward, projection } = this.camera;
    const delta = [x - eye[0], y - eye[1], z - eye[2]],
      depth = dot(delta, forward);
    if (depth <= 0.7) return null;
    return [
      0.5 + (0.5 * dot(delta, right)) / (depth * projection[0]),
      0.5 - (0.5 * dot(delta, up)) / (depth * projection[1]),
    ];
  }
}

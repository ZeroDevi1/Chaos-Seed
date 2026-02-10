import type { BackgroundEffect } from './types'

type GlState = {
  gl: WebGLRenderingContext
  prog: WebGLProgram
  buf: WebGLBuffer
  uniTime: WebGLUniformLocation | null
  uniRes: WebGLUniformLocation | null
}

function compile(gl: WebGLRenderingContext, type: number, src: string): WebGLShader {
  const sh = gl.createShader(type)
  if (!sh) throw new Error('createShader failed')
  gl.shaderSource(sh, src)
  gl.compileShader(sh)
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
    const msg = gl.getShaderInfoLog(sh) || 'shader compile failed'
    gl.deleteShader(sh)
    throw new Error(msg)
  }
  return sh
}

function link(gl: WebGLRenderingContext, vs: WebGLShader, fs: WebGLShader): WebGLProgram {
  const p = gl.createProgram()
  if (!p) throw new Error('createProgram failed')
  gl.attachShader(p, vs)
  gl.attachShader(p, fs)
  gl.linkProgram(p)
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
    const msg = gl.getProgramInfoLog(p) || 'program link failed'
    gl.deleteProgram(p)
    throw new Error(msg)
  }
  return p
}

export class FluidBackgroundEffect implements BackgroundEffect {
  private canvas: HTMLCanvasElement | null = null
  private st: GlState | null = null
  private active = false
  private raf = 0
  private t0 = performance.now()
  private w = 1
  private h = 1

  mount(canvas: HTMLCanvasElement) {
    this.canvas = canvas
    const gl = canvas.getContext('webgl', { alpha: true, antialias: false, preserveDrawingBuffer: false })
    if (!gl) return

    const vs = compile(
      gl,
      gl.VERTEX_SHADER,
      `
attribute vec2 a_pos;
varying vec2 v_uv;
void main() {
  v_uv = (a_pos + 1.0) * 0.5;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
`.trim()
    )
    const fs = compile(
      gl,
      gl.FRAGMENT_SHADER,
      `
precision mediump float;
varying vec2 v_uv;
uniform float u_time;
uniform vec2 u_res;

float hash(vec2 p) {
  return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float noise(vec2 p) {
  vec2 i = floor(p);
  vec2 f = fract(p);
  float a = hash(i);
  float b = hash(i + vec2(1.0, 0.0));
  float c = hash(i + vec2(0.0, 1.0));
  float d = hash(i + vec2(1.0, 1.0));
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}

void main() {
  vec2 uv = v_uv;
  vec2 p = (uv - 0.5) * vec2(u_res.x / max(1.0, u_res.y), 1.0);
  float t = u_time * 0.00025;

  float n = 0.0;
  n += 0.60 * noise(p * 2.0 + vec2(t * 2.0, -t * 1.5));
  n += 0.30 * noise(p * 4.0 + vec2(-t * 3.0, t * 2.0));
  n += 0.10 * noise(p * 8.0 + vec2(t * 4.0, t * 3.0));

  vec3 c1 = vec3(0.10, 0.38, 0.95);
  vec3 c2 = vec3(0.75, 0.10, 0.95);
  vec3 c3 = vec3(0.05, 0.95, 0.75);
  vec3 col = mix(c1, c2, smoothstep(0.2, 0.8, n));
  col = mix(col, c3, 0.35 * sin(n * 6.2831 + t * 2.0) + 0.35);

  float vign = smoothstep(0.95, 0.2, length(p));
  col *= vign;
  gl_FragColor = vec4(col, 0.45);
}
`.trim()
    )

    const prog = link(gl, vs, fs)
    gl.deleteShader(vs)
    gl.deleteShader(fs)

    const buf = gl.createBuffer()
    if (!buf) return
    gl.bindBuffer(gl.ARRAY_BUFFER, buf)
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW)

    gl.useProgram(prog)
    const loc = gl.getAttribLocation(prog, 'a_pos')
    gl.enableVertexAttribArray(loc)
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0)

    this.st = {
      gl,
      prog,
      buf,
      uniTime: gl.getUniformLocation(prog, 'u_time'),
      uniRes: gl.getUniformLocation(prog, 'u_res')
    }
    this.resize(canvas.width, canvas.height)
  }

  resize(width: number, height: number) {
    this.w = Math.max(1, Math.floor(width))
    this.h = Math.max(1, Math.floor(height))
    const st = this.st
    if (!st) return
    st.gl.viewport(0, 0, this.w, this.h)
  }

  setActive(active: boolean) {
    this.active = active
    if (!this.active) {
      cancelAnimationFrame(this.raf)
      return
    }
    const loop = () => {
      this.raf = requestAnimationFrame(loop)
      this.draw()
    }
    cancelAnimationFrame(this.raf)
    this.raf = requestAnimationFrame(loop)
  }

  private draw() {
    const st = this.st
    if (!st || !this.canvas) return
    const gl = st.gl
    gl.useProgram(st.prog)
    if (st.uniTime) gl.uniform1f(st.uniTime, performance.now() - this.t0)
    if (st.uniRes) gl.uniform2f(st.uniRes, this.w, this.h)
    gl.drawArrays(gl.TRIANGLES, 0, 6)
  }

  dispose() {
    cancelAnimationFrame(this.raf)
    this.raf = 0
    const st = this.st
    if (st) {
      try {
        st.gl.deleteBuffer(st.buf)
        st.gl.deleteProgram(st.prog)
      } catch {
        // ignore
      }
    }
    this.st = null
    this.canvas = null
  }
}


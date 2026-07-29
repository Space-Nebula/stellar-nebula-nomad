/**
 * Experimental AR nebula viewer for the Stellar Nebula Nomad mobile shell.
 *
 * Like the WalletConnect bridge, the app is a thin WebView around a hosted
 * web build of the game (see App.tsx), so the AR experience itself runs in
 * web content: WebXR (`immersive-ar`) where the WebView engine supports it,
 * with an ARKit Quick Look fallback on iOS (USDZ model launched through an
 * `<a rel="ar">` anchor, which WKWebView hands to ARKit natively).
 *
 * This module builds the JavaScript injected into the WebView to expose a
 * `window.__stellarAR` bridge: capability detection, session lifecycle, and
 * a deterministic 3D nebula point-cloud renderer seeded by the same u64
 * nebula seed the contract's `nebula_gen` module uses, so the AR view of a
 * nebula matches what the player scanned on-chain. Lifecycle state flows
 * back to native through `ReactNativeWebView.postMessage`, mirroring the
 * wallet bridge message flow.
 *
 * The whole feature ships behind an experimental flag
 * (`EXPO_PUBLIC_EXPERIMENTAL_AR_VIEWER`) and is fully inert when disabled:
 * no script is injected and no messages are produced.
 */

/** Rendering mode the web content selected after capability detection. */
export type ARViewerMode = "webxr" | "arkit-quicklook" | "none";

/** Lifecycle status reported by the injected AR bridge. */
export type ARViewerStatus =
  | "supported"
  | "unsupported"
  | "session_started"
  | "session_ended"
  | "error";

export interface ARViewerMessage {
  type: "ar_viewer";
  status: ARViewerStatus;
  mode: ARViewerMode;
  /** Present on `error` messages — human-readable failure description. */
  detail?: string;
}

export interface ARViewerConfig {
  /**
   * Number of particles in the generated nebula point cloud. Bounded to keep
   * mobile GPU cost predictable; defaults to a mid-range density.
   */
  particleCount?: number;
  /**
   * URL of a USDZ nebula model used for the ARKit Quick Look fallback on
   * iOS WebViews without WebXR. Omit to disable the fallback.
   */
  quickLookModelUrl?: string | null;
}

const DEFAULT_PARTICLE_COUNT = 1500;
const MIN_PARTICLE_COUNT = 100;
const MAX_PARTICLE_COUNT = 10_000;

/**
 * Parses the experimental-flag environment value. Only explicit affirmative
 * values enable the viewer — anything else (unset, "0", "false", garbage)
 * leaves it off, so the flag fails closed.
 */
export function isARViewerEnabled(value: string | null | undefined): boolean {
  if (!value) return false;
  switch (value.trim().toLowerCase()) {
    case "1":
    case "true":
    case "yes":
    case "on":
      return true;
    default:
      return false;
  }
}

/** Clamps a configured particle count into the supported density range. */
export function clampParticleCount(count: number | undefined): number {
  if (count === undefined || !Number.isFinite(count)) {
    return DEFAULT_PARTICLE_COUNT;
  }
  const n = Math.floor(count);
  if (n < MIN_PARTICLE_COUNT) return MIN_PARTICLE_COUNT;
  if (n > MAX_PARTICLE_COUNT) return MAX_PARTICLE_COUNT;
  return n;
}

/**
 * Parses a WebView `onMessage` payload and returns the AR viewer message if
 * it is one, or null for anything else (wallet messages, malformed JSON).
 */
export function parseARViewerMessage(
  data: string | null | undefined,
): ARViewerMessage | null {
  if (!data) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(data);
  } catch {
    return null;
  }
  if (typeof parsed !== "object" || parsed === null) return null;
  const record = parsed as Record<string, unknown>;
  if (record.type !== "ar_viewer") return null;

  const status = record.status;
  if (
    status !== "supported" &&
    status !== "unsupported" &&
    status !== "session_started" &&
    status !== "session_ended" &&
    status !== "error"
  ) {
    return null;
  }

  const mode =
    record.mode === "webxr" || record.mode === "arkit-quicklook"
      ? record.mode
      : "none";

  const message: ARViewerMessage = { type: "ar_viewer", status, mode };
  if (typeof record.detail === "string") message.detail = record.detail;
  return message;
}

/**
 * Builds the JavaScript injected into the WebView when the experimental AR
 * flag is on. The script is idempotent (safe to inject on every page load)
 * and self-contained: capability detection runs immediately, and the game
 * frontend drives sessions through `window.__stellarAR.startSession(seed)`.
 */
export function buildARViewerInjection(config: ARViewerConfig = {}): string {
  const particleCount = clampParticleCount(config.particleCount);
  const quickLookModelUrl = config.quickLookModelUrl ?? null;

  return `
(function() {
  if (window.__stellarAR) return;

  function post(status, mode, detail) {
    if (window.ReactNativeWebView && window.ReactNativeWebView.postMessage) {
      var payload = { type: "ar_viewer", status: status, mode: mode };
      if (detail) payload.detail = detail;
      window.ReactNativeWebView.postMessage(JSON.stringify(payload));
    }
  }

  // Deterministic PRNG (mulberry32) so a nebula seed always renders the
  // same cloud — mirrors the determinism of the on-chain nebula_gen module.
  function mulberry32(seed) {
    var a = seed >>> 0;
    return function() {
      a = (a + 0x6D2B79F5) >>> 0;
      var t = a;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  // Generates a nebula point cloud: positions clustered into a squashed
  // ellipsoid with spiral arms, plus per-particle color drifting between
  // the nebula's two seed-derived hues.
  function generatePointCloud(seed, count) {
    var rand = mulberry32(seed);
    var positions = new Float32Array(count * 3);
    var colors = new Float32Array(count * 3);
    var hueA = rand(), hueB = rand();
    for (var i = 0; i < count; i++) {
      var arm = rand() * Math.PI * 2;
      var radius = Math.pow(rand(), 0.5) * 0.5;
      var swirl = radius * 4.0;
      var x = Math.cos(arm + swirl) * radius;
      var z = Math.sin(arm + swirl) * radius;
      var y = (rand() - 0.5) * 0.18 * (1.0 - radius);
      positions[i * 3] = x;
      positions[i * 3 + 1] = y;
      positions[i * 3 + 2] = z;
      var mix = rand();
      colors[i * 3] = 0.3 + 0.7 * (hueA * (1 - mix) + hueB * mix);
      colors[i * 3 + 1] = 0.2 + 0.4 * mix;
      colors[i * 3 + 2] = 0.5 + 0.5 * (hueB * (1 - mix) + hueA * mix);
    }
    return { positions: positions, colors: colors };
  }

  var VERTEX_SHADER =
    "attribute vec3 position;" +
    "attribute vec3 color;" +
    "uniform mat4 projection;" +
    "uniform mat4 view;" +
    "uniform mat4 model;" +
    "varying vec3 vColor;" +
    "void main() {" +
    "  vColor = color;" +
    "  gl_Position = projection * view * model * vec4(position, 1.0);" +
    "  gl_PointSize = 6.0 / gl_Position.w;" +
    "}";

  var FRAGMENT_SHADER =
    "precision mediump float;" +
    "varying vec3 vColor;" +
    "void main() {" +
    "  vec2 c = gl_PointCoord - vec2(0.5);" +
    "  float d = length(c);" +
    "  if (d > 0.5) discard;" +
    "  gl_FragColor = vec4(vColor, (1.0 - d * 2.0) * 0.85);" +
    "}";

  function compileProgram(gl) {
    function shader(type, source) {
      var s = gl.createShader(type);
      gl.shaderSource(s, source);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
        throw new Error("AR shader compile failed: " + gl.getShaderInfoLog(s));
      }
      return s;
    }
    var program = gl.createProgram();
    gl.attachShader(program, shader(gl.VERTEX_SHADER, VERTEX_SHADER));
    gl.attachShader(program, shader(gl.FRAGMENT_SHADER, FRAGMENT_SHADER));
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      throw new Error("AR shader link failed: " + gl.getProgramInfoLog(program));
    }
    return program;
  }

  // Model matrix: place the nebula ~1m in front of the viewer at session
  // start, gently rotating. Column-major, as WebGL expects.
  function modelMatrix(angle) {
    var c = Math.cos(angle), s = Math.sin(angle);
    return new Float32Array([
      c, 0, -s, 0,
      0, 1, 0, 0,
      s, 0, c, 0,
      0, -0.1, -1.0, 1,
    ]);
  }

  var state = {
    session: null,
    gl: null,
    referenceSpace: null,
    program: null,
    buffers: null,
    particleCount: ${particleCount},
    startedAt: 0,
  };

  function endSessionCleanup() {
    state.session = null;
    state.gl = null;
    state.referenceSpace = null;
    state.program = null;
    state.buffers = null;
    post("session_ended", "webxr");
  }

  function onXRFrame(time, frame) {
    var session = state.session;
    if (!session) return;
    session.requestAnimationFrame(onXRFrame);

    var pose = frame.getViewerPose(state.referenceSpace);
    if (!pose) return;

    var gl = state.gl;
    var layer = session.renderState.baseLayer;
    gl.bindFramebuffer(gl.FRAMEBUFFER, layer.framebuffer);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE);

    gl.useProgram(state.program);
    var model = modelMatrix((time - state.startedAt) / 12000);

    for (var v = 0; v < pose.views.length; v++) {
      var view = pose.views[v];
      var viewport = layer.getViewport(view);
      gl.viewport(viewport.x, viewport.y, viewport.width, viewport.height);

      gl.uniformMatrix4fv(
        gl.getUniformLocation(state.program, "projection"), false, view.projectionMatrix);
      gl.uniformMatrix4fv(
        gl.getUniformLocation(state.program, "view"), false, view.transform.inverse.matrix);
      gl.uniformMatrix4fv(
        gl.getUniformLocation(state.program, "model"), false, model);

      gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.position);
      var posLoc = gl.getAttribLocation(state.program, "position");
      gl.enableVertexAttribArray(posLoc);
      gl.vertexAttribPointer(posLoc, 3, gl.FLOAT, false, 0, 0);

      gl.bindBuffer(gl.ARRAY_BUFFER, state.buffers.color);
      var colLoc = gl.getAttribLocation(state.program, "color");
      gl.enableVertexAttribArray(colLoc);
      gl.vertexAttribPointer(colLoc, 3, gl.FLOAT, false, 0, 0);

      gl.drawArrays(gl.POINTS, 0, state.particleCount);
    }
  }

  window.__stellarAR = {
    experimental: true,
    mode: "none",
    quickLookModelUrl: ${JSON.stringify(quickLookModelUrl)},

    // Resolved by detect(); cached so the game frontend can gate its AR UI.
    ready: null,

    detect: function() {
      var self = this;
      if (!this.ready) {
        this.ready = new Promise(function(resolve) {
          if (navigator.xr && navigator.xr.isSessionSupported) {
            navigator.xr.isSessionSupported("immersive-ar").then(function(ok) {
              if (ok) {
                self.mode = "webxr";
                post("supported", "webxr");
              } else if (self.quickLookModelUrl && self.isQuickLookAvailable()) {
                self.mode = "arkit-quicklook";
                post("supported", "arkit-quicklook");
              } else {
                post("unsupported", "none");
              }
              resolve(self.mode);
            }).catch(function() {
              post("unsupported", "none");
              resolve("none");
            });
          } else if (self.quickLookModelUrl && self.isQuickLookAvailable()) {
            self.mode = "arkit-quicklook";
            post("supported", "arkit-quicklook");
            resolve(self.mode);
          } else {
            post("unsupported", "none");
            resolve("none");
          }
        });
      }
      return this.ready;
    },

    // ARKit Quick Look is available when WKWebView advertises AR support on
    // <a rel="ar"> anchors (iOS 12+). Feature-detected, never UA-sniffed.
    isQuickLookAvailable: function() {
      var a = document.createElement("a");
      return a.relList && a.relList.supports && a.relList.supports("ar");
    },

    // Launches the AR experience for a scanned nebula. "seed" is the u64
    // nebula seed from the contract, passed as a decimal string or number;
    // only the low 32 bits feed the PRNG.
    startSession: function(seed) {
      var self = this;
      return this.detect().then(function(mode) {
        if (mode === "arkit-quicklook") {
          var anchor = document.createElement("a");
          anchor.setAttribute("rel", "ar");
          anchor.setAttribute("href", self.quickLookModelUrl);
          anchor.appendChild(document.createElement("img"));
          document.body.appendChild(anchor);
          anchor.click();
          document.body.removeChild(anchor);
          post("session_started", "arkit-quicklook");
          return true;
        }
        if (mode !== "webxr") {
          post("error", "none", "AR is not supported on this device");
          return false;
        }
        if (state.session) return true;

        var numericSeed = typeof seed === "number" ? seed : parseInt(seed, 10);
        if (!isFinite(numericSeed)) numericSeed = 0;

        return navigator.xr.requestSession("immersive-ar", {
          requiredFeatures: ["local"],
        }).then(function(session) {
          state.session = session;
          var canvas = document.createElement("canvas");
          var gl = canvas.getContext("webgl", { xrCompatible: true, alpha: true });
          if (!gl) throw new Error("WebGL unavailable");
          state.gl = gl;
          state.program = compileProgram(gl);

          var cloud = generatePointCloud(numericSeed, state.particleCount);
          var posBuf = gl.createBuffer();
          gl.bindBuffer(gl.ARRAY_BUFFER, posBuf);
          gl.bufferData(gl.ARRAY_BUFFER, cloud.positions, gl.STATIC_DRAW);
          var colBuf = gl.createBuffer();
          gl.bindBuffer(gl.ARRAY_BUFFER, colBuf);
          gl.bufferData(gl.ARRAY_BUFFER, cloud.colors, gl.STATIC_DRAW);
          state.buffers = { position: posBuf, color: colBuf };

          session.updateRenderState({
            baseLayer: new XRWebGLLayer(session, gl),
          });
          session.addEventListener("end", endSessionCleanup);
          return session.requestReferenceSpace("local").then(function(space) {
            state.referenceSpace = space;
            state.startedAt = performance.now();
            session.requestAnimationFrame(onXRFrame);
            post("session_started", "webxr");
            return true;
          });
        }).catch(function(err) {
          post("error", "webxr", String(err && err.message ? err.message : err));
          return false;
        });
      });
    },

    endSession: function() {
      if (state.session) {
        state.session.end();
      }
    },
  };

  window.__stellarAR.detect();
})();
true;
`;
}

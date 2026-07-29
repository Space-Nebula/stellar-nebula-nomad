import {
  buildARViewerInjection,
  clampParticleCount,
  isARViewerEnabled,
  parseARViewerMessage,
} from "./ar-nebula-viewer";

describe("isARViewerEnabled", () => {
  it.each(["1", "true", "TRUE", "yes", "on", " on "])(
    "enables for affirmative value %p",
    (value) => {
      expect(isARViewerEnabled(value)).toBe(true);
    },
  );

  it.each([undefined, null, "", "0", "false", "off", "no", "enabled?"])(
    "stays disabled for %p (fails closed)",
    (value) => {
      expect(isARViewerEnabled(value)).toBe(false);
    },
  );
});

describe("clampParticleCount", () => {
  it("defaults when unset or non-finite", () => {
    expect(clampParticleCount(undefined)).toBe(1500);
    expect(clampParticleCount(NaN)).toBe(1500);
    expect(clampParticleCount(Infinity)).toBe(1500);
  });

  it("clamps into the supported range", () => {
    expect(clampParticleCount(1)).toBe(100);
    expect(clampParticleCount(999999)).toBe(10000);
    expect(clampParticleCount(2500.7)).toBe(2500);
  });
});

describe("parseARViewerMessage", () => {
  it("parses a well-formed lifecycle message", () => {
    const message = parseARViewerMessage(
      JSON.stringify({ type: "ar_viewer", status: "session_started", mode: "webxr" }),
    );
    expect(message).toEqual({
      type: "ar_viewer",
      status: "session_started",
      mode: "webxr",
    });
  });

  it("carries the error detail through", () => {
    const message = parseARViewerMessage(
      JSON.stringify({
        type: "ar_viewer",
        status: "error",
        mode: "webxr",
        detail: "WebGL unavailable",
      }),
    );
    expect(message?.detail).toBe("WebGL unavailable");
  });

  it("normalizes unknown modes to none", () => {
    const message = parseARViewerMessage(
      JSON.stringify({ type: "ar_viewer", status: "unsupported", mode: "hololens" }),
    );
    expect(message?.mode).toBe("none");
  });

  it("ignores other bridge messages", () => {
    expect(
      parseARViewerMessage(
        JSON.stringify({ type: "wallet_connect_session", status: "connected" }),
      ),
    ).toBeNull();
  });

  it("ignores malformed payloads", () => {
    expect(parseARViewerMessage(undefined)).toBeNull();
    expect(parseARViewerMessage("")).toBeNull();
    expect(parseARViewerMessage("not json")).toBeNull();
    expect(parseARViewerMessage(JSON.stringify(null))).toBeNull();
    expect(parseARViewerMessage(JSON.stringify({ type: "ar_viewer" }))).toBeNull();
    expect(
      parseARViewerMessage(
        JSON.stringify({ type: "ar_viewer", status: "launching" }),
      ),
    ).toBeNull();
  });
});

describe("buildARViewerInjection", () => {
  it("is idempotent and self-detecting", () => {
    const script = buildARViewerInjection();
    expect(script).toContain("if (window.__stellarAR) return;");
    expect(script).toContain("window.__stellarAR.detect();");
  });

  it("embeds the clamped particle count", () => {
    expect(buildARViewerInjection({ particleCount: 500000 })).toContain(
      "particleCount: 10000,",
    );
    expect(buildARViewerInjection()).toContain("particleCount: 1500,");
  });

  it("embeds the Quick Look model URL as a JSON string literal", () => {
    const script = buildARViewerInjection({
      quickLookModelUrl: "https://cdn.example.com/nebula.usdz",
    });
    expect(script).toContain(
      'quickLookModelUrl: "https://cdn.example.com/nebula.usdz",',
    );
  });

  it("disables the Quick Look fallback when no model URL is configured", () => {
    expect(buildARViewerInjection()).toContain("quickLookModelUrl: null,");
  });

  it("requests an immersive-ar WebXR session", () => {
    const script = buildARViewerInjection();
    expect(script).toContain('navigator.xr.isSessionSupported("immersive-ar")');
    expect(script).toContain('navigator.xr.requestSession("immersive-ar"');
  });

  it("reports lifecycle over the ReactNativeWebView message channel", () => {
    const script = buildARViewerInjection();
    expect(script).toContain('type: "ar_viewer"');
    expect(script).toContain("window.ReactNativeWebView.postMessage");
  });
});

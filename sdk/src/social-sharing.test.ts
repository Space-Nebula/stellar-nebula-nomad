import {
  AchievementShare,
  DocumentLike,
  applyOpenGraphMeta,
  buildAchievementPreviewDataUri,
  buildAchievementPreviewSvg,
  buildAchievementShareText,
  buildAchievementUrl,
  buildOpenGraphMeta,
  buildShareLinks,
  renderOpenGraphMetaHtml,
  shareAchievement,
} from "./social-sharing";

const SHARE: AchievementShare = {
  id: "first-wormhole-jump",
  title: "First Wormhole Jump",
  description: "Traversed a wormhole and lived to tell the tale.",
  playerName: "NomadOne",
  rarity: "rare",
};

describe("buildAchievementUrl", () => {
  it("derives the canonical URL from the site and achievement id", () => {
    expect(buildAchievementUrl(SHARE, { siteUrl: "https://example.app/" })).toBe(
      "https://example.app/achievements/first-wormhole-jump",
    );
  });

  it("percent-encodes unsafe achievement ids", () => {
    expect(
      buildAchievementUrl(
        { id: "spaced id/1", title: "T" },
        { siteUrl: "https://example.app" },
      ),
    ).toBe("https://example.app/achievements/spaced%20id%2F1");
  });

  it("prefers an explicit share URL", () => {
    expect(
      buildAchievementUrl({ ...SHARE, url: "https://x.app/a/1" }),
    ).toBe("https://x.app/a/1");
  });
});

describe("buildAchievementShareText", () => {
  it("includes player, rarity, title, and description", () => {
    expect(buildAchievementShareText(SHARE)).toBe(
      'NomadOne has unlocked the rare achievement "First Wormhole Jump" in ' +
        "Stellar Nebula Nomad! Traversed a wormhole and lived to tell the tale.",
    );
  });

  it("falls back to first person without a player name", () => {
    expect(
      buildAchievementShareText({ id: "a", title: "Stargazer" }),
    ).toBe('I unlocked the achievement "Stargazer" in Stellar Nebula Nomad!');
  });
});

describe("buildShareLinks", () => {
  const links = buildShareLinks(SHARE, {
    siteUrl: "https://example.app",
    hashtags: ["Nebula"],
  });

  it("builds an X intent with encoded text, url, and hashtags", () => {
    expect(links.x).toContain("https://twitter.com/intent/tweet?text=");
    expect(links.x).toContain(
      encodeURIComponent("https://example.app/achievements/first-wormhole-jump"),
    );
    expect(links.x).toContain("&hashtags=Nebula");
    expect(links.x).not.toContain('"First');
  });

  it("builds sharer links for the other platforms", () => {
    const encodedUrl = encodeURIComponent(
      "https://example.app/achievements/first-wormhole-jump",
    );
    expect(links.facebook).toBe(
      `https://www.facebook.com/sharer/sharer.php?u=${encodedUrl}`,
    );
    expect(links.telegram).toContain(`url=${encodedUrl}`);
    expect(links.whatsapp).toContain("https://wa.me/?text=");
    expect(links.reddit).toContain(`url=${encodedUrl}`);
    expect(links.reddit).toContain(
      encodeURIComponent("Unlocked: First Wormhole Jump"),
    );
  });

  it("omits the hashtags parameter when the list is empty", () => {
    const bare = buildShareLinks(SHARE, { hashtags: [] });
    expect(bare.x).not.toContain("hashtags=");
  });
});

describe("buildOpenGraphMeta", () => {
  const tags = buildOpenGraphMeta(SHARE, {
    siteUrl: "https://example.app",
    siteName: "Nebula Nomad",
  });
  const byKey = Object.fromEntries(tags.map((t) => [t.key, t]));

  it("emits the core Open Graph tags", () => {
    expect(byKey["og:title"].content).toBe(
      "Achievement unlocked: First Wormhole Jump",
    );
    expect(byKey["og:site_name"].content).toBe("Nebula Nomad");
    expect(byKey["og:url"].content).toBe(
      "https://example.app/achievements/first-wormhole-jump",
    );
    expect(byKey["og:description"].content).toBe(SHARE.description);
    expect(byKey["og:image:width"].content).toBe("1200");
    expect(byKey["og:image:height"].content).toBe("630");
  });

  it("emits a large-image Twitter card mirroring the OG tags", () => {
    expect(byKey["twitter:card"].content).toBe("summary_large_image");
    expect(byKey["twitter:card"].attribute).toBe("name");
    expect(byKey["og:title"].attribute).toBe("property");
    expect(byKey["twitter:image"].content).toBe(byKey["og:image"].content);
  });

  it("defaults og:image to the generated preview data URI", () => {
    expect(byKey["og:image"].content).toBe(
      buildAchievementPreviewDataUri(SHARE),
    );
  });

  it("prefers an explicit image URL", () => {
    const withImage = buildOpenGraphMeta({
      ...SHARE,
      imageUrl: "https://cdn.example.app/previews/1.png",
    });
    expect(withImage.find((t) => t.key === "og:image")?.content).toBe(
      "https://cdn.example.app/previews/1.png",
    );
  });
});

describe("renderOpenGraphMetaHtml", () => {
  it("serializes tags with escaped content", () => {
    const html = renderOpenGraphMetaHtml([
      {
        attribute: "property",
        key: "og:title",
        content: 'Say "hi" <&> more',
      },
    ]);
    expect(html).toBe(
      '<meta property="og:title" content="Say &quot;hi&quot; &lt;&amp;&gt; more">',
    );
  });
});

describe("applyOpenGraphMeta", () => {
  function fakeDocument() {
    const elements: Array<{
      attrs: Record<string, string>;
      setAttribute(name: string, value: string): void;
    }> = [];
    const doc = {
      head: {
        querySelector: (selector: string) => {
          const match = /meta\[(\w+)="([^"]+)"\]/.exec(selector);
          if (!match) return null;
          return (
            elements.find((el) => el.attrs[match[1]] === match[2]) ?? null
          );
        },
        appendChild: (el: any) => elements.push(el),
      },
      createElement: () => {
        const el = {
          attrs: {} as Record<string, string>,
          setAttribute(name: string, value: string) {
            this.attrs[name] = value;
          },
        };
        return el;
      },
    };
    return { doc: doc as DocumentLike, elements };
  }

  it("creates tags and updates them in place on re-apply", () => {
    const { doc, elements } = fakeDocument();
    applyOpenGraphMeta(
      [{ attribute: "property", key: "og:title", content: "one" }],
      doc,
    );
    expect(elements).toHaveLength(1);
    expect(elements[0].attrs.content).toBe("one");

    applyOpenGraphMeta(
      [{ attribute: "property", key: "og:title", content: "two" }],
      doc,
    );
    expect(elements).toHaveLength(1);
    expect(elements[0].attrs.content).toBe("two");
  });

  it("no-ops without a document", () => {
    expect(() =>
      applyOpenGraphMeta(
        [{ attribute: "property", key: "og:title", content: "x" }],
        undefined,
      ),
    ).not.toThrow();
  });
});

describe("buildAchievementPreviewSvg", () => {
  it("is deterministic for the same achievement", () => {
    expect(buildAchievementPreviewSvg(SHARE)).toBe(
      buildAchievementPreviewSvg({ ...SHARE }),
    );
  });

  it("varies the star field by achievement id", () => {
    expect(buildAchievementPreviewSvg(SHARE)).not.toBe(
      buildAchievementPreviewSvg({ ...SHARE, id: "other-achievement" }),
    );
  });

  it("renders at Open Graph dimensions with rarity accent and title", () => {
    const svg = buildAchievementPreviewSvg(SHARE);
    expect(svg).toContain('width="1200" height="630"');
    expect(svg).toContain("#4da3ff"); // rare accent
    expect(svg).toContain("First Wormhole Jump");
    expect(svg).toContain("RARE ACHIEVEMENT");
    expect(svg).toContain("Unlocked by NomadOne");
  });

  it("escapes markup in user-provided fields", () => {
    const svg = buildAchievementPreviewSvg({
      id: "x",
      title: '<script>"pwn"</script>',
    });
    expect(svg).not.toContain("<script>");
    expect(svg).toContain("&lt;script&gt;");
  });
});

describe("buildAchievementPreviewDataUri", () => {
  it("wraps the SVG in an encoded data URI", () => {
    const uri = buildAchievementPreviewDataUri(SHARE);
    expect(uri.startsWith("data:image/svg+xml,")).toBe(true);
    expect(decodeURIComponent(uri.slice("data:image/svg+xml,".length))).toBe(
      buildAchievementPreviewSvg(SHARE),
    );
  });
});

describe("shareAchievement", () => {
  it("shares through the Web Share API when available", async () => {
    const share = jest.fn().mockResolvedValue(undefined);
    await expect(
      shareAchievement(SHARE, { siteUrl: "https://example.app" }, { share }),
    ).resolves.toBe(true);
    expect(share).toHaveBeenCalledWith({
      title: "Achievement unlocked: First Wormhole Jump",
      text: buildAchievementShareText(SHARE),
      url: "https://example.app/achievements/first-wormhole-jump",
    });
  });

  it("returns false when the API is unavailable", async () => {
    await expect(shareAchievement(SHARE, {}, undefined)).resolves.toBe(false);
    await expect(shareAchievement(SHARE, {}, {})).resolves.toBe(false);
  });

  it("returns false when the user dismisses the sheet", async () => {
    const share = jest.fn().mockRejectedValue(new Error("AbortError"));
    await expect(shareAchievement(SHARE, {}, { share })).resolves.toBe(false);
  });
});

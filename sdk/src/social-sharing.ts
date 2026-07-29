/**
 * Social sharing for the game frontend: achievement share links, Open Graph
 * meta tags for crawler-facing achievement pages, and deterministic SVG
 * preview images.
 *
 * Everything here is pure and environment-agnostic — DOM and Web Share API
 * entry points take their globals as injectable parameters so the module
 * works (and tests) identically in browsers, the mobile WebView, and Node
 * (e.g. a server rendering Open Graph tags for link unfurling).
 */

/** Achievement rarity tiers, matching the contract's achievement engine. */
export type AchievementRarity =
  | "common"
  | "uncommon"
  | "rare"
  | "epic"
  | "legendary";

export interface AchievementShare {
  /** Achievement identifier, used to build canonical share URLs. */
  id: string;
  /** Display title, e.g. "First Wormhole Jump". */
  title: string;
  /** Optional flavor/description line. */
  description?: string;
  /** Player display name shown in share copy when present. */
  playerName?: string;
  rarity?: AchievementRarity;
  /**
   * Canonical achievement page URL. When omitted it is derived from
   * `siteUrl` + `/achievements/` + the achievement id.
   */
  url?: string;
  /** Absolute preview image URL. Defaults to the generated SVG data URI. */
  imageUrl?: string;
}

export interface OpenGraphTag {
  /** Attribute the tag keys on: `property` for og:*, `name` for twitter:*. */
  attribute: "property" | "name";
  key: string;
  content: string;
}

export interface SocialShareOptions {
  /** Site origin used for canonical URLs, e.g. "https://nebula-nomad.app". */
  siteUrl?: string;
  /** Site display name for og:site_name. */
  siteName?: string;
  /** Hashtags (without `#`) appended to X/Twitter shares. */
  hashtags?: string[];
}

export interface ShareLinks {
  x: string;
  facebook: string;
  telegram: string;
  whatsapp: string;
  reddit: string;
}

const DEFAULT_SITE_URL = "https://stellar-nebula-nomad.app";
const DEFAULT_SITE_NAME = "Stellar Nebula Nomad";
const DEFAULT_HASHTAGS = ["StellarNebulaNomad", "Stellar"];

const RARITY_COLORS: Record<AchievementRarity, string> = {
  common: "#8fa3c8",
  uncommon: "#4dd08a",
  rare: "#4da3ff",
  epic: "#b06dff",
  legendary: "#ffb84d",
};

/** Resolves the canonical achievement page URL for a share. */
export function buildAchievementUrl(
  share: AchievementShare,
  options: SocialShareOptions = {},
): string {
  if (share.url) return share.url;
  const base = (options.siteUrl ?? DEFAULT_SITE_URL).replace(/\/+$/, "");
  return `${base}/achievements/${encodeURIComponent(share.id)}`;
}

/** Builds the human-readable share copy for an unlocked achievement. */
export function buildAchievementShareText(share: AchievementShare): string {
  const rarity = share.rarity ? `${share.rarity} ` : "";
  const who = share.playerName ?? "I";
  const suffix = who === "I" ? "" : " has";
  const base = `${who}${suffix} unlocked the ${rarity}achievement "${share.title}" in Stellar Nebula Nomad!`;
  return share.description ? `${base} ${share.description}` : base;
}

/**
 * Builds prefilled share URLs for the major social platforms. All parameter
 * values are percent-encoded; callers open these in a new tab/window.
 */
export function buildShareLinks(
  share: AchievementShare,
  options: SocialShareOptions = {},
): ShareLinks {
  const url = buildAchievementUrl(share, options);
  const text = buildAchievementShareText(share);
  const hashtags = options.hashtags ?? DEFAULT_HASHTAGS;

  const encodedUrl = encodeURIComponent(url);
  const encodedText = encodeURIComponent(text);

  return {
    x:
      `https://twitter.com/intent/tweet?text=${encodedText}&url=${encodedUrl}` +
      (hashtags.length
        ? `&hashtags=${encodeURIComponent(hashtags.join(","))}`
        : ""),
    facebook: `https://www.facebook.com/sharer/sharer.php?u=${encodedUrl}`,
    telegram: `https://t.me/share/url?url=${encodedUrl}&text=${encodedText}`,
    whatsapp: `https://wa.me/?text=${encodeURIComponent(`${text} ${url}`)}`,
    reddit: `https://www.reddit.com/submit?url=${encodedUrl}&title=${encodeURIComponent(
      `Unlocked: ${share.title}`,
    )}`,
  };
}

/**
 * Builds the Open Graph + Twitter Card meta tags for an achievement page so
 * shared links unfurl with a rich preview. Render server-side with
 * `renderOpenGraphMetaHtml` or apply client-side with `applyOpenGraphMeta`.
 */
export function buildOpenGraphMeta(
  share: AchievementShare,
  options: SocialShareOptions = {},
): OpenGraphTag[] {
  const url = buildAchievementUrl(share, options);
  const image = share.imageUrl ?? buildAchievementPreviewDataUri(share);
  const description =
    share.description ?? buildAchievementShareText(share);

  return [
    { attribute: "property", key: "og:type", content: "website" },
    {
      attribute: "property",
      key: "og:site_name",
      content: options.siteName ?? DEFAULT_SITE_NAME,
    },
    {
      attribute: "property",
      key: "og:title",
      content: `Achievement unlocked: ${share.title}`,
    },
    { attribute: "property", key: "og:description", content: description },
    { attribute: "property", key: "og:url", content: url },
    { attribute: "property", key: "og:image", content: image },
    { attribute: "property", key: "og:image:width", content: "1200" },
    { attribute: "property", key: "og:image:height", content: "630" },
    { attribute: "name", key: "twitter:card", content: "summary_large_image" },
    {
      attribute: "name",
      key: "twitter:title",
      content: `Achievement unlocked: ${share.title}`,
    },
    { attribute: "name", key: "twitter:description", content: description },
    { attribute: "name", key: "twitter:image", content: image },
  ];
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Serializes Open Graph tags to HTML `<meta>` elements (server rendering). */
export function renderOpenGraphMetaHtml(tags: OpenGraphTag[]): string {
  return tags
    .map(
      (tag) =>
        `<meta ${tag.attribute}="${escapeHtml(tag.key)}" content="${escapeHtml(
          tag.content,
        )}">`,
    )
    .join("\n");
}

/**
 * Minimal structural slice of the DOM used by `applyOpenGraphMeta`, so the
 * SDK compiles without the DOM lib and tests can pass fakes.
 */
export interface MetaElementLike {
  setAttribute(name: string, value: string): void;
}

export interface DocumentLike {
  head: {
    querySelector(selector: string): MetaElementLike | null;
    appendChild(element: MetaElementLike): void;
  } | null;
  createElement(tagName: string): MetaElementLike;
}

/**
 * Applies Open Graph tags to a live document, replacing any existing tag
 * with the same key so repeated shares don't accumulate duplicates.
 * No-ops outside a DOM environment.
 */
export function applyOpenGraphMeta(
  tags: OpenGraphTag[],
  doc: DocumentLike | undefined = (globalThis as { document?: DocumentLike })
    .document,
): void {
  if (!doc || !doc.head) return;
  for (const tag of tags) {
    let element = doc.head.querySelector(
      `meta[${tag.attribute}="${tag.key}"]`,
    );
    if (!element) {
      element = doc.createElement("meta");
      element.setAttribute(tag.attribute, tag.key);
      doc.head.appendChild(element);
    }
    element.setAttribute("content", tag.content);
  }
}

/**
 * Renders a deterministic 1200x630 SVG preview image for an achievement:
 * dark nebula backdrop with a seed-derived star field (seeded by the
 * achievement id, so the same achievement always produces the same image),
 * rarity-colored accent, title, and player line.
 */
export function buildAchievementPreviewSvg(share: AchievementShare): string {
  const rarity = share.rarity ?? "common";
  const accent = RARITY_COLORS[rarity];

  // Small deterministic hash → PRNG over the achievement id (FNV-1a 32-bit).
  let seed = 0x811c9dc5;
  for (let i = 0; i < share.id.length; i++) {
    seed ^= share.id.charCodeAt(i);
    seed = Math.imul(seed, 0x01000193) >>> 0;
  }
  const next = () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 0x100000000;
  };

  let stars = "";
  for (let i = 0; i < 80; i++) {
    const cx = (next() * 1200).toFixed(1);
    const cy = (next() * 630).toFixed(1);
    const r = (0.5 + next() * 1.8).toFixed(2);
    const opacity = (0.2 + next() * 0.8).toFixed(2);
    stars += `<circle cx="${cx}" cy="${cy}" r="${r}" fill="#e8eefc" opacity="${opacity}"/>`;
  }

  const title = escapeHtml(share.title);
  const playerLine = share.playerName
    ? `Unlocked by ${escapeHtml(share.playerName)}`
    : "Achievement unlocked";
  const rarityLabel = rarity.toUpperCase();

  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">` +
    `<defs>` +
    `<radialGradient id="nebula" cx="30%" cy="35%" r="80%">` +
    `<stop offset="0%" stop-color="${accent}" stop-opacity="0.55"/>` +
    `<stop offset="45%" stop-color="#273054" stop-opacity="0.6"/>` +
    `<stop offset="100%" stop-color="#0b1020" stop-opacity="1"/>` +
    `</radialGradient>` +
    `</defs>` +
    `<rect width="1200" height="630" fill="#0b1020"/>` +
    `<rect width="1200" height="630" fill="url(#nebula)"/>` +
    stars +
    `<text x="80" y="300" font-family="Helvetica, Arial, sans-serif" font-size="26" letter-spacing="6" fill="${accent}">${rarityLabel} ACHIEVEMENT</text>` +
    `<text x="80" y="370" font-family="Helvetica, Arial, sans-serif" font-size="56" font-weight="bold" fill="#e8eefc">${title}</text>` +
    `<text x="80" y="425" font-family="Helvetica, Arial, sans-serif" font-size="28" fill="#a8b6da">${playerLine}</text>` +
    `<text x="80" y="560" font-family="Helvetica, Arial, sans-serif" font-size="24" fill="#a8b6da">Stellar Nebula Nomad</text>` +
    `</svg>`
  );
}

/** The SVG preview encoded as a `data:` URI, usable directly in og:image. */
export function buildAchievementPreviewDataUri(
  share: AchievementShare,
): string {
  return `data:image/svg+xml,${encodeURIComponent(
    buildAchievementPreviewSvg(share),
  )}`;
}

/** Minimal slice of the Web Share API used by `shareAchievement`. */
export interface WebShareNavigator {
  share?: (data: { title?: string; text?: string; url?: string }) => Promise<void>;
}

/**
 * Shares an achievement through the native Web Share sheet when available.
 * Returns true when the share sheet was opened and completed, false when
 * the API is unavailable or the user dismissed the sheet — callers should
 * fall back to `buildShareLinks` in that case.
 */
export async function shareAchievement(
  share: AchievementShare,
  options: SocialShareOptions = {},
  nav: WebShareNavigator | undefined = (
    globalThis as { navigator?: WebShareNavigator }
  ).navigator,
): Promise<boolean> {
  if (!nav || typeof nav.share !== "function") return false;
  try {
    await nav.share({
      title: `Achievement unlocked: ${share.title}`,
      text: buildAchievementShareText(share),
      url: buildAchievementUrl(share, options),
    });
    return true;
  } catch {
    // AbortError (user dismissed) or NotAllowedError — treat as "not shared".
    return false;
  }
}

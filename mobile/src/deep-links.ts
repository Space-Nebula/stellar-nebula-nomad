export type DeepLinkType =
  | "wallet_connect_pairing"
  | "transaction_approval"
  | "wallet_callback"
  | "unknown";

export interface DeepLinkRoute {
  type: DeepLinkType;
  uri: string;
  params: Record<string, string>;
}

/**
 * Parses a raw deep link URL and classifies it into a known route type.
 */
export function parseDeepLink(url: string): DeepLinkRoute {
  const trimmed = url.trim();
  const queryStart = trimmed.indexOf("?");
  const query = queryStart !== -1 ? trimmed.slice(queryStart + 1) : "";
  const params = parseQueryParams(query);

  if (trimmed.startsWith("wc:")) {
    return { type: "wallet_connect_pairing", uri: trimmed, params };
  }

  if (trimmed.includes("/approve") || trimmed.includes("/transaction")) {
    return { type: "transaction_approval", uri: trimmed, params };
  }

  if (trimmed.includes("/wallet-connect") || trimmed.includes("/wallet_callback")) {
    return { type: "wallet_callback", uri: trimmed, params };
  }

  const uriParam = params["uri"];
  if (uriParam && uriParam.startsWith("wc:")) {
    return { type: "wallet_connect_pairing", uri: uriParam, params };
  }

  if (trimmed.startsWith("stellar:")) {
    return { type: "wallet_callback", uri: trimmed, params };
  }

  return { type: "unknown", uri: trimmed, params };
}

/**
 * Extracts transaction approval details from a deep link.
 */
export interface TransactionApproval {
  txHash?: string;
  requestId?: string;
  topic?: string;
  chainId?: string;
}

export function parseTransactionApprovalLink(
  url: string,
): TransactionApproval | null {
  const route = parseDeepLink(url);
  if (route.type !== "transaction_approval") return null;

  return {
    txHash: route.params["txHash"] || route.params["tx_hash"],
    requestId: route.params["requestId"] || route.params["id"],
    topic: route.params["topic"],
    chainId: route.params["chainId"] || route.params["chain_id"],
  };
}

/**
 * Builds a JS injection string to notify the WebView about a transaction
 * approval deep link. The web content is expected to handle the event.
 */
export function buildTransactionApprovalInjection(
  approval: TransactionApproval,
): string {
  const safe = JSON.stringify(approval);
  return `
(function() {
  try {
    if (window.__stellarMobile && typeof window.__stellarMobile.onTransactionApproval === "function") {
      window.__stellarMobile.onTransactionApproval(${safe});
    } else {
      window.__stellarMobilePendingTransactionApproval = ${safe};
    }
  } catch (e) {
    console.warn("[stellar-mobile] failed to relay transaction approval:", e);
  }
  true;
})();
`;
}

/**
 * Builds a JS injection string to notify the WebView about a wallet
 * callback redirect (after a user approves/rejects in their wallet app).
 */
export function buildWalletCallbackInjection(
  uri: string,
  params: Record<string, string>,
): string {
  const safeUri = JSON.stringify(uri);
  const safeParams = JSON.stringify(params);
  return `
(function() {
  try {
    if (window.__stellarMobile && typeof window.__stellarMobile.onWalletCallback === "function") {
      window.__stellarMobile.onWalletCallback(${safeUri}, ${safeParams});
    } else {
      window.__stellarMobilePendingWalletCallback = { uri: ${safeUri}, params: ${safeParams} };
    }
  } catch (e) {
    console.warn("[stellar-mobile] failed to relay wallet callback:", e);
  }
  true;
})();
`;
}

function parseQueryParams(query: string): Record<string, string> {
  const params: Record<string, string> = {};
  if (!query) return params;
  const segments = query.split("&");
  for (const segment of segments) {
    if (!segment) continue;
    const eq = segment.indexOf("=");
    const rawKey = eq === -1 ? segment : segment.slice(0, eq);
    const rawValue = eq === -1 ? "" : segment.slice(eq + 1);
    try {
      params[decodeURIComponent(rawKey)] = decodeURIComponent(rawValue);
    } catch {
      params[rawKey] = rawValue;
    }
  }
  return params;
}

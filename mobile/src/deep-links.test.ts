import {
  parseDeepLink,
  parseTransactionApprovalLink,
  buildTransactionApprovalInjection,
  buildWalletCallbackInjection,
} from "./deep-links";

const WC_PAIRING_URI =
  "wc:7f6e504bfad60b485450578e05678ed3e8e8c47bd1e0e024f5e5b9c2a7c5b1c@2?relay-protocol=irn&symKey=deadbeef";

describe("parseDeepLink", () => {
  it("classifies raw wc: URIs as wallet_connect_pairing", () => {
    const route = parseDeepLink(WC_PAIRING_URI);
    expect(route.type).toBe("wallet_connect_pairing");
    expect(route.uri).toBe(WC_PAIRING_URI);
  });

  it("classifies wrapped wc: URIs as wallet_connect_pairing", () => {
    const wrapped = `stellarnebulanomad://wc?uri=${encodeURIComponent(WC_PAIRING_URI)}`;
    const route = parseDeepLink(wrapped);
    expect(route.type).toBe("wallet_connect_pairing");
    expect(route.uri).toBe(WC_PAIRING_URI);
  });

  it("classifies /approve links as transaction_approval", () => {
    const url = "stellarnebulanomad://approve?txHash=abc123&requestId=req_1";
    const route = parseDeepLink(url);
    expect(route.type).toBe("transaction_approval");
    expect(route.params["txHash"]).toBe("abc123");
  });

  it("classifies /transaction links as transaction_approval", () => {
    const url = "https://stellar-nebula-nomad.app/transaction?id=tx_1";
    const route = parseDeepLink(url);
    expect(route.type).toBe("transaction_approval");
  });

  it("classifies /wallet-connect links as wallet_callback", () => {
    const url = "stellarnebulanomad://wallet-connect?topic=abc";
    const route = parseDeepLink(url);
    expect(route.type).toBe("wallet_callback");
  });

  it("classifies stellar: scheme links as wallet_callback", () => {
    const url = "stellar:pay?amount=100&destination=GABC";
    const route = parseDeepLink(url);
    expect(route.type).toBe("wallet_callback");
  });

  it("classifies /wallet_callback links as wallet_callback", () => {
    const url = "stellarnebulanomad://wallet_callback?result=approved";
    const route = parseDeepLink(url);
    expect(route.type).toBe("wallet_callback");
  });

  it("classifies unknown links as unknown", () => {
    const url = "stellarnebulanomad://settings";
    const route = parseDeepLink(url);
    expect(route.type).toBe("unknown");
  });
});

describe("parseTransactionApprovalLink", () => {
  it("parses transaction approval with txHash and requestId", () => {
    const url = "stellarnebulanomad://approve?txHash=0xabc&requestId=req_1";
    const approval = parseTransactionApprovalLink(url);
    expect(approval).not.toBeNull();
    expect(approval!.txHash).toBe("0xabc");
    expect(approval!.requestId).toBe("req_1");
  });

  it("parses transaction approval with topic and chainId", () => {
    const url = "stellarnebulanomad://approve?topic=abc123&chainId=stellar:testnet";
    const approval = parseTransactionApprovalLink(url);
    expect(approval).not.toBeNull();
    expect(approval!.topic).toBe("abc123");
    expect(approval!.chainId).toBe("stellar:testnet");
  });

  it("returns null for non-approval links", () => {
    expect(parseTransactionApprovalLink("stellarnebulanomad://home")).toBeNull();
    expect(parseTransactionApprovalLink(WC_PAIRING_URI)).toBeNull();
  });
});

describe("buildTransactionApprovalInjection", () => {
  it("produces JS that embeds the approval payload", () => {
    const script = buildTransactionApprovalInjection({
      txHash: "0xabc",
      requestId: "req_1",
    });
    expect(script).toContain("window.__stellarMobile.onTransactionApproval");
    expect(script).toContain("0xabc");
    expect(script).toContain("req_1");
  });

  it("handles empty approval gracefully", () => {
    const script = buildTransactionApprovalInjection({});
    expect(script).toContain("onTransactionApproval");
  });
});

describe("buildWalletCallbackInjection", () => {
  it("produces JS that embeds the callback URI and params", () => {
    const script = buildWalletCallbackInjection(
      "stellarnebulanomad://callback",
      { result: "approved", txHash: "0xabc" },
    );
    expect(script).toContain("window.__stellarMobile.onWalletCallback");
    expect(script).toContain("approved");
    expect(script).toContain("0xabc");
  });
});

import express from "express";
import { Server } from "@stellar/stellar-sdk";
import { WebhookManager } from "./webhook-manager";
import { Database } from "./database";
import { ContractEvent } from "./types";
import dotenv from "dotenv";

dotenv.config();

const app = express();
app.use(express.json());

const db = new Database(
  process.env.DATABASE_URL || "postgresql://localhost/webhooks",
);
const webhookManager = new WebhookManager(db);

// Event listener setup
const server = new Server(
  process.env.STELLAR_RPC_URL || "https://soroban-testnet.stellar.org",
);
const contractId = process.env.CONTRACT_ID || "";

/**
 * Register a new webhook
 */
app.post("/webhooks", async (req, res) => {
  try {
    const { url, events, filters } = req.body;

    if (!url || !events || !Array.isArray(events)) {
      return res.status(400).json({ error: "Invalid request body" });
    }

    const webhook = await webhookManager.registerWebhook(url, events, filters);
    res.json({ webhook });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Update webhook
 */
app.patch("/webhooks/:id", async (req, res) => {
  try {
    const { id } = req.params;
    await webhookManager.updateWebhook(id, req.body);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Delete webhook
 */
app.delete("/webhooks/:id", async (req, res) => {
  try {
    const { id } = req.params;
    await webhookManager.deleteWebhook(id);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Get webhook statistics
 */
app.get("/webhooks/:id/stats", async (req, res) => {
  try {
    const { id } = req.params;
    const stats = await webhookManager.getWebhookStats(id);
    res.json(stats);
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Health check
 */
app.get("/health", (req, res) => {
  res.json({ status: "healthy" });
});

/**
 * Listen for contract events
 */
async function startEventListener() {
  console.log("Starting event listener for contract:", contractId);

  let cursor: string | undefined;

  setInterval(async () => {
    try {
      const events = await server.getEvents({
        startLedger: cursor,
        filters: [
          {
            type: "contract",
            contractIds: [contractId],
          },
        ],
      });

      for (const event of events.events) {
        const contractEvent: ContractEvent = {
          type: event.topic[0] || "unknown",
          contractId: event.contractId,
          ledger: event.ledger,
          txHash: event.txHash,
          timestamp: new Date(event.ledgerClosedAt),
          data: event.value,
        };

        await webhookManager.processEvent(contractEvent);
      }

      if (events.latestLedger) {
        cursor = events.latestLedger;
      }
    } catch (error) {
      console.error("Error fetching events:", error);
    }
  }, 5000); // Poll every 5 seconds
}

const PORT = process.env.PORT || 3000;

app.listen(PORT, () => {
  console.log(`Webhook service running on port ${PORT}`);
  startEventListener();
});

export { WebhookManager, Database };

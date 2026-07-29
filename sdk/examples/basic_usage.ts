import { StellarNebulaClient, ResourceType, ShipType } from "../src";
import { Keypair } from "@stellar/stellar-sdk";

async function main() {
  // Initialize the client
  const client = new StellarNebulaClient({
    contractId: "CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
    rpcUrl: "https://soroban-testnet.stellar.org",
    networkPassphrase: "Test SDF Network ; September 2015",
  });

  // Create a caller keypair
  const caller = Keypair.random();

  console.log("Minting a new ship...");
  const mintResult = await client.mintShip(
    caller,
    caller.publicKey(),
    ShipType.Explorer,
  );

  if (mintResult.success) {
    console.log("Ship minted with ID:", mintResult.result);
    
    // Scan a nebula
    const scanResult = await client.scanNebula(caller, BigInt(1));
    if (scanResult.success) {
      console.log("Nebula Layout scanned:", scanResult.result);
    }
    
    // Get balance
    const balance = await client.getResourceBalance(caller.publicKey(), ResourceType.CosmicEssence);
    console.log("Cosmic Essence Balance:", balance);
  } else {
    console.error("Failed to mint ship:", mintResult.error);
  }
}

main().catch(console.error);

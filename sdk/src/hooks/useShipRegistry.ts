import { useState, useCallback } from 'react';
import { StellarNebulaClient, Ship, ShipType, TxResult, Signer } from '../';
import { Keypair } from '@stellar/stellar-sdk';

export function useShipRegistry(client: StellarNebulaClient) {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mintShip = useCallback(
    async (caller: Keypair | Signer, owner: string, shipType: ShipType): Promise<TxResult<bigint>> => {
      setIsLoading(true);
      setError(null);
      try {
        const result = await client.mintShip(caller, owner, shipType);
        if (!result.success) {
          setError(result.error || 'Failed to mint ship');
        }
        return result;
      } catch (err: any) {
        setError(err.message || 'Unknown error');
        return { success: false, error: err.message };
      } finally {
        setIsLoading(false);
      }
    },
    [client]
  );

  const getShip = useCallback(
    async (shipId: bigint): Promise<Ship | null> => {
      setIsLoading(true);
      setError(null);
      try {
        return await client.getShip(shipId);
      } catch (err: any) {
        setError(err.message || 'Unknown error');
        return null;
      } finally {
        setIsLoading(false);
      }
    },
    [client]
  );

  return { mintShip, getShip, isLoading, error };
}

import { useState, useCallback } from 'react';
import { StellarNebulaClient, NebulaLayout, TxResult, Signer } from '../';
import { Keypair } from '@stellar/stellar-sdk';

export function useNebulaScan(client: StellarNebulaClient) {
  const [isScanning, setIsScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const scanNebula = useCallback(
    async (caller: Keypair | Signer, nebulaId: bigint): Promise<TxResult<NebulaLayout>> => {
      setIsScanning(true);
      setError(null);
      try {
        const result = await client.scanNebula(caller, nebulaId);
        if (!result.success) {
          setError(result.error || 'Failed to scan nebula');
        }
        return result;
      } catch (err: any) {
        setError(err.message || 'Unknown error');
        return { success: false, error: err.message };
      } finally {
        setIsScanning(false);
      }
    },
    [client]
  );

  return { scanNebula, isScanning, error };
}

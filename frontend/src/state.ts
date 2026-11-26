export interface WalletState {
  address: string | null;
  isConnected: boolean;
  network: string;
  connect: (address: string, network?: string) => void;
  disconnect: () => void;
}

export interface GameState {
  currentSector: number;
  shipId: string | null;
  scanning: boolean;
  setSector: (sector: number) => void;
  setShipId: (id: string | null) => void;
  setScanning: (status: boolean) => void;
}

export class AppStateStore {
  private wallet: WalletState = {
    address: null,
    isConnected: false,
    network: 'testnet',
    connect: (addr, net = 'testnet') => {
      this.wallet.address = addr;
      this.wallet.isConnected = true;
      this.wallet.network = net;
    },
    disconnect: () => {
      this.wallet.address = null;
      this.wallet.isConnected = false;
    },
  };

  private game: GameState = {
    currentSector: 1,
    shipId: null,
    scanning: false,
    setSector: (sector: number) => {
      this.game.currentSector = sector;
    },
    setShipId: (id: string | null) => {
      this.game.shipId = id;
    },
    setScanning: (status: boolean) => {
      this.game.scanning = status;
    },
  };

  public getWallet(): WalletState {
    return this.wallet;
  }

  public getGame(): GameState {
    return this.game;
  }
}

export const defaultStore = new AppStateStore();

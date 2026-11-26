import React, { useState } from 'react';
import { responsiveImage } from '../utils/assets';
import './Marketplace.css';

export interface MarketplaceItem {
  id: string;
  name: string;
  type: 'resource' | 'nft' | 'item';
  price: number;
  currency: 'credits' | 'tokens' | 'nft';
  seller: string;
  timestamp: number;
  quantity?: number;
  rarity?: string;
  image?: string;
}

export interface PricePoint {
  time: number;
  price: number;
}

export interface Transaction {
  id: string;
  itemName: string;
  type: 'buy' | 'sell';
  price: number;
  quantity: number;
  timestamp: number;
  status: 'completed' | 'pending' | 'failed';
}

export interface WalletInfo {
  credits: number;
  tokens: number;
  nftBalance: number;
}

export interface MarketplaceProps {
  items: MarketplaceItem[];
  wallet: WalletInfo;
  priceHistory?: Record<string, PricePoint[]>;
  transactions: Transaction[];
  onBuy?: (itemId: string, quantity: number) => void;
  onSell?: (itemId: string, quantity: number, price: number) => void;
  onCreateOrder?: (itemType: string, price: number, quantity: number) => void;
}

export const Marketplace: React.FC<MarketplaceProps> = ({
  items,
  wallet,
  priceHistory,
  transactions,
  onBuy,
  onSell,
  onCreateOrder,
}) => {
  const [activeTab, setActiveTab] = useState<'trading' | 'nft' | 'orderbook' | 'history' | 'wallet'>('trading');
  const [selectedItem, setSelectedItem] = useState<MarketplaceItem | null>(null);
  const [quantity, setQuantity] = useState(1);
  const [filterType, setFilterType] = useState<'all' | 'resource' | 'item'>('all');

  const filteredItems = items.filter(item =>
    filterType === 'all' ? item.type !== 'nft' : item.type === filterType
  );

  const nftItems = items.filter(item => item.type === 'nft');

  const TradingTab = () => (
    <div className="marketplace-tab">
      <div className="trading-container">
        <div className="market-filters">
          <button
            className={`filter-btn ${filterType === 'all' ? 'active' : ''}`}
            onClick={() => setFilterType('all')}
          >
            All Items
          </button>
          <button
            className={`filter-btn ${filterType === 'resource' ? 'active' : ''}`}
            onClick={() => setFilterType('resource')}
          >
            Resources
          </button>
          <button
            className={`filter-btn ${filterType === 'item' ? 'active' : ''}`}
            onClick={() => setFilterType('item')}
          >
            Items
          </button>
        </div>

        <div className="market-grid">
          {filteredItems.map(item => (
            <div
              key={item.id}
              className={`market-item ${selectedItem?.id === item.id ? 'selected' : ''}`}
              onClick={() => setSelectedItem(item)}
            >
              <div className="item-header">
                <h4>{item.name}</h4>
                <span className="item-type">{item.type}</span>
              </div>
              <div className="item-price">
                <span className="price-value">{item.price}</span>
                <span className="currency">{item.currency}</span>
              </div>
              {item.quantity && (
                <div className="item-quantity">Stock: {item.quantity}</div>
              )}
              <div className="item-seller">Seller: {item.seller}</div>
            </div>
          ))}
        </div>

        {selectedItem && (
          <div className="item-detail-panel">
            <div className="detail-header">
              <h3>{selectedItem.name}</h3>
              <button className="close-btn" onClick={() => setSelectedItem(null)}>✕</button>
            </div>
            <div className="detail-content">
              <div className="detail-row">
                <span>Price:</span>
                <strong>{selectedItem.price} {selectedItem.currency}</strong>
              </div>
              <div className="detail-row">
                <span>Type:</span>
                <strong>{selectedItem.type}</strong>
              </div>
              <div className="detail-row">
                <span>Seller:</span>
                <strong>{selectedItem.seller}</strong>
              </div>
              {selectedItem.quantity && (
                <div className="detail-row">
                  <span>Available:</span>
                  <strong>{selectedItem.quantity}</strong>
                </div>
              )}

              <div className="quantity-selector">
                <label>Quantity:</label>
                <input
                  type="number"
                  min="1"
                  value={quantity}
                  onChange={(e) => setQuantity(Math.max(1, parseInt(e.target.value) || 1))}
                />
              </div>

              <div className="total-price">
                <span>Total:</span>
                <strong>{selectedItem.price * quantity} {selectedItem.currency}</strong>
              </div>

              <button
                className="buy-btn"
                onClick={() => {
                  onBuy?.(selectedItem.id, quantity);
                  setQuantity(1);
                  setSelectedItem(null);
                }}
              >
                Buy Now
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );

  const NFTTab = () => (
    <div className="marketplace-tab">
      <div className="nft-container">
        <div className="nft-grid">
          {nftItems.map(item => (
            <div key={item.id} className="nft-card">
              <div className="nft-image">
                {item.image && (
                  <img
                    {...responsiveImage(item.image)}
                    alt={item.name}
                    width={640}
                    height={480}
                  />
                )}
                <div className="nft-overlay">
                  <span className="rarity">{item.rarity?.toUpperCase()}</span>
                </div>
              </div>
              <div className="nft-info">
                <h4>{item.name}</h4>
                <div className="nft-price">
                  <span className="price">{item.price}</span>
                  <span className="currency">{item.currency}</span>
                </div>
                <button className="nft-buy-btn" onClick={() => onBuy?.(item.id, 1)}>
                  Buy
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );

  const OrderBookTab = () => (
    <div className="marketplace-tab">
      <div className="orderbook">
        <div className="orderbook-side buy-orders">
          <h3>Buy Orders</h3>
          <div className="orderbook-list">
            {[...items].reverse().slice(0, 5).map((item, i) => (
              <div key={i} className="order-entry">
                <span className="order-price">{item.price}</span>
                <span className="order-quantity">{item.quantity || 1}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="orderbook-spread">
          <div className="spread-info">
            Spread: {items.length > 0 ? items[0].price * 0.02 : 0}
          </div>
        </div>

        <div className="orderbook-side sell-orders">
          <h3>Sell Orders</h3>
          <div className="orderbook-list">
            {items.slice(0, 5).map((item, i) => (
              <div key={i} className="order-entry">
                <span className="order-price">{item.price}</span>
                <span className="order-quantity">{item.quantity || 1}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );

  const HistoryTab = () => (
    <div className="marketplace-tab">
      <div className="transaction-history">
        <h3>Transaction History</h3>
        <div className="transaction-list">
          {transactions.map(tx => (
            <div key={tx.id} className={`transaction-item ${tx.type}`}>
              <div className="tx-info">
                <div className="tx-name">{tx.itemName}</div>
                <div className="tx-time">{new Date(tx.timestamp).toLocaleString()}</div>
              </div>
              <div className="tx-details">
                <span className="tx-type">{tx.type.toUpperCase()}</span>
                <span className="tx-quantity">×{tx.quantity}</span>
                <span className="tx-price">{tx.price}</span>
                <span className={`tx-status ${tx.status}`}>{tx.status}</span>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );

  const WalletTab = () => (
    <div className="marketplace-tab">
      <div className="wallet-panel">
        <h3>Wallet</h3>
        <div className="wallet-balances">
          <div className="balance-card">
            <div className="balance-label">Credits</div>
            <div className="balance-amount">{wallet.credits.toLocaleString()}</div>
            <div className="balance-change">+1,250 today</div>
          </div>
          <div className="balance-card">
            <div className="balance-label">Tokens</div>
            <div className="balance-amount">{wallet.tokens.toLocaleString()}</div>
            <div className="balance-change">+50 today</div>
          </div>
          <div className="balance-card">
            <div className="balance-label">NFTs Owned</div>
            <div className="balance-amount">{wallet.nftBalance}</div>
            <div className="balance-change">+2 this week</div>
          </div>
        </div>

        <div className="wallet-actions">
          <button className="action-btn">Deposit</button>
          <button className="action-btn">Withdraw</button>
          <button className="action-btn">Transfer</button>
        </div>

        <div className="wallet-history">
          <h4>Recent Activity</h4>
          <div className="activity-list">
            {transactions.slice(0, 3).map(tx => (
              <div key={tx.id} className="activity-item">
                <span className="activity-name">{tx.itemName} {tx.type === 'buy' ? 'purchased' : 'sold'}</span>
                <span className="activity-amount">{tx.price}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );

  return (
    <div className="marketplace">
      <div className="marketplace-header">
        <h1>Cosmic Marketplace</h1>
        <p>Trade resources, items, and NFTs across the galaxy</p>
      </div>

      <div className="marketplace-tabs">
        <button
          className={`tab-button ${activeTab === 'trading' ? 'active' : ''}`}
          onClick={() => setActiveTab('trading')}
        >
          Trading
        </button>
        <button
          className={`tab-button ${activeTab === 'nft' ? 'active' : ''}`}
          onClick={() => setActiveTab('nft')}
        >
          NFT Marketplace
        </button>
        <button
          className={`tab-button ${activeTab === 'orderbook' ? 'active' : ''}`}
          onClick={() => setActiveTab('orderbook')}
        >
          Order Book
        </button>
        <button
          className={`tab-button ${activeTab === 'history' ? 'active' : ''}`}
          onClick={() => setActiveTab('history')}
        >
          History
        </button>
        <button
          className={`tab-button ${activeTab === 'wallet' ? 'active' : ''}`}
          onClick={() => setActiveTab('wallet')}
        >
          Wallet
        </button>
      </div>

      <div className="marketplace-content">
        {activeTab === 'trading' && <TradingTab />}
        {activeTab === 'nft' && <NFTTab />}
        {activeTab === 'orderbook' && <OrderBookTab />}
        {activeTab === 'history' && <HistoryTab />}
        {activeTab === 'wallet' && <WalletTab />}
      </div>
    </div>
  );
};

export default Marketplace;

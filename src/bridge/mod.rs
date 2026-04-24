//! Cross-chain bridge module for multi-chain support.
//!
//! Provides lock/mint mechanism, multi-sig validator set, and cross-chain
//! message passing for bridging assets between Stellar and other chains.

pub mod ethereum;
pub mod validator;

pub use ethereum::*;
pub use validator::*;

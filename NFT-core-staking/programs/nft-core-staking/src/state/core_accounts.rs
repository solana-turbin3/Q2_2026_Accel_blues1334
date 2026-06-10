//! Thin Anchor wrappers around Metaplex Core account types.
//!
//! Anchor 1.0.2 cannot use `mpl-core`'s own `anchor` feature (that feature pins
//! `anchor-lang 0.31.1`). Instead we wrap the Core account structs so they can be
//! used as `Account<'info, _>` in instruction contexts, with Anchor performing the
//! owner check + discriminator-style validation for us.
//!
//! Pattern adapted from `bergabman/anchor-1-mplxcore`.

use std::ops::Deref;

use anchor_lang::prelude::*;
use mpl_core::accounts::{BaseAssetV1, BaseCollectionV1};
use mpl_core::types::Key;

/// Lets us write `Program<'info, MplCore>` for the Metaplex Core program.
#[derive(Clone)]
pub struct MplCore;

impl anchor_lang::Id for MplCore {
    fn id() -> Pubkey {
        mpl_core::ID
    }
}

/// Anchor wrapper for a Core `BaseAssetV1` (a single NFT asset account).
#[derive(Clone, Debug)]
pub struct BaseAssetV1Wrap(BaseAssetV1);

impl anchor_lang::AccountDeserialize for BaseAssetV1Wrap {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        let asset = BaseAssetV1::from_bytes(buf)
            .map_err(|_| Error::from(ErrorCode::AccountDidNotDeserialize))?;
        if asset.key != Key::AssetV1 {
            return Err(Error::from(ErrorCode::AccountDiscriminatorMismatch));
        }
        Ok(Self(asset))
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        Self::try_deserialize(buf)
    }
}

// Read-only via CPI: Core mutates the account, so serialization is a no-op.
impl anchor_lang::AccountSerialize for BaseAssetV1Wrap {}

impl anchor_lang::Discriminator for BaseAssetV1Wrap {
    // Core accounts use a 1-byte `Key` enum, not an 8-byte Anchor discriminator.
    const DISCRIMINATOR: &'static [u8] = &[];
}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for BaseAssetV1Wrap {}

impl anchor_lang::Owner for BaseAssetV1Wrap {
    fn owner() -> Pubkey {
        mpl_core::ID
    }
}

impl Deref for BaseAssetV1Wrap {
    type Target = BaseAssetV1;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Anchor wrapper for a Core `BaseCollectionV1` (a collection account).
#[derive(Clone, Debug)]
pub struct BaseCollectionV1Wrap(BaseCollectionV1);

impl anchor_lang::AccountDeserialize for BaseCollectionV1Wrap {
    fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
        let collection = BaseCollectionV1::from_bytes(buf)
            .map_err(|_| Error::from(ErrorCode::AccountDidNotDeserialize))?;
        if collection.key != Key::CollectionV1 {
            return Err(Error::from(ErrorCode::AccountDiscriminatorMismatch));
        }
        Ok(Self(collection))
    }

    fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
        Self::try_deserialize(buf)
    }
}

impl anchor_lang::AccountSerialize for BaseCollectionV1Wrap {}

impl anchor_lang::Discriminator for BaseCollectionV1Wrap {
    const DISCRIMINATOR: &'static [u8] = &[];
}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for BaseCollectionV1Wrap {}

impl anchor_lang::Owner for BaseCollectionV1Wrap {
    fn owner() -> Pubkey {
        mpl_core::ID
    }
}

impl Deref for BaseCollectionV1Wrap {
    type Target = BaseCollectionV1;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

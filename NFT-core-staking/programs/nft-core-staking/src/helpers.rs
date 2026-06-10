//! Shared logic for the staking instructions:
//! - parsing the on-asset stake state stored in the Attributes plugin,
//! - whole-day reward accrual math,
//! - and the Core CPI helpers that write asset / collection attributes.

use anchor_lang::prelude::*;
use mpl_core::accounts::{BaseAssetV1, BaseCollectionV1};
use mpl_core::fetch_plugin;
use mpl_core::instructions::{
    AddCollectionPluginV1CpiBuilder, AddPluginV1CpiBuilder, UpdateCollectionPluginV1CpiBuilder,
    UpdatePluginV1CpiBuilder,
};
use mpl_core::types::{Attribute, Attributes, Plugin, PluginType};

use crate::constants::*;
use crate::error::StakeError;
use crate::state::Config;

/// Decoded staking state held on a Core asset's Attributes plugin.
pub struct AssetStakeState {
    /// Whether the asset already carries an Attributes plugin.
    pub has_attributes_plugin: bool,
    /// `staked == "true"`.
    pub staked: bool,
    /// `staked_at` timestamp (freeze-period clock). 0 if absent.
    pub staked_at: i64,
    /// `last_claim` timestamp (reward clock). 0 if absent.
    pub last_claim: i64,
    /// Every attribute that is NOT one of our managed keys, preserved verbatim.
    pub others: Vec<Attribute>,
}

impl AssetStakeState {
    /// Read and decode the Attributes plugin from a Core asset account.
    pub fn read(asset_ai: &AccountInfo) -> Result<Self> {
        let fetched: Option<Attributes> =
            fetch_plugin::<BaseAssetV1, Attributes>(asset_ai, PluginType::Attributes)
                .ok()
                .map(|(_, attrs, _)| attrs);

        let mut state = AssetStakeState {
            has_attributes_plugin: fetched.is_some(),
            staked: false,
            staked_at: 0,
            last_claim: 0,
            others: Vec::new(),
        };

        if let Some(attributes) = fetched {
            for attribute in attributes.attribute_list {
                match attribute.key.as_str() {
                    ATTR_STAKED => state.staked = attribute.value == "true",
                    ATTR_STAKED_AT => {
                        state.staked_at = attribute
                            .value
                            .parse::<i64>()
                            .map_err(|_| StakeError::InvalidTimestamp)?
                    }
                    ATTR_LAST_CLAIM => {
                        state.last_claim = attribute
                            .value
                            .parse::<i64>()
                            .map_err(|_| StakeError::InvalidTimestamp)?
                    }
                    _ => state.others.push(attribute),
                }
            }
        }

        Ok(state)
    }

    /// Rebuild the full attribute list: preserved `others` plus our managed keys.
    pub fn to_attribute_list(
        &self,
        staked: bool,
        staked_at: i64,
        last_claim: i64,
    ) -> Vec<Attribute> {
        let mut list = self.others.clone();
        list.push(Attribute {
            key: ATTR_STAKED.to_string(),
            value: if staked { "true" } else { "false" }.to_string(),
        });
        list.push(Attribute {
            key: ATTR_STAKED_AT.to_string(),
            value: staked_at.to_string(),
        });
        list.push(Attribute {
            key: ATTR_LAST_CLAIM.to_string(),
            value: last_claim.to_string(),
        });
        list
    }
}

/// Compute the reward owed for the period `[since, now]`, counting only whole
/// staked days. Returns `(amount_in_base_units, new_checkpoint)` where the new
/// checkpoint advances by exactly the consumed whole days so the leftover
/// seconds keep accruing toward the next claim.
///
/// `reward tokens per day = rewards_bps / 10_000` (scaled by `REWARDS_DECIMALS`).
pub fn accrued_rewards(now: i64, since: i64, config: &Config) -> Result<(u64, i64)> {
    let elapsed = now.checked_sub(since).ok_or(StakeError::Overflow)?;
    if elapsed <= 0 {
        return Ok((0, since));
    }

    let days = elapsed / SECONDS_PER_DAY; // floor
    if days == 0 {
        return Ok((0, since));
    }

    let new_checkpoint = since
        .checked_add(days.checked_mul(SECONDS_PER_DAY).ok_or(StakeError::Overflow)?)
        .ok_or(StakeError::Overflow)?;

    let amount = (days as u64)
        .checked_mul(config.rewards_bps as u64)
        .ok_or(StakeError::Overflow)?
        .checked_mul(10u64.pow(REWARDS_DECIMALS as u32))
        .ok_or(StakeError::Overflow)?
        .checked_div(10_000)
        .ok_or(StakeError::Overflow)?;

    Ok((amount, new_checkpoint))
}

/// Write the asset's Attributes plugin, adding it if absent or updating it if
/// already present. Signed by the `update_authority` PDA (the authority-managed
/// Attributes plugin owner).
#[allow(clippy::too_many_arguments)]
pub fn write_asset_attributes<'info>(
    attributes: Vec<Attribute>,
    has_plugin: bool,
    mpl_core: &AccountInfo<'info>,
    asset: &AccountInfo<'info>,
    collection: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    update_authority: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    ua_signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let plugin = Plugin::Attributes(Attributes {
        attribute_list: attributes,
    });

    if has_plugin {
        UpdatePluginV1CpiBuilder::new(mpl_core)
            .asset(asset)
            .collection(Some(collection))
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .invoke_signed(ua_signer_seeds)?;
    } else {
        AddPluginV1CpiBuilder::new(mpl_core)
            .asset(asset)
            .collection(Some(collection))
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .init_authority(mpl_core::types::PluginAuthority::UpdateAuthority)
            .invoke_signed(ua_signer_seeds)?;
    }

    Ok(())
}

/// Adjust the collection-level `total_staked` counter by `delta` (+1 on stake,
/// -1 on unstake), creating the collection Attributes plugin if it is missing.
/// Signed by the `update_authority` PDA.
#[allow(clippy::too_many_arguments)]
pub fn bump_collection_total_staked<'info>(
    delta: i64,
    mpl_core: &AccountInfo<'info>,
    collection: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    update_authority: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    ua_signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let fetched: Option<Attributes> =
        fetch_plugin::<BaseCollectionV1, Attributes>(collection, PluginType::Attributes)
            .ok()
            .map(|(_, attrs, _)| attrs);

    let has_plugin = fetched.is_some();

    let mut others: Vec<Attribute> = Vec::new();
    let mut current: i64 = 0;
    if let Some(attributes) = fetched {
        for attribute in attributes.attribute_list {
            if attribute.key == ATTR_TOTAL_STAKED {
                current = attribute
                    .value
                    .parse::<i64>()
                    .map_err(|_| StakeError::InvalidCollectionStats)?;
            } else {
                others.push(attribute);
            }
        }
    }

    let updated = current.checked_add(delta).ok_or(StakeError::Overflow)?;
    let updated = updated.max(0); // never report a negative count

    others.push(Attribute {
        key: ATTR_TOTAL_STAKED.to_string(),
        value: updated.to_string(),
    });

    let plugin = Plugin::Attributes(Attributes {
        attribute_list: others,
    });

    if has_plugin {
        UpdateCollectionPluginV1CpiBuilder::new(mpl_core)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .invoke_signed(ua_signer_seeds)?;
    } else {
        AddCollectionPluginV1CpiBuilder::new(mpl_core)
            .collection(collection)
            .payer(payer)
            .authority(Some(update_authority))
            .system_program(system_program)
            .plugin(plugin)
            .init_authority(mpl_core::types::PluginAuthority::UpdateAuthority)
            .invoke_signed(ua_signer_seeds)?;
    }

    Ok(())
}

//! On-chain account state.
//!
//! Unlike Anchor, Pinocchio does not impose an 8-byte discriminator. These
//! accounts are PDAs whose address already encodes their identity (via the
//! seeds), so we drop the discriminator entirely. This saves 8 bytes of rent
//! per account *and* the compute spent (de)serialising it.
//!
//! The structs are plain `#[repr(C)]` byte layouts with every field aligned to
//! 1 byte (all integers are stored as little-endian byte arrays). That makes
//! casting the raw account data to a reference sound regardless of the data
//! pointer alignment, and lets us read/write fields in place — true zero-copy.

use pinocchio::{
    account::{AccountView, Ref, RefMut},
    error::ProgramError,
};

/// State of a single fundraising campaign.
///
/// Layout (122 bytes, no padding — every field is byte aligned):
/// ```text
///    0..32  maker            Pubkey
///   32..64  mint_to_raise    Pubkey
///   64..96  vault            Pubkey  (pinned token account address)
///   96..104 amount_to_raise  u64  (LE)
///  104..112 current_amount   u64  (LE)
///  112..120 time_started     i64  (LE)
///  120..121 duration         u8   (days)
///  121..122 bump             u8
/// ```
#[repr(C)]
pub struct Fundraiser {
    pub maker: [u8; 32],
    pub mint_to_raise: [u8; 32],
    pub vault: [u8; 32],
    amount_to_raise: [u8; 8],
    current_amount: [u8; 8],
    time_started: [u8; 8],
    pub duration: u8,
    pub bump: u8,
}

impl Fundraiser {
    pub const LEN: usize = 122;

    #[inline(always)]
    pub fn load(account: &AccountView) -> Result<Ref<'_, Self>, ProgramError> {
        let data = account.try_borrow()?;
        if data.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: length checked above; `Self` is a 1-byte-aligned POD layout.
        Ok(Ref::map(data, |d: &[u8]| unsafe {
            &*(d.as_ptr() as *const Self)
        }))
    }

    #[inline(always)]
    pub fn load_mut(account: &mut AccountView) -> Result<RefMut<'_, Self>, ProgramError> {
        let data = account.try_borrow_mut()?;
        if data.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: length checked above; `Self` is a 1-byte-aligned POD layout.
        Ok(RefMut::map(data, |d: &mut [u8]| unsafe {
            &mut *(d.as_mut_ptr() as *mut Self)
        }))
    }

    // Accessors for the byte-array integer fields.
    #[inline(always)]
    pub fn amount_to_raise(&self) -> u64 {
        u64::from_le_bytes(self.amount_to_raise)
    }
    #[inline(always)]
    pub fn current_amount(&self) -> u64 {
        u64::from_le_bytes(self.current_amount)
    }
    #[inline(always)]
    pub fn time_started(&self) -> i64 {
        i64::from_le_bytes(self.time_started)
    }
    #[inline(always)]
    pub fn set_amount_to_raise(&mut self, v: u64) {
        self.amount_to_raise = v.to_le_bytes();
    }
    #[inline(always)]
    pub fn set_current_amount(&mut self, v: u64) {
        self.current_amount = v.to_le_bytes();
    }
    #[inline(always)]
    pub fn set_time_started(&mut self, v: i64) {
        self.time_started = v.to_le_bytes();
    }
}

/// Per-contributor running total.
///
/// Layout (8 bytes): `0..8 amount u64 (LE)`.
#[repr(C)]
pub struct Contributor {
    amount: [u8; 8],
}

impl Contributor {
    pub const LEN: usize = 8;

    #[inline(always)]
    pub fn load_mut(account: &mut AccountView) -> Result<RefMut<'_, Self>, ProgramError> {
        let data = account.try_borrow_mut()?;
        if data.len() != Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        // SAFETY: length checked above; `Self` is a 1-byte-aligned POD layout.
        Ok(RefMut::map(data, |d: &mut [u8]| unsafe {
            &mut *(d.as_mut_ptr() as *mut Self)
        }))
    }

    #[inline(always)]
    pub fn amount(&self) -> u64 {
        u64::from_le_bytes(self.amount)
    }
    #[inline(always)]
    pub fn set_amount(&mut self, v: u64) {
        self.amount = v.to_le_bytes();
    }
}

// Compile-time guarantees that the structs have the exact byte size we rely on
// when casting raw account data (no padding from `#[repr(C)]`).
const _: () = assert!(core::mem::size_of::<Fundraiser>() == Fundraiser::LEN);
const _: () = assert!(core::mem::size_of::<Contributor>() == Contributor::LEN);
const _: () = assert!(core::mem::align_of::<Fundraiser>() == 1);
const _: () = assert!(core::mem::align_of::<Contributor>() == 1);

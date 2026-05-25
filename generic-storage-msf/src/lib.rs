//! Generic storage with multiple serialization formats.
//!
//! Provides a [`Serializer`] trait with three implementations — [`Borsh`],
//! [`Wincode`], and [`Json`] — and a generic [`Storage<T, S>`] container
//! that stores bytes internally and uses `PhantomData<T>` to track the
//! logical type without holding a value.

use std::marker::PhantomData;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("borsh error: {0}")]
    Borsh(#[from] std::io::Error),
    #[error("wincode write error: {0}")]
    WincodeWrite(#[from] wincode::error::WriteError),
    #[error("wincode read error: {0}")]
    WincodeRead(#[from] wincode::error::ReadError),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no data stored")]
    Empty,
}

/// Convert values of type `T` to and from bytes.
///
/// The trait is generic over `T` so each implementation can require its own
/// trait bounds: Borsh needs `BorshSerialize + BorshDeserialize`, Wincode
/// needs `wincode::Serialize<Src = T> + wincode::DeserializeOwned<Dst = T>`,
/// and JSON needs `serde::Serialize + serde::de::DeserializeOwned`.
pub trait Serializer<T> {
    fn to_bytes(&self, value: &T) -> Result<Vec<u8>, StorageError>;
    fn from_bytes(&self, bytes: &[u8]) -> Result<T, StorageError>;
}

pub struct Borsh;
pub struct Wincode;
pub struct Json;

impl<T> Serializer<T> for Borsh
where
    T: borsh::BorshSerialize + borsh::BorshDeserialize,
{
    fn to_bytes(&self, value: &T) -> Result<Vec<u8>, StorageError> {
        borsh::to_vec(value).map_err(StorageError::from)
    }

    fn from_bytes(&self, bytes: &[u8]) -> Result<T, StorageError> {
        borsh::from_slice(bytes).map_err(StorageError::from)
    }
}

impl<T> Serializer<T> for Wincode
where
    T: wincode::Serialize<Src = T> + wincode::DeserializeOwned<Dst = T>,
{
    fn to_bytes(&self, value: &T) -> Result<Vec<u8>, StorageError> {
        wincode::serialize(value).map_err(StorageError::from)
    }

    fn from_bytes(&self, bytes: &[u8]) -> Result<T, StorageError> {
        wincode::deserialize(bytes).map_err(StorageError::from)
    }
}

impl<T> Serializer<T> for Json
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    fn to_bytes(&self, value: &T) -> Result<Vec<u8>, StorageError> {
        serde_json::to_vec(value).map_err(StorageError::from)
    }

    fn from_bytes(&self, bytes: &[u8]) -> Result<T, StorageError> {
        serde_json::from_slice(bytes).map_err(StorageError::from)
    }
}

pub struct Storage<T, S: Serializer<T>> {
    serializer: S,
    data: Option<Vec<u8>>,
    _marker: PhantomData<T>,
}

impl<T, S: Serializer<T>> Storage<T, S> {
    pub fn new(serializer: S) -> Self {
        Self {
            serializer,
            data: None,
            _marker: PhantomData,
        }
    }

    pub fn save(&mut self, value: &T) -> Result<(), StorageError> {
        self.data = Some(self.serializer.to_bytes(value)?);
        Ok(())
    }

    pub fn load(&self) -> Result<T, StorageError> {
        let bytes = self.data.as_ref().ok_or(StorageError::Empty)?;
        self.serializer.from_bytes(bytes)
    }

    pub fn has_data(&self) -> bool {
        self.data.is_some()
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    /// Bonus: migrate stored data to a different serializer.
    pub fn convert<S2: Serializer<T>>(
        &self,
        new_serializer: S2,
    ) -> Result<Storage<T, S2>, StorageError> {
        let mut next = Storage::new(new_serializer);
        if let Some(bytes) = &self.data {
            let value: T = self.serializer.from_bytes(bytes)?;
            next.save(&value)?;
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::{BorshDeserialize, BorshSerialize};
    use serde::{Deserialize, Serialize};
    use wincode::{SchemaRead, SchemaWrite};

    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        BorshSerialize,
        BorshDeserialize,
        Serialize,
        Deserialize,
        SchemaWrite,
        SchemaRead,
    )]
    struct Person {
        name: String,
        age: u32,
    }

    fn sample() -> Person {
        Person {
            name: "Marcelo".to_string(),
            age: 40,
        }
    }

    fn roundtrip<T, S>(serializer: S, value: &T)
    where
        T: std::fmt::Debug + PartialEq,
        S: Serializer<T>,
    {
        let mut storage: Storage<T, S> = Storage::new(serializer);
        storage.save(value).unwrap();
        assert_eq!(&storage.load().unwrap(), value);
    }

    #[test]
    fn borsh_roundtrip() {
        let mut storage: Storage<Person, _> = Storage::new(Borsh);
        assert!(!storage.has_data());
        storage.save(&sample()).unwrap();
        assert!(storage.has_data());
        assert_eq!(storage.load().unwrap(), sample());
    }

    #[test]
    fn wincode_roundtrip() {
        let mut storage: Storage<Person, _> = Storage::new(Wincode);
        storage.save(&sample()).unwrap();
        assert_eq!(storage.load().unwrap(), sample());
    }

    #[test]
    fn json_roundtrip() {
        let mut storage: Storage<Person, _> = Storage::new(Json);
        storage.save(&sample()).unwrap();
        assert_eq!(storage.load().unwrap(), sample());

        let raw = std::str::from_utf8(storage.as_bytes().unwrap()).unwrap();
        assert!(raw.contains("Marcelo"));
        assert!(raw.contains("40"));
    }

    #[test]
    fn empty_storage_errors_on_load() {
        let storage: Storage<Person, _> = Storage::new(Borsh);
        assert!(matches!(storage.load(), Err(StorageError::Empty)));
    }

    #[test]
    fn convert_between_serializers() {
        let mut borsh_storage: Storage<Person, _> = Storage::new(Borsh);
        borsh_storage.save(&sample()).unwrap();

        let json_storage = borsh_storage.convert(Json).unwrap();
        assert_eq!(json_storage.load().unwrap(), sample());

        let wincode_storage = json_storage.convert(Wincode).unwrap();
        assert_eq!(wincode_storage.load().unwrap(), sample());
    }

    #[test]
    fn formats_produce_different_bytes() {
        let value = sample();
        let borsh_bytes = Borsh.to_bytes(&value).unwrap();
        let wincode_bytes = Wincode.to_bytes(&value).unwrap();
        let json_bytes = Json.to_bytes(&value).unwrap();

        assert_ne!(borsh_bytes, wincode_bytes);
        assert_ne!(borsh_bytes, json_bytes);
        assert_ne!(wincode_bytes, json_bytes);
    }

    // ---------- edge cases ----------

    #[test]
    fn save_overwrites_previous_value() {
        let mut storage: Storage<Person, _> = Storage::new(Borsh);
        storage
            .save(&Person {
                name: "first".into(),
                age: 1,
            })
            .unwrap();
        storage
            .save(&Person {
                name: "second".into(),
                age: 99,
            })
            .unwrap();

        let loaded = storage.load().unwrap();
        assert_eq!(loaded.name, "second");
        assert_eq!(loaded.age, 99);
    }

    #[test]
    fn borsh_rejects_garbage_bytes() {
        // Borsh reads a length-prefixed string; a single 0xff byte means it tries
        // to read a 4 GB-sized String, which fails immediately.
        let result: Result<Person, _> = Borsh.from_bytes(&[0xff, 0xff, 0xff, 0xff]);
        assert!(
            matches!(result, Err(StorageError::Borsh(_))),
            "expected Borsh error, got {result:?}"
        );
    }

    #[test]
    fn wincode_rejects_garbage_bytes() {
        let result: Result<Person, _> = Wincode.from_bytes(&[0xff, 0xff, 0xff, 0xff]);
        assert!(
            matches!(result, Err(StorageError::WincodeRead(_))),
            "expected WincodeRead error, got {result:?}"
        );
    }

    #[test]
    fn json_rejects_invalid_bytes() {
        let result: Result<Person, _> = Json.from_bytes(b"not valid json at all");
        assert!(
            matches!(result, Err(StorageError::Json(_))),
            "expected Json error, got {result:?}"
        );
    }

    #[test]
    fn json_rejects_truncated_bytes() {
        // missing closing brace
        let result: Result<Person, _> = Json.from_bytes(br#"{"name":"x","age":1"#);
        assert!(matches!(result, Err(StorageError::Json(_))));
    }

    #[test]
    fn json_rejects_borsh_bytes() {
        // Cross-format: Borsh bytes are binary, JSON expects text. Must fail.
        let borsh_bytes = Borsh.to_bytes(&sample()).unwrap();
        let result: Result<Person, _> = Json.from_bytes(&borsh_bytes);
        assert!(
            matches!(result, Err(StorageError::Json(_))),
            "expected Json error decoding borsh bytes, got {result:?}"
        );
    }

    #[test]
    fn borsh_rejects_json_bytes() {
        let json_bytes = Json.to_bytes(&sample()).unwrap();
        let result: Result<Person, _> = Borsh.from_bytes(&json_bytes);
        assert!(
            matches!(result, Err(StorageError::Borsh(_))),
            "expected Borsh error decoding json bytes, got {result:?}"
        );
    }

    #[test]
    fn boundary_values_roundtrip() {
        let cases = [
            Person {
                name: String::new(),
                age: 0,
            },
            Person {
                name: "x".into(),
                age: u32::MAX,
            },
            Person {
                name: "a".repeat(10_000),
                age: 12_345,
            },
        ];
        for case in &cases {
            roundtrip(Borsh, case);
            roundtrip(Wincode, case);
            roundtrip(Json, case);
        }
    }

    #[test]
    fn unicode_and_escapes_roundtrip() {
        let value = Person {
            name: "🦀 André López 日本語 \"quoted\" \\backslash \n newline".to_string(),
            age: 42,
        };
        roundtrip(Borsh, &value);
        roundtrip(Wincode, &value);
        roundtrip(Json, &value);
    }

    #[test]
    fn convert_from_empty_storage_stays_empty() {
        let storage: Storage<Person, _> = Storage::new(Borsh);
        assert!(!storage.has_data());

        let migrated = storage.convert(Json).unwrap();
        assert!(!migrated.has_data());
        assert!(matches!(migrated.load(), Err(StorageError::Empty)));
    }

    #[test]
    fn convert_chain_preserves_value_across_all_formats() {
        let mut start: Storage<Person, _> = Storage::new(Borsh);
        start.save(&sample()).unwrap();
        let via_json = start.convert(Json).unwrap();
        let via_wincode = via_json.convert(Wincode).unwrap();
        let back_to_borsh = via_wincode.convert(Borsh).unwrap();
        assert_eq!(back_to_borsh.load().unwrap(), sample());
    }

    // Second test type to prove the generic bounds aren't accidentally
    // tailored to `Person`.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        BorshSerialize,
        BorshDeserialize,
        Serialize,
        Deserialize,
        SchemaWrite,
        SchemaRead,
    )]
    struct Account {
        id: u64,
        balance: i64,
        tags: Vec<String>,
        active: bool,
    }

    #[test]
    fn complex_struct_roundtrips_across_formats() {
        let account = Account {
            id: 12_345,
            balance: -500,
            tags: vec!["solana".into(), "anchor".into(), String::new()],
            active: true,
        };
        roundtrip(Borsh, &account);
        roundtrip(Wincode, &account);
        roundtrip(Json, &account);
    }

    #[test]
    fn empty_bytes_fail_to_decode() {
        // No format can decode an empty buffer into a Person (which has
        // required fields).
        assert!(Borsh.from_bytes(&[] as &[u8]).map(|_: Person| ()).is_err());
        assert!(Wincode.from_bytes(&[] as &[u8]).map(|_: Person| ()).is_err());
        assert!(Json.from_bytes(&[] as &[u8]).map(|_: Person| ()).is_err());
    }
}

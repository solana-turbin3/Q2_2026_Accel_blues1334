use borsh::{BorshDeserialize, BorshSerialize};
use generic_storage_msf::{Borsh, Json, Storage, Wincode};
use serde::{Deserialize, Serialize};
use wincode::{SchemaRead, SchemaWrite};

#[derive(
    Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize, SchemaWrite, SchemaRead,
)]
struct Person {
    name: String,
    age: u32,
}

fn main() {
    let person = Person {
        name: "Marcelo".to_string(),
        age: 40,
    };

    let mut borsh_storage: Storage<Person, _> = Storage::new(Borsh);
    borsh_storage.save(&person).unwrap();
    println!(
        "Borsh   ({} bytes): {:?}",
        borsh_storage.as_bytes().unwrap().len(),
        borsh_storage.load().unwrap()
    );

    let mut wincode_storage: Storage<Person, _> = Storage::new(Wincode);
    wincode_storage.save(&person).unwrap();
    println!(
        "Wincode ({} bytes): {:?}",
        wincode_storage.as_bytes().unwrap().len(),
        wincode_storage.load().unwrap()
    );

    let mut json_storage: Storage<Person, _> = Storage::new(Json);
    json_storage.save(&person).unwrap();
    println!(
        "Json    ({} bytes): {}",
        json_storage.as_bytes().unwrap().len(),
        std::str::from_utf8(json_storage.as_bytes().unwrap()).unwrap()
    );

    let migrated = borsh_storage.convert(Json).unwrap();
    println!(
        "Migrated borsh -> json: {}",
        std::str::from_utf8(migrated.as_bytes().unwrap()).unwrap()
    );
}

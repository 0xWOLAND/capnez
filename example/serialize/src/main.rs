use capnez_macros::capnp;
use capnez_codegen::capnp_include;
use serde::{Serialize, Deserialize};
use std::{fs, io::Cursor};

capnp_include!();

// Define a simple struct that we want to serialize
#[capnp]
#[derive(Serialize, Deserialize, Debug)]
struct Person {
    name: String,
    age: u32,
    email: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an instance of our struct
    let person = Person {
        name: "John Doe".to_string(),
        age: 30,
        email: "john@example.com".to_string(),
    };

    // Serialize the struct to bytes using capnp
    let mut message = capnp::message::Builder::new_default();
    let mut builder = message.init_root::<schema_capnp::person::Builder>();
    builder.set_name(&person.name);
    builder.set_age(person.age);
    builder.set_email(&person.email);
    
    let mut bytes = Vec::new();
    capnp::serialize::write_message(&mut bytes, &message)?;
    
    println!("Serialized {} bytes", bytes.len());
    
    // Deserialize the bytes back into a Person struct
    let reader = capnp::serialize::read_message(&mut Cursor::new(&bytes), Default::default())?;
    let person_reader = reader.get_root::<schema_capnp::person::Reader>()?;
    
    let deserialized_person = Person {
        name: person_reader.get_name()?.to_string()?,
        age: person_reader.get_age(),
        email: person_reader.get_email()?.to_string()?,
    };
    
    // Print the deserialized struct
    println!("Deserialized person: {:#?}", deserialized_person);

    Ok(())
}
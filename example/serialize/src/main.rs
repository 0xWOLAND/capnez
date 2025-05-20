use capnez_macros::capnp;
use capnez_codegen::capnp_include;
use serde::{Serialize, Deserialize};

capnp_include!();

// Define a simple struct that we want to serialize
#[capnp]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
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
    
    // Save to file in OUT_DIR
    let path = format!("{}/target/person.bin", env!("OUT_DIR"));
    std::fs::create_dir_all(format!("{}/target", env!("OUT_DIR")))?;
    let mut file = std::fs::File::create(&path)?;
    capnp::serialize::write_message(&mut file, &message)?;
    println!("Serialized to {}", path);
    
    // Read from file
    let mut file = std::fs::File::open(&path)?;
    let reader = capnp::serialize::read_message(&mut file, Default::default())?;
    let person_reader = reader.get_root::<schema_capnp::person::Reader>()?;
    
    let deserialized_person = Person {
        name: person_reader.get_name()?.to_string()?,
        age: person_reader.get_age(),
        email: person_reader.get_email()?.to_string()?,
    };
    
    assert_eq!(person, deserialized_person);
    
    println!("All assertions passed!");

    Ok(())
}
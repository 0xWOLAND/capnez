use capnp_export::CapnpExport;


#[derive(CapnpExport)]
struct Home {
    owner: Person,
    address: String,
}


#[derive(CapnpExport)]
struct Person {
    name: String,
    age: u32,
}
fn main() {
    println!("Capnp schema generated!");
}

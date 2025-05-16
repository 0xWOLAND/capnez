use capnp_derive::{CapnpDerive, capnp_interface};

#[derive(CapnpDerive)]
pub struct HelloRequest {
    pub name: String,
}

#[derive(CapnpDerive)]
pub struct HelloReply {
    pub message: String,
}

#[capnp_interface]
pub trait HelloWorld {
    fn say_hello(&self, request: HelloRequest) -> HelloReply;
}

fn main() {
    println!("Capnp schema generated!");
}

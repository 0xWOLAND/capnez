use capnez_macros::capnp;
use serde::{Serialize, Deserialize};
use capnez_codegen::capnp_include;

capnp_include!();

#[derive(Serialize, Deserialize, Debug)]
struct Information {
    major: String,
    age: u32,
}

#[capnp]
#[derive(Serialize, Deserialize)]
pub struct HelloRequest {
    name: String,
    information: Information,
}

#[capnp]
#[derive(Serialize, Deserialize)]
pub struct HelloReply {
    message: String,
}

#[capnp]
pub trait HelloWorld {
    fn say_hello(request: HelloRequest) -> HelloReply;
}

pub mod client;
pub mod server;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = ::std::env::args().collect();
    if args.len() >= 2 {
        match &args[1][..] {
            "client" => return client::main().await,
            "server" => return server::main().await,
            _ => (),
        }
    }

    println!("usage: {} [client | server] ADDRESS", args[0]);
    Ok(())
}
use capnez_macros::capnp;
#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};
use capnez_codegen::capnp_include;

capnp_include!();

#[capnp]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct HelloRequest {
    name: String,
    age: u32,
}

#[capnp]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct HelloReply {
    message: String,
}

#[capnp]
trait HelloWorld {
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
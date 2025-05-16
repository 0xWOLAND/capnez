use capnez_macros::capnp;

#[capnp]
struct HelloRequest {
    name: String,
}

#[capnp]
struct HelloReply {
    message: String,
}

#[capnp]
trait HelloWorld {
    fn sayHello(request: HelloRequest) -> HelloReply;
}

pub mod schema_capnp {
    include!(concat!(env!("OUT_DIR"), "/generated/schema_capnp.rs"));
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
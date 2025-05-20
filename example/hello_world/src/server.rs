use capnp::capability::Promise;
use capnp_rpc::{pry, rpc_twoparty_capnp, twoparty, RpcSystem};
use serde_json;
use crate::{schema_capnp::hello_world, Information};
use futures::AsyncReadExt;
use std::net::ToSocketAddrs;

struct HelloWorldImpl;

impl hello_world::Server for HelloWorldImpl {
    fn say_hello(
        &mut self,
        params: hello_world::SayHelloParams,
        mut results: hello_world::SayHelloResults,
    ) -> Promise<(), ::capnp::Error> {
        let request = pry!(pry!(params.get()).get_request());
        let name = pry!(pry!(request.get_name()).to_str());
        let info_reader = pry!(request.get_information());
        
        let info_bytes: Vec<u8> = (0..info_reader.len()).map(|i| info_reader.get(i)).collect();
        
        match serde_json::from_slice::<Information>(&info_bytes) {
            Ok(info) => {
                println!("name: {}, information: {:?}", name, info);
                let message = format!("Hello, {}! Your major is {} and you are {} years old.", name, info.major, info.age);
                results.get().set_message(message);
                Promise::ok(())
            }
            Err(e) => Promise::err(capnp::Error::failed(format!("Failed to deserialize Information: {}", e)))
        }
    }
}

pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = ::std::env::args().collect();
    if args.len() != 3 {
        println!("usage: {} server ADDRESS[:PORT]", args[0]);
        return Ok(());
    }

    let addr = args[2].to_socket_addrs()?.next().expect("could not parse address");

    tokio::task::LocalSet::new().run_until(async move {
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        let hello_world_client: hello_world::Client = capnp_rpc::new_client(HelloWorldImpl);

        loop {
            let (stream, _) = listener.accept().await?;
            stream.set_nodelay(true)?;
            let (reader, writer) = tokio_util::compat::TokioAsyncReadCompatExt::compat(stream).split();
            let network = twoparty::VatNetwork::new(
                futures::io::BufReader::new(reader),
                futures::io::BufWriter::new(writer),
                rpc_twoparty_capnp::Side::Server,
                Default::default(),
            );

            tokio::task::spawn_local(RpcSystem::new(Box::new(network), Some(hello_world_client.clone().client)));
        }
    }).await
}
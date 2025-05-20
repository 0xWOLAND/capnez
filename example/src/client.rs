use crate::{schema_capnp::hello_world, Information};
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use std::net::ToSocketAddrs;
use serde_json;
use futures::AsyncReadExt;

pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = ::std::env::args().collect();
    if args.len() != 4 {
        println!("usage: {} client HOST:PORT MESSAGE", args[0]);
        return Ok(());
    }

    let addr = args[2]
        .to_socket_addrs()?
        .next()
        .expect("could not parse address");

    let msg = args[3].to_string();

    tokio::task::LocalSet::new()
        .run_until(async move {
            let stream = tokio::net::TcpStream::connect(&addr).await?;
            stream.set_nodelay(true)?;
            let (reader, writer) =
                tokio_util::compat::TokioAsyncReadCompatExt::compat(stream).split();
            let rpc_network = Box::new(twoparty::VatNetwork::new(
                futures::io::BufReader::new(reader),
                futures::io::BufWriter::new(writer),
                rpc_twoparty_capnp::Side::Client,
                Default::default(),
            ));
            let mut rpc_system = RpcSystem::new(rpc_network, None);
            let hello_world: hello_world::Client =
                rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);

            tokio::task::spawn_local(rpc_system);

            let mut request = hello_world.say_hello_request();
            let mut request_builder = request.get().init_request();
            request_builder.set_name(&msg);
            
            // Create Information struct and serialize it to bytes
            let info = Information {
                major: "Computer Science".to_string(),
                age: 25,
            };
            let info_bytes = serde_json::to_vec(&info)?;
            let mut info_list = request_builder.init_information(info_bytes.len() as u32);
            for (i, &byte) in info_bytes.iter().enumerate() {
                info_list.set(i as u32, byte);
            }

            let reply = request.send().promise.await?;
            
            println!(
                "received: {}",
                reply.get()?.get_message()?.to_str()?
            );
            Ok(())
        })
        .await
}
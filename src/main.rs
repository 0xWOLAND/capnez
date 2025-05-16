use capnp_derive::capnp;
use capnp_macros::maybe_include_capnp;

maybe_include_capnp!();

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

fn main() {
    println!("Capnp schema generated!");
}

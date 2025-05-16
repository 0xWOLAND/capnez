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

fn main() {

}
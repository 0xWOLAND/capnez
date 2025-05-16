@0xabcdefabcdefabcdef;

struct HelloRequest {
  name @0 :Text;
}

struct HelloReply {
  message @0 :Text;
}

interface HelloWorld {
  sayHello @0 (request :HelloRequest) -> HelloReply;
}


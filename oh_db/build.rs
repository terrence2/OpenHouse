extern crate capnpc;

fn main() {
    ::capnpc::compile("schema", &["oh_shared/db/messages.capnp"]).unwrap();
}

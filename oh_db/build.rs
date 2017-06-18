extern crate capnpc;

fn main() {
    capnpc::CompilerCommand::new()
        .src_prefix("oh_shared/db/")
        .file("oh_shared/db/messages.capnp")
        .run().expect("schema compiler command");
}

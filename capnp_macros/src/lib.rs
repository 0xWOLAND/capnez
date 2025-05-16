#[macro_export]
macro_rules! maybe_include_capnp {
    () => {
        #[cfg(include_capnp)]
        pub mod capnp {
            include!(concat!(env!("OUT_DIR"), "/target/capnp/generated_capnp.rs"));
        }
    };
}

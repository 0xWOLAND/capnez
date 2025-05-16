#[macro_export]
macro_rules! maybe_include_capnp {
    () => {
        #[cfg(include_capnp)]
        pub mod capnez {
            include!(concat!(env!("OUT_DIR"), "/capnp/generated_capnp.rs"));
        }
    };
}

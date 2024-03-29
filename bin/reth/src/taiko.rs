#![allow(missing_docs, rustdoc::missing_crate_level_docs)]

use reth::cli::Cli;

// We use jemalloc for performance reasons
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[cfg(not(feature = "taiko"))]
compile_error!("Cannot build the `taiko-reth` binary with the `taiko` feature flag disabled. Did you mean to build `reth`?");

#[cfg(feature = "taiko")]
fn main() {
    use reth_node_taiko::TaikoNode;

    reth::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = Cli::parse_args().run(|builder, _| async move {
        let handle = builder.launch_node(TaikoNode::new()).await?;

        handle.node_exit_future.await
    }) {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}

//! A standalone **XSLT module server**: bind `ikigai_xslt::space()` and serve it over a
//! Unix-domain socket, so a host in another process can resolve `urn:xslt:transform`
//! against it (the module pulls its `src`/`stylesheet` back over the same socket — the
//! by-reference session). This is the "thin `main()` that binds one `space()` and calls
//! `serve()`" from the module-format design.
//!
//! ```text
//! # terminal 1 — the module:
//! cargo run --example uds-server -- /tmp/ikigai-xslt.sock
//! # terminal 2 — a host that transforms through it:
//! cargo run --example uds-client -- /tmp/ikigai-xslt.sock
//! ```
//!
//! The socket is created `0600`; place it in a per-user `0700` directory for a real
//! deployment (see `ikigai-ipc::default_socket_path` for the convention).

use std::path::PathBuf;
use std::sync::Arc;

use ikigai_core::Space;

fn socket_path() -> PathBuf {
    std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ikigai-xslt.sock"))
}

fn main() -> std::io::Result<()> {
    let path = socket_path();
    eprintln!("ikigai-xslt module server listening on {}", path.display());
    eprintln!("(resolve urn:xslt:transform against it from another process; Ctrl-C to stop)");
    // `serve` blocks, handling each connection on its own thread, until an accept error.
    ikigai_module::serve(Arc::new(ikigai_xslt::space()) as Arc<dyn Space>, &path)
}

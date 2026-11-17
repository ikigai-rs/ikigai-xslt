//! How much stack does one transform need? Ledger #3702: on wasm a transform runs inline on
//! the module's stack (1 MiB by default), where the native build gives it a sized thread.
//!
//!     cargo run --release --example stack-need -- <stylesheet.xsl> [source.xml]
//!
//! Compiles the stylesheet and runs it once over the source (`<doc/>` without one) on a thread
//! of a given stack, in a CHILD process, because an overflow aborts the process; the parent
//! halves the interval until it knows the smallest stack, to 4 KiB, that runs it.
//!
//! To measure xrust's XPath parser alone, name the expression instead:
//!
//!     IKIGAI_XSLT_STACK_NEED_XPATH='((1))' cargo run --release --example stack-need

use std::process::Command;

const CHILD: &str = "IKIGAI_XSLT_STACK_NEED_KIB";

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        args.push(String::new())
    }
    let xpath = std::env::var("IKIGAI_XSLT_STACK_NEED_XPATH").ok();
    let style = if xpath.is_some() {
        String::new()
    } else {
        std::fs::read_to_string(args.first().expect("usage: <stylesheet.xsl> [source.xml]"))
            .expect("read the stylesheet")
    };
    let src = match args.get(1) {
        Some(path) => std::fs::read_to_string(path).expect("read the source"),
        None => "<doc/>".to_string(),
    };
    if let Ok(kib) = std::env::var(CHILD) {
        let kib: usize = kib.parse().expect("KiB");
        let out = std::thread::Builder::new()
            .stack_size(kib << 10)
            .spawn(move || {
                if let Some(expr) = xpath {
                    return xrust::parser::xpath::parse::<xrust::trees::smite::RNode>(
                        &expr, None, None,
                    )
                    .map(|_| String::new())
                    .map_err(|e| e.to_string());
                }
                let compiled = ikigai_xslt::CompiledStylesheet::compile(&style)?;
                compiled.transform(&src, false)
            })
            .expect("spawn")
            .join()
            .expect("no panic");
        match out {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("refused: {e}");
                std::process::exit(3)
            }
        }
    }
    let runs = |kib: usize| {
        let status = Command::new(std::env::current_exe().unwrap())
            .args(&args)
            .env(CHILD, kib.to_string())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        match status.code() {
            Some(0) => true,
            Some(3) => panic!("the transform is refused, not a stack question"),
            _ => false,
        }
    };
    let (mut lo, mut hi) = (16usize, 512 << 10);
    assert!(runs(hi), "does not run in 512 MiB");
    while hi - lo > 4 {
        let mid = (lo + hi) / 2;
        if runs(mid) {
            hi = mid
        } else {
            lo = mid
        }
    }
    println!("{hi} KiB");
}

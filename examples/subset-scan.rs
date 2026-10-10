//! List every construct in a stylesheet that xrust answers wrongly without an error, and
//! that compiling it would therefore refuse (`ikigai_xslt::subset`, ledger #193).
//!
//! ```sh
//! cargo run --example subset-scan -- path/to/stylesheet.xsl [more.xsl …]
//! ```
//!
//! Exits 0 when no stylesheet holds one, 1 when any does, 2 on an unreadable file.

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: subset-scan <stylesheet.xsl>…");
        std::process::exit(2);
    }
    let mut any = false;
    for p in &paths {
        let text = match std::fs::read_to_string(p) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("{p}: {e}");
                std::process::exit(2);
            }
        };
        match ikigai_xslt::subset::silent_constructs(&text) {
            Ok(found) if found.is_empty() => println!("{p}: none ({} bytes)", text.len()),
            Ok(found) => {
                any = true;
                println!("{p}: {} construct(s)", found.len());
                for silent in found {
                    println!(
                        "  <{} {}=\"{}\">: {}",
                        silent.element,
                        silent.attribute,
                        silent.value,
                        silent.construct.readme_row()
                    );
                }
            }
            Err(e) => {
                eprintln!("{p}: {e}");
                std::process::exit(2);
            }
        }
    }
    std::process::exit(if any { 1 } else { 0 });
}

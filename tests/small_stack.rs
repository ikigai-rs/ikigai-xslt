//! Ordinary stylesheets run on wasm's default 1 MiB stack (ledger #3702).
//!
//! On wasm a transform runs inline, on the module's stack. Through 0.8.1 the ikigai Book's RSS
//! walkthrough trapped there inside xrust's XPath parser, which cost ~150 KiB of stack a
//! bracket in release: its date template's `substring(concat(…), number(string-length(…)) - 1,
//! 2)` needed ~2.6 MiB natively. Each probe runs `CompiledStylesheet` directly — no sized
//! thread — on a 1 MiB thread in a CHILD process, because an overflow aborts the process.

use std::process::Command;

const PROBE: &str = "IKIGAI_XSLT_SMALL_STACK_PROBE";

/// A 1 MiB stack, rust-lld's default for wasm32.
const STACK: usize = 1 << 20;

/// The walkthrough's `items.xsl` date conversion, on a fixed RFC 822 date.
const RSS_DATE: &str = r#"<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/">
    <xsl:variable name="r" select="normalize-space(substring-after(/d, ','))"/>
    <xsl:variable name="day" select="substring-before($r, ' ')"/>
    <xsl:variable name="r2" select="substring-after($r, ' ')"/>
    <xsl:variable name="mon" select="substring(substring-before($r2, ' '), 1, 3)"/>
    <xsl:variable name="m" select="number(string-length(substring-before('JanFebMarAprMayJunJulAugSepOctNovDec', $mon))) div 3 + 1"/>
    <xsl:variable name="mm" select="substring(concat('0', $m), number(string-length(concat('0', $m))) - 1, 2)"/>
    <xsl:variable name="dd" select="substring(concat('0', $day), number(string-length(concat('0', $day))) - 1, 2)"/>
    <xsl:value-of select="concat($mm, '-', $dd)"/>
  </xsl:template>
</xsl:stylesheet>"#;

/// An XPath nesting `n` parentheses.
fn parens(n: usize) -> String {
    format!(
        r#"<xsl:stylesheet xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="text"/>
  <xsl:template match="/"><xsl:value-of select="{}1{}"/></xsl:template>
</xsl:stylesheet>"#,
        "(".repeat(n),
        ")".repeat(n)
    )
}

fn inputs(case: &str) -> (String, String) {
    let doc = "<d>Wed, 7 Oct 2026 10:00:00 GMT</d>".to_string();
    match case {
        "rss-date" => (doc, RSS_DATE.to_string()),
        "parens-8" => (doc, parens(8)),
        other => panic!("no probe named {other}"),
    }
}

/// The child's half: inert unless a parent named a probe.
#[test]
fn probe_child() {
    let Ok(case) = std::env::var(PROBE) else {
        return;
    };
    let (src, style) = inputs(&case);
    let outcome = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || {
            ikigai_xslt::CompiledStylesheet::compile(&style)
                .and_then(|compiled| compiled.transform(&src, true))
        })
        .unwrap()
        .join()
        .unwrap();
    println!("\nOUTCOME {outcome:?}");
    std::process::exit(0);
}

fn probe(case: &str) -> String {
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "probe_child", "--nocapture", "--test-threads=1"])
        .env(PROBE, case)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{case}: the child died on a 1 MiB stack ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .find(|l| l.contains("overflowed its stack"))
            .unwrap_or_default()
    );
    let at = stdout.find("\nOUTCOME ").expect("the child reported");
    stdout[at + "\nOUTCOME ".len()..]
        .lines()
        .next()
        .unwrap()
        .to_string()
}

#[test]
fn the_rss_walkthrough_date_template_runs_on_one_mib() {
    assert_eq!(probe("rss-date"), r#"Ok("10-07")"#);
}

#[test]
fn eight_nested_parentheses_run_on_one_mib() {
    assert_eq!(probe("parens-8"), r#"Ok("1")"#);
}

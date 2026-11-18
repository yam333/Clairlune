//! Embeds every exported sheet in `assets/clawd/` (`<name>.json` + its PNG), so
//! exporting art needs no code change.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

fn main() {
    let dir = Path::new("assets/clawd");
    println!("cargo:rerun-if-changed=assets/clawd");
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "json"))
                .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut out = String::from(
        "/// `(name, json, png)` for every embedded sheet, sorted by name.\npub const SHEETS: &[(&str, &str, &[u8])] = &[\n",
    );
    for n in &names {
        println!("cargo:rerun-if-changed=assets/clawd/{n}.json");
        println!("cargo:rerun-if-changed=assets/clawd/{n}.png");
        writeln!(
            out,
            "    ({n:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/clawd/{n}.json\")), include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/clawd/{n}.png\"))),"
        )
        .unwrap();
    }
    out.push_str("];\n");
    let dest = Path::new(&env::var("OUT_DIR").unwrap()).join("sheets.rs");
    fs::write(dest, out).unwrap();
}

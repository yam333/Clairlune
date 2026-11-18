//! Golden images: 10 fixed seeds x 2 themes at 640x360.
//!
//! Two checks per image:
//! 1. its hash matches `golden/hashes.txt` (byte-identical output; this is the
//!    cross-OS determinism check in CI), and
//! 2. its pixels match the committed PNG, with a per-pixel diff count on failure.
//!
//! Regenerate intentionally with `CLAIRLUNE_BLESS=1 cargo test -p clairlune-tests --test golden`
//! and bump `GENERATOR_VERSION` when existing recipes change. Mismatching renders
//! are written to `target/golden-actual/` for inspection.

use clairlune_render::encode_png;
use clairlune_scene::Theme;
use clairlune_tests::{
    GOLDEN_H, GOLDEN_SEEDS, GOLDEN_W, blessing, decode_indexed_png, golden_dir, image_hash,
    still_opt,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Allowed fraction of differing pixels against the committed PNG. Zero:
/// the generator is designed to be byte-identical everywhere.
const TOLERANCE: f64 = 0.0;

#[test]
fn golden_images() {
    let dir = golden_dir();
    let images = dir.join("images");
    let manifest_path = dir.join("hashes.txt");
    let actual_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/golden-actual");

    let expected: BTreeMap<String, String> = if blessing() {
        BTreeMap::new()
    } else {
        std::fs::read_to_string(&manifest_path)
            .unwrap_or_else(|_| {
                panic!(
                    "missing {}; run with CLAIRLUNE_BLESS=1",
                    manifest_path.display()
                )
            })
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .filter_map(|l| {
                l.split_once(' ')
                    .map(|(k, v)| (k.to_string(), v.trim().to_string()))
            })
            .collect()
    };

    let mut manifest = String::from(
        "# name hash (FNV-1a of size + palette + pixels). Regenerate with CLAIRLUNE_BLESS=1.\n",
    );
    let mut failures = Vec::new();
    for &seed in &GOLDEN_SEEDS {
        for theme in [Theme::Dark, Theme::Light] {
            let name = format!("seed-{seed}-{}", theme.as_str());
            let Some(s) = still_opt(seed, GOLDEN_W, GOLDEN_H, theme, true) else {
                // Theme-locked: the scene has no variant for this theme.
                writeln!(manifest, "{name} locked").unwrap();
                if !blessing() && expected.get(&name).map(String::as_str) != Some("locked") {
                    failures.push(format!(
                        "{name}: theme-locked now, but the manifest expects an image"
                    ));
                }
                continue;
            };
            let rgb = s.palette.rgb_bytes();
            let hash = format!("{:016x}", image_hash(&s.image, &rgb));
            writeln!(manifest, "{name} {hash}").unwrap();

            let mut png = Vec::new();
            encode_png(&s.image, &s.palette, &mut png).unwrap();
            let png_path = images.join(format!("{name}.png"));
            if blessing() {
                std::fs::create_dir_all(&images).unwrap();
                std::fs::write(&png_path, &png).unwrap();
                continue;
            }

            let mut problems = Vec::new();
            match expected.get(&name) {
                Some(h) if *h == hash => {}
                Some(h) => problems.push(format!("hash {hash} != expected {h}")),
                None => problems.push("not in hashes.txt".to_string()),
            }
            match std::fs::read(&png_path) {
                Ok(bytes) => {
                    let (golden, golden_rgb) = decode_indexed_png(bytes);
                    if (golden.width, golden.height) != (s.image.width, s.image.height) {
                        problems.push("size differs".to_string());
                    } else {
                        let diff = golden
                            .pixels
                            .iter()
                            .zip(&s.image.pixels)
                            .filter(|(a, b)| a != b)
                            .count();
                        let frac = diff as f64 / golden.pixels.len() as f64;
                        if frac > TOLERANCE {
                            problems.push(format!("{diff} pixels differ ({:.4} %)", frac * 100.0));
                        }
                        if golden_rgb != rgb {
                            problems.push("palette differs".to_string());
                        }
                    }
                }
                Err(_) => problems.push(format!("missing {}", png_path.display())),
            }
            if !problems.is_empty() {
                std::fs::create_dir_all(&actual_dir).ok();
                std::fs::write(actual_dir.join(format!("{name}.png")), &png).ok();
                failures.push(format!("{name}: {}", problems.join("; ")));
            }
        }
    }

    if blessing() {
        std::fs::write(&manifest_path, manifest).unwrap();
        eprintln!(
            "blessed {} golden images in {}",
            GOLDEN_SEEDS.len() * 2,
            dir.display()
        );
        return;
    }
    assert!(
        failures.is_empty(),
        "golden mismatches:\n  {}",
        failures.join("\n  ")
    );
}

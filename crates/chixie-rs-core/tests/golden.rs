//! Byte-for-byte G-code golden captured from the pre-refactor writing pipeline
//! (`writing::text_to_gcode`, commit afafaba). The layout/emit split must reproduce it exactly
//! with all effects off. Regenerate only on purpose: `BLESS=1 cargo test --test golden`.

use std::path::PathBuf;

use chixie_rs_core::font::Font;
use chixie_rs_core::gcode::{DeviceProfile, emit_document};
use chixie_rs_core::layout::{PageParams, layout};

const TEXT: &str = "你好 A.\n好你";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[test]
fn writing_matches_pre_refactor_golden() {
    let font = Font::from_json(&std::fs::read(fixture("font_fixture.json")).unwrap()).unwrap();
    // Same knobs as `chixie write --scale 3` (origin defaults 0,24; zero not applied here).
    let p = PageParams { mm_per_unit: 3.0, origin: (0.0, 24.0), ..Default::default() };
    let out = emit_document(&layout(TEXT, &font, &p), &DeviceProfile::default());

    let golden = fixture("golden_pre_refactor.gcode");
    if std::env::var_os("BLESS").is_some() {
        std::fs::write(&golden, &out).unwrap();
    }
    let expected = std::fs::read_to_string(&golden).expect("golden missing; run with BLESS=1 once");
    assert_eq!(out, expected);
}

/// Fixed-seed snapshot with jitter + base + shape deformation on. Created after the effects
/// landed (it is NOT pre-refactor output); it pins the RNG stream and the effect math.
/// Re-blessed 2026-11-07 when randomness became keyed per character id (WYSIWYG editing must not
/// reshuffle other characters); `layout()` uses ids 0..n.
#[test]
fn styled_output_is_stable() {
    use chixie_rs_core::deformation::Deformation;
    use chixie_rs_core::handwriting::HandwritingStyle;

    let font = Font::from_json(&std::fs::read(fixture("font_fixture.json")).unwrap()).unwrap();
    let p = PageParams {
        mm_per_unit: 1.0,
        origin: (0.0, 0.0),
        style: HandwritingStyle { enabled: true, seed: 1, ..Default::default() },
        deformation: Deformation { enabled: true, base: true, shape: true, ..Default::default() },
        ..Default::default()
    };
    let out = emit_document(&layout(TEXT, &font, &p), &DeviceProfile::default());

    let golden = fixture("golden_styled_seed1.gcode");
    if std::env::var_os("BLESS").is_some() {
        std::fs::write(&golden, &out).unwrap();
    }
    let expected = std::fs::read_to_string(&golden).expect("golden missing; run with BLESS=1 once");
    assert_eq!(out, expected);
}

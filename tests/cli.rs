//! End-to-end test: convert a file, convert it back, expect the original bytes.

use std::fs;
use std::process::Command;

fn adf_bin() -> &'static str {
    env!("CARGO_BIN_EXE_gta-adf-converter")
}

#[test]
fn convert_round_trips() {
    let dir = std::env::temp_dir().join(format!("adf-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();

    let original = dir.join("song.mp3");
    let converted = dir.join("song.adf");
    let restored = dir.join("restored.mp3");

    // A blob with every byte value so we exercise the full XOR range.
    let data: Vec<u8> = (0..=255u8).cycle().take(100_003).collect();
    fs::write(&original, &data).unwrap();

    // MP3 → ADF
    let enc = Command::new(adf_bin())
        .args([original.to_str().unwrap(), converted.to_str().unwrap(), "--quiet"])
        .status()
        .unwrap();
    assert!(enc.success());

    // The converted bytes must actually differ from the input.
    assert_ne!(fs::read(&converted).unwrap(), data);

    // ADF → MP3, using the same single operation.
    let dec = Command::new(adf_bin())
        .args([converted.to_str().unwrap(), restored.to_str().unwrap(), "--quiet"])
        .status()
        .unwrap();
    assert!(dec.success());

    assert_eq!(fs::read(&restored).unwrap(), data);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn refuses_same_input_and_output() {
    let status = Command::new(adf_bin())
        .args(["same.bin", "same.bin"])
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn missing_output_arg_fails() {
    let status = Command::new(adf_bin())
        .args(["only-input.mp3"])
        .status()
        .unwrap();
    assert!(!status.success());
}

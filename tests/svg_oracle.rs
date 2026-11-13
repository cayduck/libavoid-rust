//! Differential test of the SVG debug output against the real C++:
//! `Router::outputInstanceToSVG()` (the SVG with the embedded reproduction
//! code: `ShapeRef`/`JunctionRef`/`ConnRef`/`ConnEnd`/`ShapeConnectionPin`
//! `outputCode()`, clusters, routing parameters and options) and
//! `Router::outputDiagramSVG()`.
//!
//! `tools/oracle/svg_oracle.sh` runs every golden-mode upstream fixture
//! (`tests/fixtures/`) and every audit scene with goldens
//! (`tests/audit_scenes/`) through a patched copy of upstream that writes
//! both SVGs at every diagram dump, and records the sha256 and size of each
//! file in `tests/golden-svg/manifest.txt` (a few complete files are in
//! `tests/golden-svg/full/`).  This test runs the same op scripts through the
//! port (all the usual golden comparisons included), takes the SVGs at the
//! same points and requires byte-identical output.
//!
//! Environment:
//!  * `SVG_SCRIPT=name`  only that script
//!  * `SVG_DUMP=dir`     write the port's SVGs to `dir/<script>/` (for
//!    diffing against `target/svg-oracle/run/<script>/output/`)
//!  * `LIBAVOID_SLOW_FIXTURES=1` also the four slow fixtures (release
//!    builds recommended)
//!  * `SVG_SUBSAMPLE=k`  only every k-th script (default 4 in debug builds)

#![allow(clippy::uninlined_format_args)]

mod common;
#[path = "common/fixture_ops.rs"]
mod fixture_ops;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const SLOW: [&str; 4] = ["performance01", "slowrouting", "reallyslowrouting", "unsatisfiableRangeAssertion"];

/// SHA-256 (FIPS 180-4), for comparing with the manifest without storing
/// the files.
fn sha256(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{:08x}", x)).collect()
}

#[test]
fn sha256_known_answers() {
    assert_eq!(sha256(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    let long = vec![b'a'; 1000];
    assert_eq!(sha256(&long), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
}

/// `script -> (file -> (sha256, bytes))`.
fn read_manifest() -> BTreeMap<String, BTreeMap<String, (String, usize)>> {
    let path = format!("{}/tests/golden-svg/manifest.txt", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {}", path, e));
    let mut out: BTreeMap<String, BTreeMap<String, (String, usize)>> = BTreeMap::new();
    let mut cur: Option<String> = None;
    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks[0] == "script" {
            cur = Some(toks[1].to_string());
            out.entry(toks[1].to_string()).or_default();
            continue;
        }
        let script = cur.as_ref().expect("manifest entry before any script line");
        out.get_mut(script)
            .unwrap()
            .insert(toks[0].to_string(), (toks[1].to_string(), toks[2].parse().unwrap()));
    }
    out
}

fn full_golden(file: &str) -> Option<String> {
    std::fs::read_to_string(format!("{}/tests/golden-svg/full/{}", env!("CARGO_MANIFEST_DIR"), file)).ok()
}

fn check_script(script: &str, expected: &BTreeMap<String, (String, usize)>) -> Result<usize, String> {
    let (_, svgs) = fixture_ops::run_fixture_capturing_svg(script);
    if let Ok(dir) = std::env::var("SVG_DUMP") {
        let d = format!("{}/{}", dir, script);
        std::fs::create_dir_all(&d).unwrap();
        for (f, s) in svgs.iter() {
            std::fs::write(format!("{}/{}", d, f), s).unwrap();
        }
    }
    let got: Vec<&String> = svgs.keys().collect();
    let want: Vec<&String> = expected.keys().collect();
    if got != want {
        return Err(format!("{}: the port wrote {:?}, the C++ {:?}", script, got, want));
    }
    for (file, (hash, bytes)) in expected.iter() {
        let svg = &svgs[file];
        if sha256(svg.as_bytes()) != *hash {
            let mut msg = format!(
                "{}: {} differs from the C++ ({} bytes, C++ {} bytes)",
                script,
                file,
                svg.len(),
                bytes
            );
            if let Some(full) = full_golden(file) {
                if let Some((i, (a, b))) = svg.lines().zip(full.lines()).enumerate().find(|(_, (a, b))| a != b) {
                    msg.push_str(&format!("\n  first difference at line {}:\n  rust: {}\n  c++:  {}", i + 1, a, b));
                }
            }
            return Err(msg);
        }
    }
    Ok(expected.len())
}

#[test]
fn svg_output_matches_cpp() {
    let manifest = read_manifest();
    let only = std::env::var("SVG_SCRIPT").ok();
    let slow = std::env::var_os("LIBAVOID_SLOW_FIXTURES").is_some_and(|v| !v.is_empty() && v != "0");
    let subsample: usize = std::env::var("SVG_SUBSAMPLE")
        .ok()
        .map(|v| v.parse().expect("SVG_SUBSAMPLE"))
        .unwrap_or(if cfg!(debug_assertions) { 4 } else { 1 });
    let scripts: Vec<(&String, &BTreeMap<String, (String, usize)>)> = manifest
        .iter()
        .filter(|(s, _)| match &only {
            Some(o) => *s == o,
            None => slow || !SLOW.contains(&s.as_str()),
        })
        .enumerate()
        .filter(|(i, _)| only.is_some() || i % subsample == 0)
        .map(|(_, e)| e)
        .collect();
    assert!(!scripts.is_empty(), "no scripts selected");

    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let compared = AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(8);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= scripts.len() {
                    break;
                }
                let (script, expected) = scripts[i];
                let r = std::panic::catch_unwind(|| check_script(script, expected));
                match r {
                    Ok(Ok(n)) => {
                        compared.fetch_add(n, Ordering::SeqCst);
                    }
                    Ok(Err(m)) => failures.lock().unwrap().push(m),
                    Err(_) => failures.lock().unwrap().push(format!("{}: panicked", script)),
                }
            });
        }
    });
    let failures = failures.into_inner().unwrap();
    eprintln!(
        "svg oracle: {} scripts, {} SVG files compared, {} scripts differ",
        scripts.len(),
        compared.load(Ordering::SeqCst),
        failures.len()
    );
    assert!(failures.is_empty(), "SVG output differs from the C++:\n{}", failures.join("\n"));
}

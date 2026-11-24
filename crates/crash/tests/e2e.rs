//! A real freeze, panic and crash, end to end on each system: examples/crashy.rs does it, its
//! monitor writes the minidump, reads it with the example's own debug info, and the report names
//! the function where it happened.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

fn crashy() -> PathBuf {
    // Cargo keeps examples under the profile directory, but its test binaries can live in
    // either deps/ or build/<package>/<hash>/out/. Find the shared profile ancestor.
    let exe = std::env::current_exe().unwrap();
    example_from(&exe).unwrap_or_else(|| {
        panic!(
            "examples/crashy not found above {}; build the examples first",
            exe.display()
        )
    })
}

fn example_from(exe: &std::path::Path) -> Option<PathBuf> {
    let name = format!("crashy{}", std::env::consts::EXE_SUFFIX);
    exe.ancestors()
        .skip(1)
        .map(|dir| dir.join("examples").join(&name))
        .find(|path| path.is_file())
}

#[test]
fn helper_is_found_in_both_cargo_test_layouts() {
    let target = tempfile::tempdir().unwrap();
    let profile = target.path().join("debug");
    let expected = profile
        .join("examples")
        .join(format!("crashy{}", std::env::consts::EXE_SUFFIX));
    std::fs::create_dir_all(expected.parent().unwrap()).unwrap();
    std::fs::write(&expected, []).unwrap();
    for relative in ["deps/e2e-hash", "build/sdl-crash/hash/out/e2e-hash"] {
        assert_eq!(
            example_from(&profile.join(relative)),
            Some(expected.clone())
        );
    }
    assert_eq!(example_from(&target.path().join("elsewhere/e2e")), None);
}

/// Run crashy in `mode` and wait (up to 90 s: reading the dump's symbols takes a while in a
/// debug build) for its report.
fn report(mode: &str) -> String {
    let data = tempfile::tempdir().unwrap();
    /// The program, ended and reaped however the test leaves (a failed assert too).
    struct Running(std::process::Child);
    impl Drop for Running {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let _child = Running(
        Command::new(crashy())
            .arg(mode)
            .arg(data.path())
            .spawn()
            .expect("examples/crashy is built with the tests"),
    );
    let crashes = data.path().join("crashes");
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let txt = std::fs::read_dir(&crashes).ok().and_then(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|x| x == "txt"))
        });
        if let Some(t) = txt {
            // Written in one go after the dump was read; give the write a moment.
            std::thread::sleep(Duration::from_millis(300));
            return std::fs::read_to_string(&t).unwrap();
        }
        if Instant::now() > deadline {
            panic!("no report for {mode} in {}", crashes.display());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn a_freeze_reports_the_frozen_function() {
    let r = report("freeze");
    assert!(r.contains("The window stopped responding"), "{r}");
    assert!(r.contains("crashy_frozen_here"), "{r}");
    assert!(
        r.contains("Signature: freeze in crashy::crashy_frozen_here"),
        "{r}"
    );
}

#[test]
fn a_panic_reports_its_message_and_function() {
    let r = report("panic");
    assert!(r.contains("crashy: a test panic"), "{r}");
    assert!(
        r.contains("Signature: panic in crashy::crashy_panics_here"),
        "{r}"
    );
}

#[test]
fn a_native_crash_reports_where_it_crashed() {
    let r = report("crash");
    assert!(r.contains("Description: SDL crashed"), "{r}");
    assert!(
        r.contains("Signature: crash in crashy::crashy_crashes_here"),
        "{r}"
    );
}

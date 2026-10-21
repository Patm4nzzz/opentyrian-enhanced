//! Targeted regressions: effective reads and path confinement.
use std::path::{Path, PathBuf};
use tyrian_mods::{folder, json, levels, Diagnostics, ModFolder};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tyrian-security-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("mod")).unwrap();
        std::fs::write(path.join("mod/mod.json"), r#"{"format_version":1}"#).unwrap();
        Self(path)
    }
    fn root(&self) -> PathBuf {
        self.0.join("mod")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn security_actual_read_is_bounded_even_if_metadata_says_zero() {
    let path = Path::new("/proc/self/maps");
    assert_eq!(std::fs::metadata(path).unwrap().len(), 0);
    assert!(folder::read_text_bounded(path, 32).is_err());
}

#[test]
fn security_readme_sparse_file_reads_only_prefix() {
    use std::io::Write;
    let f = Fixture::new();
    let mut file = std::fs::File::create(f.root().join("README.md")).unwrap();
    file.write_all(&vec![b'a'; folder::MOD_README_MAX]).unwrap();
    file.set_len(1 << 30).unwrap();
    let m = ModFolder::read(&f.root(), "mod", &mut Diagnostics::default());
    assert_eq!(m.readme, "a".repeat(folder::MOD_README_MAX));
}

#[cfg(unix)]
#[test]
fn security_special_files_rejected_and_internal_link_allowed() {
    let f = Fixture::new();
    assert!(folder::read_text_bounded(Path::new("/dev/null"), 32).is_err());
    assert!(folder::png_header(Path::new("/dev/null")).is_err());
    std::fs::write(f.root().join("inside"), "internal text").unwrap();
    std::os::unix::fs::symlink(f.root().join("inside"), f.root().join("README.md")).unwrap();
    let m = ModFolder::read(&f.root(), "mod", &mut Diagnostics::default());
    assert_eq!(m.readme, "internal text");
    assert!(m.root.is_some());
}

#[test]
fn security_text_regular_exact_limit() {
    let f = Fixture::new();
    let p = f.root().join("text");
    std::fs::write(&p, "abcd").unwrap();
    assert_eq!(folder::read_text_bounded(&p, 4).unwrap(), "abcd");
    assert!(folder::read_text_bounded(&p, 3).is_err());
    assert!(folder::read_text_bounded(&f.root(), 4).is_err());
}

#[cfg(unix)]
#[test]
fn security_mod_json_external_symlink_rejected() {
    let f = Fixture::new();
    std::fs::write(f.0.join("outside.json"), r#"{"format_version":1,"name":"secret"}"#).unwrap();
    std::fs::remove_file(f.root().join("mod.json")).unwrap();
    std::os::unix::fs::symlink(f.0.join("outside.json"), f.root().join("mod.json")).unwrap();
    let m = ModFolder::read(&f.root(), "mod", &mut Diagnostics::default());
    assert!(m.root.is_none());
}

#[cfg(unix)]
#[test]
fn security_readme_external_symlink_rejected() {
    let f = Fixture::new();
    std::fs::write(f.0.join("secret"), "secret").unwrap();
    std::os::unix::fs::symlink(f.0.join("secret"), f.root().join("README.md")).unwrap();
    let m = ModFolder::read(&f.root(), "mod", &mut Diagnostics::default());
    assert!(m.readme.is_empty());
}

#[cfg(unix)]
#[test]
fn security_external_png_file_and_directory_rejected() {
    for directory in [false, true] {
        let f = Fixture::new();
        std::fs::create_dir_all(f.0.join("outside")).unwrap();
        let mut png = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
        png.extend_from_slice(&1u32.to_be_bytes());
        png.extend_from_slice(&1u32.to_be_bytes());
        std::fs::write(f.0.join("outside/test.png"), png).unwrap();
        std::fs::create_dir_all(f.root().join("sprites")).unwrap();
        if directory {
            std::os::unix::fs::symlink(f.0.join("outside"), f.root().join("sprites/shots")).unwrap();
        } else {
            std::fs::create_dir_all(f.root().join("sprites/shots")).unwrap();
            std::os::unix::fs::symlink(f.0.join("outside/test.png"), f.root().join("sprites/shots/test.png")).unwrap();
        }
        let m = ModFolder::read(&f.root(), "mod", &mut Diagnostics::default());
        assert!(m.pngs.is_empty(), "directory={directory}");
    }
}

/// No known name: only the rejection of the symbolic link is tested here.
struct NoNames;
impl levels::LevelNames for NoNames {
    fn enemy(&mut self, name: &str) -> Result<u16, String> {
        Err(format!("unknown '{name}'"))
    }
    fn special(&mut self, name: &str) -> Result<u8, String> {
        Err(format!("unknown '{name}'"))
    }
}

#[cfg(unix)]
#[test]
fn security_external_level_file_and_directory_rejected() {
    for directory in [false, true] {
        let f = Fixture::new();
        std::fs::create_dir_all(f.0.join("outside")).unwrap();
        std::fs::write(f.0.join("outside/ep1_01.json"), r#"{"episode":1,"lvl_file_num":1,"events":[]}"#).unwrap();
        if directory {
            std::os::unix::fs::symlink(f.0.join("outside"), f.root().join("levels")).unwrap();
        } else {
            std::fs::create_dir_all(f.root().join("levels")).unwrap();
            std::os::unix::fs::symlink(f.0.join("outside/ep1_01.json"), f.root().join("levels/ep1_01.json")).unwrap();
        }
        let levels = levels::read_mod_levels(&f.root(), "mod", &mut NoNames, &mut Diagnostics::default());
        assert!(levels.is_empty(), "directory={directory}");
    }
}

#[test]
fn security_resolve_untrusted_indices_without_overflow() {
    use tyrian_mods::{
        manifest::{SpriteName, SpriteNames},
        ModSet, PngFile,
    };
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data");
    let (_, items) = tyrian_mods::episode_items(&data, 1).unwrap();
    for basename in ["fresh", "bad", "2147483647", "-1", "0"] {
        let m = ModFolder {
            folder: "mod".into(),
            enabled: true,
            root: Some(json::parse("{}").unwrap()),
            pngs: vec![PngFile {
                path: PathBuf::from("unused.png"),
                dir: "shots".into(),
                basename: basename.into(),
                width: 1,
                height: 1,
            }],
            ..Default::default()
        };
        let set = ModSet {
            data_dir: data.clone(),
            folders: vec![m],
            diagnostics: Diagnostics::default(),
            manifest: SpriteNames {
                rows: vec![SpriteName { name: "bad".into(), section: "shots".into(), index: i32::MAX }],
            },
            originals: Default::default(),
        };
        let res = set.resolve(1, &items);
        if basename == "fresh" {
            assert_eq!(res.sprites.len(), 1);
            assert!(res.sprites[0].index < 1024);
        } else {
            assert!(res.sprites.is_empty(), "{basename}");
        }
    }
}

#[test]
fn security_manifest_indices_respect_section_capacity() {
    let v = json::parse(
        r#"{"shots":{"bad":2147483647,"negative":-1,"edge":1024,"valid":1023},"portraits":{"bad":32,"valid":31}}"#,
    )
    .unwrap();
    let mut diag = Diagnostics::default();
    let names = tyrian_mods::manifest::SpriteNames::from_value(&v, &mut diag);
    assert!(names.lookup("bad").is_none());
    assert!(names.lookup("negative").is_none());
    assert!(names.lookup("edge").is_none());
    assert_eq!(names.lookup_section("valid", "shots"), Some(1023));
    assert_eq!(names.lookup_section("valid", "portraits"), Some(31));
}

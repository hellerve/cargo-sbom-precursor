use cargo_sbom_precursor::{DependencyKind, Error, Precursor};

// Written by `cargo +nightly build -Z sbom` (1.95.0-nightly) for the binary `app` in a workspace
// where `app` has a build script using `gen`, a proc-macro `mac`, and depends on `util` with the
// `extra` feature, while `gen` depends on `util` without it. Absolute paths are replaced.
const WORKSPACE: &str = include_str!("fixtures/workspace.cargo-sbom.json");

fn workspace() -> Precursor {
    Precursor::from_slice(WORKSPACE.as_bytes()).unwrap()
}

fn with(edit: impl FnOnce(&mut serde_json::Value)) -> Result<Precursor, Error> {
    let mut json: serde_json::Value = serde_json::from_str(WORKSPACE).unwrap();
    edit(&mut json);
    Precursor::from_slice(&serde_json::to_vec(&json).unwrap())
}

#[test]
fn reads_cargo_output() {
    let precursor = workspace();
    let root = precursor.root_crate();
    assert_eq!(root.id, "path+file:///path/to/ws/app#0.1.0");
    assert_eq!(root.kind, ["bin"]);
    assert!(!precursor.target.is_empty());
    assert_eq!(precursor.rustc.version, "1.95.0-nightly");

    let deps: Vec<_> = root
        .dependencies
        .iter()
        .map(|dep| (precursor.crates[dep.index].kind[0].as_str(), dep.kind))
        .collect();
    assert_eq!(
        deps,
        [
            ("custom-build", DependencyKind::Build),
            ("proc-macro", DependencyKind::Build),
            ("lib", DependencyKind::Normal),
        ]
    );
}

#[test]
fn package_can_appear_with_different_features() {
    let precursor = workspace();
    let util: Vec<_> = precursor
        .crates
        .iter()
        .filter(|krate| krate.id == "path+file:///path/to/ws/util#0.2.0")
        .map(|krate| krate.features.clone())
        .collect();
    assert_eq!(util, [vec![], vec!["extra".to_owned()]]);
}

#[test]
fn round_trips() {
    let precursor = workspace();
    let json = serde_json::to_vec(&precursor).unwrap();
    assert_eq!(Precursor::from_slice(&json).unwrap(), precursor);
}

#[test]
fn ignores_unknown_fields() {
    let precursor = with(|json| {
        json["future_field"] = true.into();
        json["crates"][0]["future_field"] = 1.into();
    });
    assert_eq!(precursor.unwrap(), workspace());
}

#[test]
fn accepts_unknown_dependency_kinds() {
    let precursor = with(|json| json["crates"][0]["dependencies"][0]["kind"] = "dev".into());
    assert_eq!(
        precursor.unwrap().root_crate().dependencies[0].kind,
        DependencyKind::Unknown
    );
}

#[test]
fn accepts_missing_commit_hash() {
    let precursor = with(|json| json["rustc"]["commit_hash"] = serde_json::Value::Null);
    assert_eq!(precursor.unwrap().rustc.commit_hash, None);
}

#[test]
fn rejects_other_versions() {
    let precursor = with(|json| json["version"] = 2.into());
    assert!(matches!(precursor, Err(Error::UnsupportedVersion(2))));
}

#[test]
fn rejects_out_of_bounds_indices() {
    let root = with(|json| json["root"] = 99.into());
    assert!(matches!(root, Err(Error::InvalidIndex(99))));
    let dep = with(|json| json["crates"][0]["dependencies"][0]["index"] = 99.into());
    assert!(matches!(dep, Err(Error::InvalidIndex(99))));
}

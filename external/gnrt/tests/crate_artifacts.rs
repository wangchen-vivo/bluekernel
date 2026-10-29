// Copyright (c) 2026 vivo Mobile Communication Co., Ltd.
// SPDX-License-Identifier: Apache-2.0

use gnrt_lib::{
    collect::{CrateDeclaration, Declaration, DeclarationGroups, Isolation},
    config::BuildConfig,
    deps::{collect_dependencies, LibType},
    generate,
    group::Group,
    package::{Dependency, Package, ResolutionContext},
    paths::BlueOsPaths,
    sync_context::{CrateInfo, SyncContext},
};
use std::{collections::BTreeMap, fs, path::Path, process::Command};

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn run(root: &Path, program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{program} {args:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn generated_rust_artifacts_preserve_names_versions_and_build_scripts() {
    let build = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build");
    assert!(build.join("toolchain/blueos.gni").exists());
    let fixture = tempfile::tempdir().unwrap();
    let canonical_root = fixture.path().canonicalize().unwrap();
    let root = canonical_root.as_path();
    let rustc = run(root, "rustup", &["which", "--toolchain", "stable", "rustc"])
        .trim()
        .to_owned();
    let version = run(root, &rustc, &["-vV"]);
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap();
    for relative in [
        "CARGO_BUILD.gn.hbs",
        "templates/rust.gni",
        "templates/cargo_crate.gni",
        "templates/cargo.gni",
        "toolchain/blueos.gni",
        "config/clippy.gni",
        "scripts/run_rustc.py",
        "scripts/run_build_script.py",
        "scripts/action_helpers.py",
        "scripts/resolve_cargo_aliases.py",
    ] {
        write(
            root,
            &format!("build/{relative}"),
            &fs::read_to_string(build.join(relative)).unwrap(),
        );
    }
    write(
        root,
        "build/scripts/copy_binary.py",
        "import shutil, sys\nshutil.copyfile(sys.argv[-2], sys.argv[-1])\n",
    );
    write(
        root,
        ".gn",
        "buildconfig = \"//build/BUILDCONFIG.gn\"\nscript_executable = \"python3\"\n",
    );
    write(
        root,
        "build/config/gnrt_sync.gni",
        "gnrt_crate_index = read_file(\"//third_party/gnrt_crate_index.json\", \"json\")\n",
    );
    write(root, "build/gnrt_config.toml", "[resolve]\nroot = \"fixture\"\nremove_crates = []\nremove_crates_when = []\ncargo_lockfile = \"Cargo.lock\"\n[gn]\nbuild_file_template = \"CARGO_BUILD.gn.hbs\"\n");
    write(
        root,
        "build/BUILDCONFIG.gn",
        &format!(
            r#"
set_default_toolchain("//build/toolchain:host")
current_os = host_os
current_cpu = host_cpu
gnrt_mode = "build"
build_ctx = {{
  toolchain_cfgs = [ "unix" ]
  toolchain_target_triple = "{host}"
  toolchain_rustc = "{rustc}"
  toolchain_cc = "clang"
  toolchain_cxx = "clang++"
  toolchain_ar = "ar"
  toolchain_arflags = []
  build_config_cflags = []
  build_config_cflags_c = []
  build_config_cflags_cc = []
  build_config_rustflags = []
  build_config_rust_profile = "debug"
}}
host_build_ctx = build_ctx
"#
        ),
    );
    write(
        root,
        "build/toolchain/BUILD.gn",
        &format!(
            r#"
import("//build/toolchain/blueos.gni")
blueos_toolchain("host") {{
  llvm_target = "{host}"
  target_triple = "{host}"
  toolchain_role = "host"
  cc = "clang"
  cxx = "clang++"
  ar = "ar"
  ld = "clang"
  rustc = "{rustc}"
  clippy_driver = "{rustc}"
  objcopy = "python3 ../build/scripts/copy_binary.py"
  current_os = host_os
  current_cpu = host_cpu
  toolchain_args = {{ current_os = host_os current_cpu = host_cpu }}
}}
"#
        ),
    );
    write(root, "Cargo.toml", "[workspace]\nresolver = \"2\"\nmembers = [\"driver\"]\nexclude = [\"third_party/codec-1.0.0\", \"third_party/codec-2.0.0\", \"third_party/leaf-1.0.0\"]\n");
    write(root, "driver/Cargo.toml", "[package]\nname = \"driver\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\ncodec_v1 = { package = \"codec\", path = \"../third_party/codec-1.0.0\" }\ncodec_v2 = { package = \"codec\", path = \"../third_party/codec-2.0.0\" }\nbridge = { path = \"../third_party/bridge-1.0.0\" }\n");
    write(root, "driver/src/lib.rs", "");
    write(
        root,
        "third_party/codec-1.0.0/Cargo.toml",
        r#"
[package]
name = "codec"
version = "1.0.0"
edition = "2021"
[lib]
name = "codec_api"
crate-type = ["rlib", "dylib"]
[dependencies]
renamed_leaf = { package = "leaf", path = "../leaf-1.0.0" }
"#,
    );
    write(
        root,
        "third_party/codec-1.0.0/src/lib.rs",
        r#"
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
pub struct Token(pub i32);
#[cfg(generated_flag)]
pub fn value() -> i32 { assert_eq!(env!("SCRIPT_VALUE"), "ready"); renamed_leaf::value() + GENERATED }
"#,
    );
    write(
        root,
        "third_party/codec-1.0.0/build.rs",
        r#"
fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut runs = std::fs::read_to_string(out.join("runs.txt")).unwrap_or_default();
    runs.push('x');
    std::fs::write(out.join("runs.txt"), runs).unwrap();
    std::fs::write(out.join("generated.rs"), "const GENERATED: i32 = 41;").unwrap();
    println!("cargo::rustc-cfg=generated_flag");
    println!("cargo::rustc-check-cfg=cfg(generated_flag)");
    println!("cargo::rustc-env=SCRIPT_VALUE=ready");
}
"#,
    );
    write(root, "third_party/leaf-1.0.0/Cargo.toml", "[package]\nname = \"leaf\"\nversion = \"1.0.0\"\nedition = \"2021\"\n[lib]\nname = \"leaf_api\"\n");
    write(
        root,
        "third_party/leaf-1.0.0/src/lib.rs",
        "pub fn value() -> i32 { 1 }\n",
    );
    write(root, "third_party/bridge-1.0.0/Cargo.toml", "[package]\nname = \"bridge\"\nversion = \"1.0.0\"\nedition = \"2021\"\n[lib]\nname = \"bridge_api\"\n[dependencies]\ncodec = { path = \"../codec-1.0.0\" }\n");
    write(
        root,
        "third_party/bridge-1.0.0/src/lib.rs",
        "pub fn verify(token: codec_api::Token) { assert_eq!(token.0, 42); }\n",
    );
    for relative in ["Cargo.toml", "src/lib.rs", "build.rs"] {
        let original =
            fs::read_to_string(root.join("third_party/codec-1.0.0").join(relative)).unwrap();
        let versioned = if relative == "Cargo.toml" {
            original.replace("version = \"1.0.0\"", "version = \"2.0.0\"")
        } else if relative == "src/lib.rs" {
            original.replace(" + GENERATED", " + GENERATED + 1")
        } else {
            original
        };
        write(
            root,
            &format!("third_party/codec-2.0.0/{relative}"),
            &versioned,
        );
    }
    let mut metadata = guppy::MetadataCommand::new();
    metadata.cargo_path(env!("CARGO"));
    metadata.current_dir(root);
    metadata.other_options(vec!["--offline".to_owned()]);
    let graph = metadata.exec().unwrap().build_graph().unwrap();
    let package_targets: BTreeMap<_, _> =
        collect_dependencies(&graph, "driver", &BuildConfig::default())
            .unwrap()
            .into_iter()
            .map(|package| {
                (
                    (package.package_name.clone(), package.version.clone()),
                    package,
                )
            })
            .collect();
    let mut declarations = vec![Declaration::Crate(CrateDeclaration {
        origin: "cargo".to_owned(),
        label: "//external/rust:codec_rlib".to_owned(),
        package: "codec".to_owned(),
        version: "=1.0.0".to_owned(),
        crate_type: LibType::Rlib,
        default_features: true,
        features: Vec::new(),
        rustflags: Vec::new(),
        isolation: Isolation::None,
        git: String::new(),
        rev: String::new(),
        branch: String::new(),
        tag: String::new(),
        patches: Vec::new(),
    })];
    let Declaration::Crate(mut version_two) = declarations[0].clone() else {
        unreachable!()
    };
    version_two.label = "//external/rust:codec_v2".to_owned();
    version_two.version = "=2.0.0".to_owned();
    version_two.crate_type = LibType::Dylib;
    declarations.push(Declaration::Crate(version_two));
    let declarations = DeclarationGroups {
        cargo: declarations,
        gn: Vec::new(),
    };
    let mut crates = BTreeMap::new();
    for context in [ResolutionContext::Target, ResolutionContext::Host] {
        let instance = if context == ResolutionContext::Target {
            "shared"
        } else {
            "shared#host"
        };
        for package in graph
            .packages()
            .filter(|package| package.name() != "driver")
        {
            let targets = &package_targets[&(package.name().to_owned(), package.version().clone())];
            let mut library = targets.lib_target.clone();
            let declared_by = if package.name() == "codec" {
                library.as_mut().unwrap().output_types = declarations
                    .cargo
                    .iter()
                    .filter_map(|item| match item {
                        Declaration::Crate(declaration)
                            if declaration.version == format!("={}", package.version()) =>
                        {
                            Some(declaration.crate_type)
                        }
                        _ => None,
                    })
                    .collect();
                declarations
                    .cargo
                    .iter()
                    .filter_map(|item| match item {
                        Declaration::Crate(declaration)
                            if declaration.version == format!("={}", package.version()) =>
                        {
                            Some(declaration.label.clone())
                        }
                        _ => None,
                    })
                    .collect()
            } else {
                library.as_mut().unwrap().output_types = vec![LibType::Rlib];
                Vec::new()
            };
            let dependencies = package
                .direct_links()
                .map(|link| Dependency {
                    instance: instance.to_owned(),
                    package_name: link.to().name().to_owned(),
                    version: link.to().version().clone(),
                    use_name: link.resolved_name().to_owned(),
                    context,
                    target_condition: None,
                    host_condition: None,
                    condition: None,
                    removal: None,
                })
                .collect();
            let path = format!("//third_party/{}-{}", package.name(), package.version());
            crates.insert(
                format!("{instance}|{}", package.id()),
                CrateInfo {
                    instance: instance.to_owned(),
                    context,
                    rule_dir: path.clone(),
                    source: None,
                    path,
                    package: Package {
                        package_name: package.name().to_owned(),
                        version: package.version().clone(),
                        declared_by,
                        description: None,
                        links: None,
                        authors: Vec::new(),
                        edition: "2021".to_owned(),
                        rustflags: Vec::new(),
                        repository: None,
                        homepage: None,
                        license: None,
                        license_file: None,
                        rust_version: None,
                        readme: None,
                        dependencies,
                        build_dependencies: Vec::new(),
                        target_features: Vec::new(),
                        host_features: Vec::new(),
                        declared_conditions: Vec::new(),
                        lib_target: library,
                        bin_targets: targets.bin_targets.clone(),
                        build_script: targets.build_script.clone(),
                        group: Group::Safe,
                        is_local: true,
                        is_toplevel_dep: package.name() == "codec",
                    },
                },
            );
        }
    }
    let context = SyncContext {
        catalog: declarations.clone(),
        declarations,
        collect_inputs: Vec::new(),
        scopes: Vec::new(),
        crates,
    };
    let paths = BlueOsPaths {
        root: root.to_owned(),
        third_party: "third_party".into(),
        third_party_cargo_root: "third_party".into(),
        third_party_config_file: root.join("build/gnrt_config.toml"),
    };
    generate::generate(&context, &paths).unwrap();
    write(
        root,
        "external/rust/BUILD.gn",
        r#"
import("//build/templates/cargo.gni")
cargo_spec("codec_rlib") { package = "codec" version = "=1.0.0" }
cargo_spec("codec_v2") { package = "codec" version = "=2.0.0" crate_type = "dylib" }
"#,
    );
    write(
        root,
        "BUILD.gn",
        r#"
import("//build/templates/rust.gni")
build_rust("rust_rlib") {
  crate_type = "bin"
  sources = [ "typed.rs" ]
  deps = [ "//external/rust:codec_rlib", "//third_party/bridge-1.0.0:bridge_api_variant_target" ]
}
build_rust("rust_dylib") { crate_type = "bin" sources = [ "version_two.rs" ] deps = [ "//external/rust:codec_v2" ] }
build_rust("host_consumer") { crate_type = "bin" sources = [ "main.rs" ] build_deps = [ "//external/rust:codec_rlib" ] }
group("default") { deps = [ ":rust_rlib", ":rust_dylib", ":host_consumer" ] }
"#,
    );
    write(
        root,
        "main.rs",
        "fn main() { assert_eq!(codec_api::value(), 42); }\n",
    );
    write(
        root,
        "version_two.rs",
        "fn main() { assert_eq!(codec_api::value(), 43); }\n",
    );
    write(root, "typed.rs", "fn main() { assert_eq!(codec_api::value(), 42); bridge_api::verify(codec_api::Token(42)); }\n");
    run(root, "gn", &["gen", "out"]);
    run(root, "ninja", &["-C", "out"]);
    for binary in ["rust_rlib", "rust_dylib", "host_consumer"] {
        run(
            root,
            root.join("out/bin").join(binary).to_str().unwrap(),
            &[],
        );
    }
    for domain in ["target", "host"] {
        let script_dir = root.join(format!(
            "out/obj/third_party/codec-1.0.0/codec_api_variant_{domain}-1.0.0_out"
        ));
        assert_eq!(
            fs::read_to_string(script_dir.join("runs.txt")).unwrap(),
            "x"
        );
    }
    let second = run(root, "ninja", &["-C", "out"]);
    assert!(second.contains("no work to do"), "{second}");
    let runtime = run(root, "gn", &["desc", "out", ":rust_dylib", "runtime_deps"]);
    assert!(runtime.contains("__dylib_v2_0_0"), "{runtime}");
    let script = root.join("third_party/codec-1.0.0/build.rs");
    fs::write(&script, fs::read_to_string(&script).unwrap() + "\n").unwrap();
    run(root, "ninja", &["-C", "out"]);
    for domain in ["target", "host"] {
        assert_eq!(
            fs::read_to_string(root.join(format!(
                "out/obj/third_party/codec-1.0.0/codec_api_variant_{domain}-1.0.0_out/runs.txt"
            )))
            .unwrap(),
            "xx"
        );
    }
    assert_eq!(
        fs::read_to_string(
            root.join(
                "out/obj/third_party/codec-2.0.0/codec_api_variant_target-2.0.0_out/runs.txt"
            )
        )
        .unwrap(),
        "x"
    );
}

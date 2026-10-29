# Adding and Updating Dependencies

## Add a Registry Crate

Work from the BlueKernel root. Inspect the consuming target and existing
declarations first; update an existing declaration rather than creating another
one accidentally.

The following example is the existing `log` declaration in
`external/rust/BUILD.gn`:

```gn
import("//build/templates/cargo.gni")

cargo_spec("log") {
  package = "log"
  version = "=0.4.28"
  default_features = false
}
```

Consumers use the declaration label, not a generated instance name:

```gn
import("//build/templates/build_template.gni")

build_rust("example_library") {
  crate_type = "rlib"
  sources = [ "src/lib.rs" ]
  deps = [ "//external/rust:log" ]
}
```

If a new declaration file is needed, put it under `external/rust` and append it
to `collect.declaration_files` in `external/gnrt_config.toml`. Preserve the
existing registered files. For example, the current catalog is:

```toml
[collect]
declaration_files = ["external/rust/BUILD.gn"]
```

Run `gnrt sync --all`, inspect changes to `external/rust/Cargo.lock`, run
`gnrt sync --all --check`, then generate and build an affected consumer. Check
that version, default features, optional dependencies, and source identity match
the requested change. Generated vendor files remain ignored.

## Select a Library Output

Use `crate_type` to select the library output. For a Rust dynamic library:

```gn
cargo_spec("codec_dynamic") {
  package = "codec"
  version = "=1.0.0"
  crate_type = "dylib"
}
```

Rust targets use `//external/rust:codec_dynamic` in `build_rust` dependencies.
Omit `crate_type` or set it to `"rlib"` when a static Rust library is needed.
The selected target must support dynamic linking when requesting `dylib`.

The GN template defaults to `rlib`, including for packages whose manifests list
other library outputs. Declare procedural macros with `crate_type = "proc-macro"`.
GNRT reports incompatible declarations rather than selecting a different output
type. This field selects a library output, not a binary target.

Declarations share features within the same resolution instance, but must select
the same output type. GNRT rejects requests for both `rlib` and `dylib` from one
instance, including an `rlib` required by a transitive Rust dependency. Selecting
`dylib` does not make transitive dependencies dynamic. Use feature isolation when
independent feature sets are required.

## Procedural Macros and Build Scripts

Declare the procedural macro type explicitly:

```gn
cargo_spec("paste") {
  package = "paste"
  version = "=1.0.15"
  crate_type = "proc-macro"
  default_features = false
}
```

In the consuming target:

```gn
deps = [ "//external/rust:log" ]
proc_macro_deps = [ "//external/rust:paste" ]
```

Procedural macros and build scripts execute on the host. Normal dependencies of
a host-compiled macro also use the host, but remain ordinary `deps` inside its
generated rule. A native product using the host GN toolchain can still have a
Cargo target-context recipe. Preserve those contexts rather than rewriting every
label with an explicit host qualifier.

GNRT generates supported `build.rs` rules and propagates their cfgs, environment,
and native link flags. Build-script dependencies come from the crate manifest.
Do not replace a required build script with `remove_build_rs = true` merely to
avoid an error. Its outputs or native libraries may be required by the consumer.

## Board-Dependent Features

Keep the declaration visible for the full catalog. Use conditional feature
groups rather than wrapping the declaration in a board-specific GN `if`:

```gn
cargo_spec("esp-hal") {
  package = "esp-hal"
  version = "=1.1.1"
  default_features = false
  features = [
    {
      condition = "board == \"esp32c6_devkitc_1\""
      values = [ "esp32c6" ]
    },
    {
      condition = "board == \"seeed_xiao_esp32c3\""
      values = [ "esp32c3" ]
    },
  ]
}
```

Consumers reference `//external/rust:esp-hal`. Cargo resolves the dependency
closure needed by the feature union during sync. Generated GN rules retain the
conditions so an individual board enables its matching chip feature.

`rustflags` supports the same conditional group structure. Its values are
compiler arguments, not Cargo feature names. Keep argument ordering intact.

## Feature Isolation

Cargo features accumulate across compatible requests. A dependency removal does
not subtract a transitive feature. Trace the originating manifest feature before
changing a source patch or declaration.

Choose `isolation` according to the boundary that needs independent features:

- `none`: share the root crate and its dependency closure.
- `root`: compile the root independently while sharing its dependencies.
- `closure`: keep the root and transitive dependency feature domain independent.

Set the field inside the relevant declaration when isolation is required:

```gn
isolation = "closure"
```

Do not add isolation as a default workaround for every conflict. Check which
crate needs separate features, and whether its types cross a boundary between
instances. Existing isolation does not implement the proposed external-provider
or scoped dependency-override configuration.

## Git Crates

GNRT accepts a Git source through `git` and one of `rev`, `branch`, or `tag`.
For a reproducible declaration, pin a full commit. This is an illustrative
company-package example; substitute a real URL, version, and revision:

```gn
cargo_spec("company-codec") {
  package = "company-codec"
  version = "=1.2.3"
  git = "https://git.example.com/rust/company-codec"
  rev = "0123456789abcdef0123456789abcdef01234567"
  default_features = false
}
```

Changing from a Git source to crates.io can change code even when the package
name and version are equal. Preserve source identity unless replacement is
part of the task. Two sources for the same name/version cannot share the current
vendor directory; separate feature instances do not remove this source collision.

Existing libraries outside `external/vendor` keep their manual rules unless
migration is requested. For example, the FTP consumer uses:

```gn
deps = [
  "//external/rust:log",
  "//external/rust-ftp/master:ftp",
]
```

The same boundary applies to `external/rust-fatfs/v0.4` and the root `libc`.
Changing their references to migrated dependencies does not require moving their
sources into GNRT.

## Update a Version or Feature Set

1. Change the declaration and inspect any affected version-specific patch paths.
2. Validate patches against the new upstream source before relying on an existing
   patched vendor directory.
3. Run sync and review the canonical lockfile changes, including transitive
   packages. Do not use the legacy `gnrt update` workflow for this integration.
4. Build the affected consumer and each affected chip variant. A C3-only build
   does not validate C6's feature conditions.
5. Keep declarations, lockfile, configuration, and patches together in the
   requested review scope. Avoid unrelated source or documentation cleanup.

For example, when an `esp-hal` change also updates its patch, review the changed
files from this set:

```text
external/rust/BUILD.gn
external/rust/Cargo.lock
external/gnrt_config.toml
external/rust/patches/esp-hal-1.1.1/001-stable-toolchain-compat.patch
```

Preserve exact requested versions. A declaration such as `version = "=0.23.0"`
must not drift to a newer patch release as incidental maintenance.

# Verylup: the Veryl toolchain installer

[![Actions Status](https://github.com/veryl-lang/verylup/workflows/Regression/badge.svg)](https://github.com/veryl-lang/verylup/actions)
[![Crates.io](https://img.shields.io/crates/v/verylup.svg)](https://crates.io/crates/verylup)

`verylup` installs [Veryl Hardware Description Language](https://veryl-lang.org) from the official release.

## Installation

After installing the following way, executing `verylup setup` is required.

### Download binary

Download from [release page](https://github.com/veryl-lang/verylup/releases/latest), and extract to the directory in `PATH`.

### Cargo

You can install with [cargo](https://crates.io/crates/verylup).

```
cargo install verylup
```

## Usage

```
// Setup verylup (only once at first)
verylup setup

// Update the latest toolchain
verylup update

// Install a specific toolchain
verylup install 0.12.0

// Show installed toolchains
verylup show

// Use a specific toolchain
veryl +0.12.0 build
veryl +latest build
```

## For Veryl Developer

For Veryl developer, a special toolchain target `local` is prepared.
If `veryup install local` is executed in your local Veryl repository, the built toolchain is installed as `local` toolchain.
By default, `local` becomes the default toolchain if it exists.

```
// Build and install the toolchain from local Veryl repository
verylup install local

// Use the built toolchain
veryl build

// Use the latest toolchain
veryl +latest build
```

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.

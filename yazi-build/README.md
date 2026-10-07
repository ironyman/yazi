# myyazi

Installer for [myyazi][source], my fork of the [Yazi][upstream] terminal file manager.

```sh
cargo install --force myyazi
```

This clones the fork at its `shipped` tag, builds it from source, and installs the `yazi` and `ya` binaries into Cargo's bin directory. Git and a Rust toolchain are required.

[source]: https://github.com/ironyman/yazi
[upstream]: https://github.com/sxyazi/yazi

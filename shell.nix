{
  nixpkgs ? <nixpkgs>,
  system ? builtins.currentSystem,
  pkgs ? import nixpkgs { inherit system; },
  pimalaya ? import (fetchTarball "https://github.com/pimalaya/nix/archive/master.tar.gz"),
  fenix ? import (fetchTarball "https://github.com/nix-community/fenix/archive/monthly.tar.gz") { },
}:

let
  inherit (pkgs)
    cargo-deny
    cargo-llvm-cov
    cargo-tarpaulin
    ;

  # NOTE: the standard library of the targets whose backends this host
  # cannot run, for `cargo check --target` and `cargo clippy --target`.
  rust = fenix.combine (
    [
      (fenix.stable.withComponents [
        "cargo"
        "clippy"
        "rust-analyzer"
        "rust-src"
        "rustc"
        "rustfmt"
      ])
    ]
    ++ map (target: fenix.targets.${target}.stable.rust-std) [
      "aarch64-apple-darwin"
      "aarch64-apple-ios"
      "aarch64-linux-android"
      "x86_64-pc-windows-gnu"
      "x86_64-unknown-freebsd"
    ]
  );

  shell = pimalaya.mkShell {
    inherit
      nixpkgs
      system
      pkgs
      fenix
      ;
  };

in
shell.overrideAttrs (prev: {
  buildInputs = [
    rust
  ]
  ++ (prev.buildInputs or [ ])
  ++ [
    cargo-deny
    cargo-llvm-cov
    cargo-tarpaulin
  ];
})

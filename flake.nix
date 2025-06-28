{
  description = "eddy";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Exact Rust release you want:
        rustToolchain = pkgs.rust-bin.stable."1.88.0".default;

        libs = with pkgs; [
          cmake
          pkg-config
          gtk4
          glib
          gdk-pixbuf
          cairo
          pango
          openssl
        ];
      in
      {
        # Build the crate with the pinned toolchain
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "eddy";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock; # delete if you don’t commit Cargo.lock
          cargoSha256 = "0000000000000000000000000000000000000000000000000000";
          nativeBuildInputs = [ rustToolchain ];
        };

        # nix develop —> environment with Rust 1.88
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain # cargo + rustc 1.88.0
            pkgs.cargo-outdated # handy optional extra
          ] ++ libs;
        };
      }
    );
}

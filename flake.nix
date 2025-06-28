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

        rustToolchain = pkgs.rust-bin.stable."1.88.0".default;
        rustSrc = pkgs.rust-bin.stable."1.88.0".rust-src;

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
        # nix develop —> environment with Rust 1.88
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            rustSrc
            pkgs.cargo-outdated
          ] ++ libs;
        };
      }
    );
}

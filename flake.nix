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

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rust-analyzer"
          ];
        };

        libs = with pkgs; [
          cairo
          cmake
          gdk-pixbuf
          glib
          gtk4
          openssl
          pango
          pkg-config
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain

            pkgs.cargo-outdated
          ] ++ libs;
        };
      }
    );
}

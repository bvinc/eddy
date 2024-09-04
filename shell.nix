{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = with pkgs; [
    expat
    openssl
    pkg-config
    gtk4
  ];
}

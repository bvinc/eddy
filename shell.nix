{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  buildInputs = with pkgs; [
    #cargo
    #clippy
    expat
    llvm
    openssl
    pkg-config
    #gcc
    gtk4
    #rustfmt
    #rustup
    #rustc
  ];
}

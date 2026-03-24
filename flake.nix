{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [
          rust-overlay.overlays.default
        ];
      };

      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "rust-analyzer"
          "rust-src"
          "clippy"
        ];
      };

      rustNightlyToolchain = pkgs.rust-bin.selectLatestNightlyWith (
        toolchain:
        toolchain.default.override {
          extensions = [
            "rust-analyzer"
            "rust-src"
            "rust-std"
            "clippy"
          ];
        }
      );

      mkDevShell = (
        rustToolchain:
        pkgs.mkShell {
          packages = with pkgs; [
            llvmPackages_21.libllvm
          ];

          buildInputs = with pkgs; [
            pkg-config
            rustToolchain
            wayland
            libGL
            freetype
            fontconfig
            llvmPackages_21.libllvm
          ];

          nativeBuildInputs = with pkgs; [
            wayland
            libGL
            freetype
            fontconfig
            llvmPackages_21.libllvm
          ];

          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.llvmPackages_21.libllvm.lib}/lib:${pkgs.wayland}/lib:${pkgs.libGL}/lib"
            zsh
          '';
        }
      );
    in
    {
      devShells."${system}" = {
        default = mkDevShell rustToolchain;
        nightly = mkDevShell rustNightlyToolchain;
      };
    };
}

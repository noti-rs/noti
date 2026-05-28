{
  description = "Noti Application";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane = {
      url = "github:ipetkov/crane";
    };

    flake-utils = {
      url = "github:numtide/flake-utils";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      crane,
      flake-utils,
      ...
    }:
    {
      inherit
        (flake-utils.lib.eachSystem [ "x86_64-linux" ] (
          system:
          let
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

            buildInputsWith = (
              additionalPackages:
              with pkgs;
              [
                wayland
                libGL
                freetype
                fontconfig
                llvmPackages_21.libllvm
              ]
              ++ additionalPackages
            );

            nativeBuildInputsWith = (
              additionalPackages:
              with pkgs;
              [
                pkg-config

                wayland
                libGL
                freetype
                fontconfig
                llvmPackages_21.libllvm

                python314
                ninja
                gn
              ]
              ++ additionalPackages
            );

            skiaVersion = "0.87.0";
            skiaAsset = pkgs.fetchurl {
              url = "https://github.com/rust-skia/skia-binaries/releases/download/${skiaVersion}/skia-binaries-e551f334ad5cbdf43abf-x86_64-unknown-linux-gnu-egl-gl-pdf-svg-textlayout-vulkan-wayland-webpd-webpe-x11.tar.gz";
              hash = "sha256-WT7emMyv1IGT+NASwxAlzCyo2Ak/PTyry9lHal6dRtU=";
            };

            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
              pkgs.llvmPackages_21.libllvm.lib
              pkgs.wayland
              pkgs.libGL
            ];

            mkDevShell = (
              rustToolchain:
              pkgs.mkShell {
                packages = with pkgs; [
                  llvmPackages_21.libllvm
                ];

                buildInputsWith = buildInputsWith [ rustToolchain ];
                nativeBuildInputs = nativeBuildInputsWith [ rustToolchain ];

                shellHook = ''
                  export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}"
                  export SKIA_GN_COMMAND="${pkgs.gn}/bin/gn"
                  export SKIA_BINARIES_URL="file://${skiaAsset}"
                  export SKIA_NINJA_COMMAND="${pkgs.ninja}/bin/ninja"
                  export RUSTFLAGS="-L ${pkgs.libGL}/lib -lGL";
                  zsh
                '';
              }
            );

            craneLib = toolchain: (crane.mkLib pkgs).overrideToolchain toolchain;

            unfilteredRoot = ./.;

            appSrc =
              toolchain:
              pkgs.lib.fileset.toSource {
                root = unfilteredRoot;
                fileset = pkgs.lib.fileset.unions [
                  ((craneLib toolchain).fileset.commonCargoSources unfilteredRoot)
                  ./crates/filetype/src/layout.pest
                ];
              };

            appCommonArgs = toolchain: {
              src = appSrc toolchain;
              strictDeps = true;

              buildInputs = buildInputsWith [
                toolchain

              ];
              nativeBuildInputs = nativeBuildInputsWith [
                toolchain
                pkgs.makeWrapper
              ];

              SKIA_BINARIES_URL = "file://${skiaAsset}";
              SKIA_GN_COMMAND = "${pkgs.gn}/bin/gn";
              SKIA_NINJA_COMMAND = "${pkgs.ninja}/bin/ninja";
              LD_LIBRARY_PATH = LD_LIBRARY_PATH;
              RUSTFLAGS = "-L ${pkgs.libGL}/lib -lGL";
            };
            appCargoArtifacts = toolchain: (craneLib toolchain).buildDepsOnly (appCommonArgs toolchain);

            buildNotiApplication = (
              toolchain:
              let
                cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
                selectedSkiaVersion = cargoToml.workspace.dependencies.skia-safe.version or "unknown";
              in
              assert pkgs.lib.assertMsg (
                skiaVersion == selectedSkiaVersion
              ) "SKIA VERSION MISMATCH: URL does not contain version ${skiaVersion}!";
              (craneLib toolchain).buildPackage (
                (appCommonArgs toolchain)
                // {
                  cargoArtifacts = appCargoArtifacts toolchain;

                  postInstall = ''
                    wrapProgram $out/bin/noti --prefix LD_LIBRARY_PATH : "${LD_LIBRARY_PATH}"
                  '';
                }
              )
            );
          in
          {
            devShells = {
              default = mkDevShell rustToolchain;
              nightly = mkDevShell rustNightlyToolchain;
            };

            packages = {
              default = buildNotiApplication rustToolchain;
              nightly = buildNotiApplication rustNightlyToolchain;
            };

            checks = {
              build = self.packages."${system}".default;

              noti-test = (craneLib rustToolchain).cargoNextest (
                (appCommonArgs rustToolchain)
                // {
                  cargoArtifacts = appCargoArtifacts rustToolchain;
                  cargoNextestExtraArgs = "--workspace";
                }
              );

              noti-clippy = (craneLib rustToolchain).cargoClippy (
                (appCommonArgs rustToolchain)
                // {
                  cargoArtifacts = appCargoArtifacts rustToolchain;
                  cargoClippyExtraArgs = "--all-targets -- --deny warnings";
                }
              );

              noti-fmt = (craneLib rustToolchain).cargoFmt {
                src = appSrc rustToolchain;
              };
            };
          }
        ))
        devShells
        packages
        checks
        ;

      homeModules.default =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          noti-rs = self.packages."${pkgs.stdenv.system}".default;
        in
        {
          options.programs.noti-rs = {
            enable = lib.mkEnableOption "Noti Application";

            service = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Enable noti systemd service";
            };
          };

          config = lib.mkIf config.programs.noti-rs.enable {
            home.packages = [
              noti-rs
            ];

            systemd.user.services.noti = lib.mkIf config.programs.noti-rs.service {
              Unit = {
                Description = "Noti — Wayland notification daemon";
                PartOf = [ "graphical-session.target" ];
                After = [ "graphical-session.target" ];
              };
              Install = {
                WantedBy = [ "graphical-session.target" ];
              };
              Service = {
                Type = "dbus";
                BusName = "org.freedesktop.Notifications";
                Environment = "NOTI_LOG=info";
                ExecCondition = "${pkgs.bash}/bin/sh -c '[ -n $WAYLAND_DISPLAY ]'";
                ExecStart = "${noti-rs}/bin/noti run";
                Restart = "on-failure";
              };
            };
          };
        };
    };
}

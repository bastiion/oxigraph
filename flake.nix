{
  description = "Flake for oxigraph triple store";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-22.11";
    naersk.url = "github:nmattia/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, flake-utils, naersk, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        # pkgs = import nixpkgs { inherit system; overlays = [ (import rust) ]; };
        pkgs = nixpkgs.legacyPackages.${system};

        rustBuild = fenix.packages.${system}.stable.withComponents [
          "cargo"
          "rustc"
          # "rust-src"  # just for rust-analyzer
          # "clippy"
        ];

        # Override the version used in naersk
        naersk-lib = naersk.lib."${system}".override {
          cargo = rustBuild;
          rustc = rustBuild;
        };

        pythonEnv = pkgs.python3.withPackages (ps: with ps; [
          pip
          pytest
        ]);

        patchPhase = ''
          rm -rf rocksdb lz4
          ln -s ${pkgs.rocksdb.src} oxrocksdb-sys/rocksdb
          ln -s ${pkgs.lz4.src} oxrocksdb-sys/lz4
        '';

        version = "0.3.10";

      in
        {
          #run: rustup update
          #run: pip install -r python/requirements.dev.txt
          #run: maturin build -m python/Cargo.toml
          #run: pip install  --no-index --find-links=target/wheels/ pyoxigraph
          #run: rm -r target/wheels
          #run: python generate_stubs.py pyoxigraph pyoxigraph.pyi --black
          #working-directory: ./python
          #run: maturin sdist -m python/Cargo.toml
          packages.default = with pkgs; python3Packages.buildPythonPackage {
            pname = "pyoxigraph";
            inherit version;
            src = ./python;
            format = "pyproject";
            requirementsFile = ./python/requirements.dev.txt;
            cargoSha256 = lib.fakeSha256;
            nativeBuildInputs = [ rustBuild rustPlatform.maturinBuildHook ];
            buildInputs = [ pythonEnv libclang ];
            cargoDeps = rustPlatform.importCargoLock {
              lockFile = ./python/Cargo.lock;
            };

          };
          packages.oxigraph = naersk-lib.buildPackage {
            pname = "oxigraph";
            inherit version;
            src = ./.;
            inherit patchPhase;
            nativeBuildInputs = with pkgs; [
              rustPlatform.bindgenHook
            ];
          };
          packages.oxigraph-server = naersk-lib.buildPackage {
            pname = "oxigraph-server";
            inherit version;
            src = ./.;
            inherit patchPhase;
            targets = [ "server" ];
            nativeBuildInputs = with pkgs; [
              rustPlatform.bindgenHook
            ];
          };
          devShell = with pkgs; mkShell {
            buildInputs = [
              pythonEnv
              clang
              libclang
              maturin
              rustBuild
            ];

            LIBCLANG_PATH = "${libclang.lib}/lib";
            shellHook = ''
              #rustup update
              python3 -m venv venv
              source venv/bin/activate
              cd python
              pip install -r ./requirements.dev.txt
              maturin develop --release -m Cargo.toml
              python generate_stubs.py pyoxigraph pyoxigraph.pyi --black
              maturin build --release -m Cargo.toml
            '';
          };
        }
    );
}

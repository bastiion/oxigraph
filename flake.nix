{
  description = "Flake for oxigraph triple store";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-22.05";
    rust.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };
  outputs = { self, nixpkgs, flake-utils, rust }:
    flake-utils.lib.eachDefaultSystem (system:
    let pkgs = import nixpkgs { inherit system; overlays = [ (import rust) ]; };
     pythonEnv = pkgs.python3.withPackages (ps: with ps; [ 
       pip
       pytest
     ]);
     version = "0.3.10";
     rustBuild = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default);
     cargo-toml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

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
          buildInputs = [ pythonEnv llvmPackages.libclang ];
          cargoDeps = rustPlatform.importCargoLock {
            lockFile = ./python/Cargo.lock;
          };

        };
        packages.oxigraph = with pkgs; rustPlatform.buildRustPackage {
          pname = "oxigraph";
          inherit version;
          src = ./lib;
          cargoSha256 = lib.fakeSha256;
          nativeBuildInputs = [ rustBuild ];
          buildInputs = [ llvmPackages.libclang ];
          cargoBuildType = "release";
          cargoDeps = rustPlatform.importCargoLock {
            lockFile = ./lib/Cargo.lock;
          };
        };
        packages.oxigraph-server = with pkgs; rustPlatform.buildRustPackage {
          pname = "oxigraph-server";
          inherit version;
          src = ./server;
          cargoSha256 = lib.fakeSha256;
          cargoBuildFlags = [ "--release" ];
          cargoBuildType = "release";
          nativeBuildInputs = [ rustBuild ];
          buildInputs = [ llvmPackages.libclang ];
          cargoDeps = rustPlatform.importCargoLock {
            lockFile = ./Cargo.lock;
          };
        };
        devShell = with pkgs; mkShell {
          buildInputs = [  
            pythonEnv
            llvmPackages.clang
            llvmPackages.libclang
            maturin
            rustBuild
          ];
          
      
          LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib";
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

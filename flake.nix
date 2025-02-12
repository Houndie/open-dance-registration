{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-23.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }: 
  let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ rust-overlay.overlays.default ];
    };
    myrust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

    grpcuiScript = (pkgs.writeShellScriptBin ",grpcui" "${pkgs.grpcui}/bin/grpcui -plaintext localhost:50051");

    dioxus-cli = pkgs.rustPlatform.buildRustPackage {
      pname = "dioxus-cli";
      version = "0.6.1-git";

      /*src = pkgs.fetchCrate {
        inherit pname version;
	sha256 = "sha256-0Kg2/+S8EuMYZQaK4Ao+mbS7K48VhVWjPL+LnoVJMSw="; # 0.6.0
      };*/
      src = pkgs.fetchFromGitHub {
        owner = "dioxusLabs";
        repo = "dioxus";
        rev = "857c3e232ecd024c752176bd0af14a5654014527";
        hash = "sha256-N4wWWfa7T0ldMVHSavCgvqoDCk2vGUk/Jf3O9Q1TyZU=";
      };

      buildAndTestSubdir = "packages/cli";

      #cargoHash = "sha256-RMo6q/GSAV1bCMWtR+wu9xGKCgz/Ie6t/8oirBly/LQ="; # 0.6.0
      cargoHash = "sha256-kE595bHDPmy9HOPZXqcYR4lxe7r6d+UA3yikKB/37Y8="; # git

      checkFlags = [ "--skip=wasm_bindgen::test::test_cargo_install" "--skip=wasm_bindgen::test::test_github_install" ];

      OPENSSL_NO_VENDOR = 1;

      nativeBuildInputs = [ pkgs.pkg-config pkgs.cacert myrust ];
      buildInputs = [ pkgs.openssl ];
    };
  in
  {
    devShell.${system} = pkgs.mkShell {
      packages = [
        myrust 
        pkgs.protobuf_23 
        pkgs.sqlx-cli 
        pkgs.sqlitebrowser
        dioxus-cli
        grpcuiScript
        pkgs.entr
	pkgs.cargo-expand

        (pkgs.writeShellScriptBin ",devserver" ''
	  set -e

	  ROOT=$(${pkgs.git}/bin/git rev-parse --show-toplevel)

	  # Wart:  rustup will install it's own binaries but they match our versions and we can ignore them:-)
	  PATH=${pkgs.rustup}/bin:$PATH

	  rustup show

	  cd $ROOT; cargo run --bin odr-cli --features server -- init; RUST_LOG=tower_http=trace ${dioxus-cli}/bin/dx serve
	'')
      ];
      PROTOC = "${pkgs.protobuf_23}/bin/protoc";
      PROTOC_INCLUDE = "${pkgs.protobuf_23}/include";
      RUST_SRC_PATH = "${myrust}/lib/rustlib/src/rust/library";
    };
  };
}

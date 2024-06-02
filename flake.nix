{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-23.11";
    unstable.url = "nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, unstable, rust-overlay }: 
  let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ rust-overlay.overlays.default ];
    };
    myrust = pkgs.rust-bin.stable.latest.default.override {
      targets = [ "wasm32-unknown-unknown" ];
    };

    grpcuiScript = (pkgs.writeShellScriptBin ",grpcui" "${pkgs.grpcui}/bin/grpcui -plaintext localhost:50051");

    dioxus-cli = pkgs.rustPlatform.buildRustPackage rec {
      pname = "dioxus-cli";
      version = "0.5.1";

      src = pkgs.fetchCrate {
        inherit pname version;
        #sha256 = "sha256-iNlJLDxb8v7x19q0iaAnGmtmoPjMW8YXzbx5Fcf8Yws="; # 0.5.0
        sha256 = "sha256-EQGidjyqB48H33vFvBLUpHYGUm1RHMQM+eiU2tmCSwc="; # 0.5.1
      };

      #cargoHash = "sha256-6XKNBLDNWYd5+O7buHupXzVss2jCdh3wu9mXVLivH44="; # 0.5.0
      cargoHash = "sha256-IOwD9I70hqY3HwRMhqxtRmDP/yO4OdNkNRAIIIAqbmY="; # 0.5.1

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

        (pkgs.writeShellScriptBin ",devserver" ''
	  set -e

	  ROOT=$(${pkgs.git}/bin/git rev-parse --show-toplevel)

	  ${pkgs.tmux}/bin/tmux \
	    new-session "cd $ROOT/odr-admin; ${unstable.legacyPackages.${system}.dioxus-cli}/bin/dx serve; read" \; \
	    split-window "cd $ROOT/odr-cmd; cargo run init; cd $ROOT/odr-server; RUST_LOG=tower_http=trace find src/ | ${pkgs.entr}/bin/entr -r ${myrust}/bin/cargo run; read" \; \
	    select-layout even-vertical
	'')
      ];
      PROTOC = "${pkgs.protobuf_23}/bin/protoc";
      PROTOC_INCLUDE = "${pkgs.protobuf_23}/include";
    };
  };
}

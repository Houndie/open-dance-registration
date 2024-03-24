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
  in
  {
    devShell.${system} = pkgs.mkShell {
      packages = with pkgs; [
        myrust 
        protobuf_23 
        sqlx-cli 
	sqlitebrowser
	unstable.legacyPackages.${system}.dioxus-cli
	grpcuiScript
	entr

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

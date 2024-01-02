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
  in
  {
    devShell.${system} = pkgs.mkShell {
      nativeBuildInputs = with pkgs; [rust-bin.stable.latest.default protobuf_23];
    };
  };
}

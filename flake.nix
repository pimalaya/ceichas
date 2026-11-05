{
  description = "Wait until a wall-clock time, counting suspended time and following clock changes";

  inputs = {
    nixpkgs = {
      url = "github:nixos/nixpkgs/nixos-26.05";
    };
    fenix = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    pimalaya = {
      url = "github:pimalaya/nix";
      flake = false;
    };
  };

  outputs =
    inputs:
    let
      outputs = (import inputs.pimalaya).mkFlakeOutputs inputs {
        shell = ./shell.nix;
        default = ./default.nix;
      };

      pkgs = inputs.nixpkgs.legacyPackages.x86_64-linux;

      src = pkgs.lib.fileset.toSource {
        root = ./.;
        fileset = pkgs.lib.fileset.unions [
          ./Cargo.lock
          ./Cargo.toml
          ./examples
          ./src
        ];
      };
    in
    outputs
    // {
      checks.x86_64-linux.vm = import ./nix/vm-test.nix { inherit pkgs src; };
    };
}

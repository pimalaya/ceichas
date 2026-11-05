# TODO: move this to nixpkgs
# This file aims to be a replacement for the nixpkgs derivation.

{
  buildFeatures ? [ ],
  buildNoDefaultFeatures ? false,
  buildPackages,
  fetchFromGitHub,
  lib,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
  inherit buildNoDefaultFeatures buildFeatures;

  pname = "ceichas";
  version = "0.1.0";
  cargoHash = "";

  src = fetchFromGitHub {
    owner = "pimalaya";
    repo = "ceichas";
    rev = "v0.1.0";
    hash = "";
  };

  meta = {
    description = "Wait until a wall-clock time, counting suspended time and following clock changes";
    homepage = "https://github.com/pimalaya/ceichas";
    changelog = "https://github.com/pimalaya/ceichas/blob/master/CHANGELOG.md";
    license = [
      lib.licenses.mit
      lib.licenses.asl20
    ];
    maintainers = with lib.maintainers; [ soywod ];
  };
}

{ ... }:

{
  imports = [
    ./nix/common.nix
    ./nix/linux.nix
    ./nix/darwin.nix
    ./nix/ci.nix
  ];
}

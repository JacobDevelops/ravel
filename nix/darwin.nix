{ lib, pkgs, ... }:

{
  config = lib.mkIf pkgs.stdenv.isDarwin {
    env.RAVEL_PLATFORM = "aarch64-darwin";
  };
}

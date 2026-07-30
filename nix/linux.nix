{ lib, pkgs, ... }:

{
  config = lib.mkIf pkgs.stdenv.isLinux {
    env.RAVEL_PLATFORM = "x86_64-linux";

    packages = with pkgs; [
      perf
      procps
      sysstat
      vulkan-tools
    ];
  };
}

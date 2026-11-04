{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:

{
  # https://devenv.sh/basics/
  env.GREET = "devenv";

  # https://devenv.sh/packages/
  packages = [
    pkgs.git
    pkgs.bun
    pkgs.cmake
    pkgs.perl
    pkgs.pkg-config
    pkgs.stdenv.cc
    pkgs.gnumake
    pkgs.openssl
    # Tauri system deps for the full Rust workspace build (outl-server links
    # outl-tauri-shared → tauri). These need a full nixpkgs; the minimal
    # devenv variant lacks them. Add when your nixpkgs has them:
    # pkgs.libwebkit2gtk_4_1, pkgs.librsvg2, pkgs.libxdo, pkgs.ayatana-appindicator3
  ];

  # https://devenv.sh/languages/
  languages.rust.enable = true;

  # https://devenv.sh/processes/
  # processes.dev.exec = "${lib.getExe pkgs.watchexec} -n -- ls -la";

  # https://devenv.sh/services/
  # services.postgres.enable = true;

  # https://devenv.sh/scripts/
  scripts.hello.exec = ''
    echo hello from $GREET
  '';

  scripts.test-rust.exec = ''
    cd vendor/outl && cargo test --workspace --exclude outl-desktop --exclude outl-mobile
  '';

  scripts.test-frontend.exec = ''
    cd frontend && bun install --frozen-lockfile && bun run typecheck && bun run build
  '';

  scripts.test.exec = ''
    devenv run test-rust
    devenv run test-frontend
  '';

  scripts.run-server.exec = ''
    cd vendor/outl && cargo run -p outl-server
  '';

  # https://devenv.sh/basics/
  enterShell = ''
    hello         # Run scripts directly
    git --version # Use packages
  '';

  # https://devenv.sh/tests/
  enterTest = ''
    echo "Running tests"
    git --version | grep --color=auto "${pkgs.git.version}"
  '';

  # https://devenv.sh/git-hooks/
  # git-hooks.hooks.shellcheck.enable = true;

  # See full reference at https://devenv.sh/reference/options/
}

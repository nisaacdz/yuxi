# To learn more about how to use Nix to configure your environment
# see: https://developers.google.com/idx/guides/customize-idx-env
{ pkgs, ... }: {
  channel = "unstable";

  packages = [
    pkgs.rustc
    pkgs.cargo
    pkgs.rust-analyzer
    pkgs.cargo-watch
    pkgs.azure-cli
  ];

  env = {
    RUST_BACKTRACE = "1";
  };

  services = {
    docker = {
      enable = true;
    };
  };

  idx = {
    extensions = [
      "rust-lang.rust-analyzer"
      "vadimcn.vscode-lldb"
    ];

    workspace = {
      onCreate = {
        fetch-deps = "cargo fetch";
      };

      onStart = {
        check = "cargo check";
      };
    };

    previews = {
      enable = true;
      previews = {
        app = {
          command = ["cargo" "run"];
          manager = "web";
        };
      };
    };
  };
}

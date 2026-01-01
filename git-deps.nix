# Git dependency hashes
# Update with: nix run .#update-git-deps
{
  "whisper-rs" = "sha256-NGbi1qKRC+A70k+Y5DYJOP75dgpcbTw7FqdCgMPmCjk=";
  
  # whisper.cpp submodule commit that whisper-rs uses
  # This is needed because cargo vendoring doesn't handle git submodules
  "whisper-cpp-submodule" = "fc45bb86251f774ef817e89878bb4c2636c8a58f";
  "whisper-cpp-submodule-hash" = "sha256-BEpdr8sSvB+84H4m7Ekov+mjzwo/Vn5QMevya0ugNjA=";
}

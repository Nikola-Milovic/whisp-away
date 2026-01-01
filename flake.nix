{
  description = "WhispAway flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, crane, ... }@inputs:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    craneLib = crane.mkLib pkgs;
  in
  {
    packages.${system} = rec {
      # Standard nixpkgs-compatible build (for potential upstream contribution)
      whisp-away-package = pkgs.callPackage ./build.nix {
        inherit (pkgs) rustPlatform;
        useCrane = false;
        accelerationType = "vulkan";
      };
      
      # Crane-based build with better caching for development
      whisp-away = pkgs.callPackage ./build.nix {
        inherit craneLib;
        useCrane = true;
        accelerationType = "vulkan";
      };
      
      # Variants with different acceleration (using crane for development)
      whisp-away-cpu = pkgs.callPackage ./build.nix {
        inherit craneLib;
        useCrane = true;
        accelerationType = "cpu";
      };
      
      whisp-away-cuda = pkgs.callPackage ./build.nix {
        inherit craneLib;
        useCrane = true;
        accelerationType = "cuda";
      };
      
      whisp-away-openvino = pkgs.callPackage ./build.nix {
        inherit craneLib;
        useCrane = true;
        accelerationType = "openvino";
      };
      
      default = whisp-away;
    };
    
    nixosModules = {
      # Basic modules (will use rustPlatform)
      home-manager = ./packaging/nixos/home-manager.nix;
      nixos = ./packaging/nixos/nixos.nix;

      # Pre-configured modules with crane support
      # These can be used directly: imports = [ whisp-away.nixosModules.home-manager-with-crane ];
      home-manager-with-crane = { config, lib, pkgs, ... }: {
        imports = [ ./packaging/nixos/home-manager.nix ];
        _module.args.craneLib = craneLib;
      };

      nixos-with-crane = { config, lib, pkgs, ... }: {
        imports = [ ./packaging/nixos/nixos.nix ];
        _module.args.craneLib = craneLib;
      };
    };

    apps.${system} = {
      update-git-deps = {
        type = "app";
        program = "${pkgs.writeShellScript "update-git-deps" ''
          set -euo pipefail

          echo "Updating git dependency hashes from Cargo.lock..."

          # Parse Cargo.lock for whisper-rs
          REV=$(${pkgs.gnugrep}/bin/grep -A2 'name = "whisper-rs"' Cargo.lock | \
                ${pkgs.gnugrep}/bin/grep -oP '#\K[a-f0-9]+' | head -1)

          if [ -z "$REV" ]; then
            echo "Error: Could not find whisper-rs in Cargo.lock"
            exit 1
          fi

          echo "Found whisper-rs rev: $REV"
          echo "Fetching whisper-rs hash..."

          HASH=$(${pkgs.nix-prefetch-git}/bin/nix-prefetch-git \
            https://codeberg.org/madjinn/whisper-rs.git \
            --rev "$REV" --quiet 2>/dev/null | ${pkgs.jq}/bin/jq -r .hash)

          echo "whisper-rs hash: $HASH"

          # Get the whisper.cpp submodule commit from the whisper-rs repo
          echo "Fetching whisper.cpp submodule reference..."
          TEMP_DIR=$(mktemp -d)
          trap "rm -rf $TEMP_DIR" EXIT
          
          ${pkgs.git}/bin/git clone --depth 1 --no-checkout \
            https://codeberg.org/madjinn/whisper-rs.git "$TEMP_DIR/whisper-rs" 2>/dev/null
          cd "$TEMP_DIR/whisper-rs"
          ${pkgs.git}/bin/git fetch origin "$REV" --depth 1 2>/dev/null
          ${pkgs.git}/bin/git checkout "$REV" 2>/dev/null
          
          # Get submodule commit from .gitmodules and git ls-tree
          WHISPER_CPP_REV=$(${pkgs.git}/bin/git ls-tree HEAD sys/whisper.cpp 2>/dev/null | ${pkgs.gawk}/bin/awk '{print $3}')
          
          if [ -z "$WHISPER_CPP_REV" ]; then
            echo "Warning: Could not find whisper.cpp submodule reference, using fallback"
            WHISPER_CPP_REV="fc45bb86251f774ef817e89878bb4c2636c8a58f"
          fi
          
          echo "Found whisper.cpp submodule rev: $WHISPER_CPP_REV"
          echo "Fetching whisper.cpp hash..."
          
          cd /
          WHISPER_CPP_HASH=$(${pkgs.nix-prefetch-git}/bin/nix-prefetch-git \
            https://github.com/ggerganov/whisper.cpp.git \
            --rev "$WHISPER_CPP_REV" --fetch-submodules --quiet 2>/dev/null | ${pkgs.jq}/bin/jq -r .hash)
          
          echo "whisper.cpp hash: $WHISPER_CPP_HASH"

          # Update git-deps.nix
          cat > git-deps.nix <<EOF
# Git dependency hashes
# Update with: nix run .#update-git-deps
{
  "whisper-rs" = "$HASH";
  
  # whisper.cpp submodule commit that whisper-rs uses
  # This is needed because cargo vendoring doesn't handle git submodules
  "whisper-cpp-submodule" = "$WHISPER_CPP_REV";
  "whisper-cpp-submodule-hash" = "$WHISPER_CPP_HASH";
}
EOF

          echo "✓ Updated git-deps.nix"
        ''}";
      };

      default = self.apps.${system}.update-git-deps;
    };
  };
}

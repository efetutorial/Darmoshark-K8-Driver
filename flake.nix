{
  description = "Darmoshark K8 Studio — native keyboard control application";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          runtimeLibs = with pkgs; [
            fontconfig
            freetype
            libxkbcommon
            vulkan-loader
            wayland
            libx11
            libxcb
            libxcursor
            libxi
            libxrandr
          ];
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "darmoshark-k8-studio";
            version = "2.0.0";
            src = nixpkgs.lib.cleanSourceWith {
              src = ./.;
              filter = path: type:
                let
                  relative = nixpkgs.lib.removePrefix (toString ./. + "/") (toString path);
                in
                  !(nixpkgs.lib.hasPrefix "target" relative
                    || nixpkgs.lib.hasPrefix "dist" relative
                    || relative == "result");
            };
            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = with pkgs; [
              pkg-config
              makeWrapper
            ];
            buildInputs = runtimeLibs;

            postInstall = ''
              install -Dm644 ${./packaging/darmoshark-k8.desktop} \
                $out/share/applications/darmoshark-k8.desktop
              install -Dm644 ${./packaging/darmoshark-k8.svg} \
                $out/share/icons/hicolor/scalable/apps/darmoshark-k8.svg
              install -Dm644 ${./packaging/99-darmoshark-k8.rules} \
                $out/lib/udev/rules.d/99-darmoshark-k8.rules
            '';

            # GPUI loads Vulkan, Wayland and X11 libraries at runtime.
            postFixup = ''
              patchelf --add-rpath ${pkgs.lib.makeLibraryPath runtimeLibs} \
                $out/bin/darmoshark-k8-studio
              wrapProgram $out/bin/darmoshark-k8-studio \
                --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.grim pkgs.pipewire ]}
            '';

            meta = {
              description = "RGB, LCD, macro and key control for the Darmoshark K8";
              homepage = "https://github.com/efetutorial/Darmoshark-K8-Driver";
              license = pkgs.lib.licenses.mit;
              mainProgram = "darmoshark-k8-studio";
              platforms = pkgs.lib.platforms.linux;
            };
          };
        });

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/darmoshark-k8-studio";
          meta.description = "Darmoshark K8 Studio";
        };
      });

      checks = forAllSystems (system: {
        package = self.packages.${system}.default;
      });

      nixosModules.default = { pkgs, ... }: {
        environment.systemPackages = [ self.packages.${pkgs.system}.default ];
        services.udev.packages = [ self.packages.${pkgs.system}.default ];
      };

      devShells = forAllSystems (system:
        let pkgs = nixpkgs.legacyPackages.${system};
            libs = with pkgs; [
              fontconfig freetype libxkbcommon vulkan-loader wayland
              libx11 libxcb libxcursor libxi libxrandr
            ];
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [ cargo rustc rustfmt pkg-config grim pipewire ];
            buildInputs = libs;
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libs;
          };
        });
    };
}

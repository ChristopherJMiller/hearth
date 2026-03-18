# tests/kanidm-auth.nix — NixOS VM test: Kanidm PAM/NSS authentication
#
# Two-node test verifying that Hearth devices can authenticate users against
# a Kanidm identity provider via PAM and resolve users/groups via NSS.
#
# Nodes:
#   - kanidm: Kanidm server with TLS and bootstrapped test data
#   - client: Hearth device with kanidm-unixd for PAM/NSS
#
# Verifications:
#   - Kanidm server starts and is healthy
#   - Bootstrap creates test users and groups
#   - kanidm-unixd resolves users via NSS (getent passwd)
#   - kanidm-unixd resolves groups via NSS (getent group)
#   - PAM authentication works for Kanidm users

{ pkgs, lib, ... }:

let
  kanidmTest = import ./lib/kanidm-test.nix { inherit pkgs; };
in
pkgs.testers.nixosTest {
  name = "hearth-kanidm-auth";

  nodes = {
    kanidm = { config, pkgs, ... }: {
      imports = [ (kanidmTest.module {}) ];
    };

    client = { config, pkgs, ... }: {
      imports = [
        ../modules/pam.nix
        ../modules/kanidm-client.nix
      ];

      services.kanidm.package = pkgs.kanidm_1_7;

      services.hearth.pam = {
        enable = true;
        authBackend = "kanidm";
      };

      services.hearth.kanidmClient = {
        enable = true;
        uri = "https://kanidm:8443";
        caCertPath = kanidmTest.caCertPath;
        allowedLoginGroups = [ "hearth-users" ];
        hsmType = "soft";
      };

      security.pam.services.su.makeHomeDir = true;

      virtualisation.memorySize = 1024;
    };
  };

  testScript = ''
    kanidm.start()
    kanidm.wait_for_unit("kanidm.service")
    kanidm.wait_for_open_port(8443)
    kanidm.wait_for_unit("kanidm-bootstrap.service", timeout=120)
    kanidm.succeed("test -f /tmp/bootstrap-done")

    testuser_password = kanidm.succeed("cat /tmp/testuser-password").strip()

    client.start()
    client.wait_for_unit("multi-user.target")
    client.wait_for_unit("kanidm-unixd.service")

    # Verify user resolution via NSS
    client.wait_until_succeeds(
        "getent passwd testuser@kanidm",
        timeout=120,
    )

    # Verify group resolution via NSS
    client.wait_until_succeeds(
        "getent group hearth-users@kanidm",
        timeout=30,
    )

    # Verify PAM authentication
    client.succeed(
        f"echo '{testuser_password}' | su - 'testuser@kanidm' -c 'whoami'"
    )
  '';
}

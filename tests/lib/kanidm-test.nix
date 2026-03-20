# tests/lib/kanidm-test.nix — Shared Kanidm test infrastructure.
#
# Provides self-signed TLS certs, a bootstrap script, and a reusable
# NixOS module for Kanidm server nodes in VM integration tests.
#
# Usage in a NixOS VM test:
#
#   let
#     kanidmTest = import ./lib/kanidm-test.nix { inherit pkgs; };
#   in
#   pkgs.testers.nixosTest {
#     nodes.kanidm = {
#       imports = [ (kanidmTest.module {}) ];
#     };
#     nodes.client = {
#       services.kanidm.package = pkgs.kanidm_1_7;
#       services.hearth.kanidmClient = {
#         enable = true;
#         uri = "https://kanidm:8443";
#         caCertPath = kanidmTest.caCertPath;
#         ...
#       };
#     };
#     ...
#   };

{ pkgs }:

let
  # TLS certificates: proper CA + server cert chain.
  # Newer rustls versions reject self-signed certs used directly as server
  # certs (CaUsedAsEndEntity). We generate a CA, then issue a server cert
  # signed by it, so the chain validates correctly.
  certs = pkgs.runCommand "kanidm-test-certs" {
    nativeBuildInputs = [ pkgs.openssl ];
  } ''
    mkdir -p $out

    # 1. Generate CA key + cert
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
      -keyout $out/ca-key.pem -out $out/ca.pem \
      -days 365 -nodes \
      -subj "/CN=Hearth Test CA" \
      -addext "basicConstraints=critical,CA:TRUE" \
      -addext "keyUsage=critical,keyCertSign,cRLSign"

    # 2. Generate server key + CSR
    openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
      -keyout $out/key.pem -out $out/server.csr \
      -nodes \
      -subj "/CN=kanidm"

    # 3. Sign server cert with CA
    openssl x509 -req -in $out/server.csr \
      -CA $out/ca.pem -CAkey $out/ca-key.pem -CAcreateserial \
      -out $out/server.pem \
      -days 365 \
      -extfile <(printf "subjectAltName=DNS:kanidm,IP:127.0.0.1\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth")

    # 4. Build chain file (server cert + CA cert) for Kanidm's tls_chain
    cat $out/server.pem $out/ca.pem > $out/cert.pem

    chmod 644 $out/key.pem $out/ca-key.pem
    rm -f $out/server.csr $out/ca.srl
  '';

  # Bootstrap script: provisions Kanidm with test users and groups via REST API.
  # Creates: hearth-users group (POSIX), testuser person (POSIX, in hearth-users).
  # Writes the testuser password to /tmp/testuser-password and touches /tmp/bootstrap-done.
  bootstrapScript = pkgs.writeShellScript "kanidm-bootstrap" ''
    set -euo pipefail

    KANIDM_URL="https://localhost:8443"
    KANIDMD="${pkgs.kanidm_1_7}/bin/kanidmd"
    C="curl -sk"

    echo "[bootstrap] Waiting for Kanidm..."
    for i in $(seq 1 60); do
      if $C "$KANIDM_URL/status" 2>/dev/null | grep -q '"true"'; then
        break
      fi
      sleep 1
    done

    # --- Recover admin accounts ---
    RECOVER_OUTPUT=$($KANIDMD recover-account -c /etc/kanidm/server.toml admin 2>&1 || true)
    ADMIN_PASS=$(echo "$RECOVER_OUTPUT" | grep -oP 'new_password:\s*"\K[^"]+' || true)
    [ -z "$ADMIN_PASS" ] && { echo "FATAL: admin recovery failed"; exit 1; }

    IDM_RECOVER=$($KANIDMD recover-account -c /etc/kanidm/server.toml idm_admin 2>&1 || true)
    IDM_ADMIN_PASS=$(echo "$IDM_RECOVER" | grep -oP 'new_password:\s*"\K[^"]+' || true)
    [ -z "$IDM_ADMIN_PASS" ] && { echo "FATAL: idm_admin recovery failed"; exit 1; }

    # --- Authenticate via REST API ---
    auth_kanidm() {
      local user="$1" pass="$2" cookies headers
      cookies=$(mktemp) headers=$(mktemp)
      $C -D "$headers" -c "$cookies" -X POST "$KANIDM_URL/v1/auth" \
        -H "Content-Type: application/json" \
        -d "{\"step\":{\"init\":\"$user\"}}" > /dev/null
      $C -b "$cookies" -c "$cookies" -X POST "$KANIDM_URL/v1/auth" \
        -H "Content-Type: application/json" \
        -d '{"step":{"begin":"password"}}' > /dev/null
      local resp
      resp=$($C -b "$cookies" -X POST "$KANIDM_URL/v1/auth" \
        -H "Content-Type: application/json" \
        -d "{\"step\":{\"cred\":{\"password\":\"$pass\"}}}")
      rm -f "$headers" "$cookies"
      echo "$resp" | ${pkgs.jq}/bin/jq -r '.state.success // empty'
    }

    ADMIN_TOKEN=$(auth_kanidm "admin" "$ADMIN_PASS")
    IDM_TOKEN=$(auth_kanidm "idm_admin" "$IDM_ADMIN_PASS")
    [ -z "$ADMIN_TOKEN" ] || [ -z "$IDM_TOKEN" ] && { echo "FATAL: auth failed"; exit 1; }

    # --- Configure dev credential policy (password-only, no MFA) ---
    $C -X POST "$KANIDM_URL/v1/group/idm_all_persons/_attr/class" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '["account_policy"]' > /dev/null 2>&1 || true
    $C -X PUT "$KANIDM_URL/v1/group/idm_all_persons/_attr/credential_type_minimum" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '["any"]' > /dev/null 2>&1 || true
    $C -X PUT "$KANIDM_URL/v1/group/idm_all_persons/_attr/auth_password_minimum_length" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '["1"]' > /dev/null 2>&1 || true

    # --- Create hearth-users group with POSIX attrs ---
    $C -X POST "$KANIDM_URL/v1/group" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '{"attrs":{"name":["hearth-users"]}}' > /dev/null 2>&1 || true
    $C -X POST "$KANIDM_URL/v1/group/hearth-users/_attr/class" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '["posixgroup"]' > /dev/null 2>&1 || true
    $C -X PUT "$KANIDM_URL/v1/group/hearth-users/_attr/gidnumber" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '["50000"]' > /dev/null 2>&1 || true

    # --- Create testuser with POSIX attrs ---
    $C -X POST "$KANIDM_URL/v1/person" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '{"attrs":{"name":["testuser"],"displayname":["Test User"]}}' > /dev/null 2>&1 || true
    $C -X POST "$KANIDM_URL/v1/person/testuser/_attr/class" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '["posixaccount"]' > /dev/null 2>&1 || true
    $C -X PUT "$KANIDM_URL/v1/person/testuser/_attr/loginshell" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '["/bin/bash"]' > /dev/null 2>&1 || true
    $C -X POST "$KANIDM_URL/v1/group/hearth-users/_attr/member" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d '["testuser"]' > /dev/null 2>&1 || true

    # --- Set testuser POSIX password ---
    # kanidm-unixd PAM uses unix_auth which requires a POSIX password
    # (separate from the primary/web password). We set it via the credential
    # update API using the idm_admin token.
    POSIX_PASS="hearth-test-password-42"
    CU_RESP=$($C -X GET "$KANIDM_URL/v1/person/testuser/_credential/_update" \
      -H "Authorization: Bearer $IDM_TOKEN" 2>/dev/null)
    CU_TOKEN=$(echo "$CU_RESP" | ${pkgs.jq}/bin/jq -r '.[0].token // empty' 2>/dev/null || true)
    if [ -n "$CU_TOKEN" ]; then
      # Set primary password via credential update intent token
      $C -X PUT "$KANIDM_URL/v1/credential/_update/$CU_TOKEN/primarypassword" \
        -H "Content-Type: application/json" \
        -d "{\"value\":\"$POSIX_PASS\"}" 2>/dev/null || true
      $C -X GET "$KANIDM_URL/v1/credential/_update/$CU_TOKEN/commit" 2>/dev/null || true
      echo "[bootstrap] Set primary password via credential update"
    fi

    # Set the POSIX/unix password (used by kanidm-unixd for PAM auth)
    # Kanidm expects SingleStringRequest: {"value": "..."}
    $C -X PUT "$KANIDM_URL/v1/person/testuser/_unix/_credential" \
      -H "Authorization: Bearer $IDM_TOKEN" \
      -H "Content-Type: application/json" \
      -d "{\"value\":\"$POSIX_PASS\"}"
    echo "[bootstrap] Set POSIX password for testuser"
    echo "$POSIX_PASS" > /tmp/testuser-password

    touch /tmp/bootstrap-done
    echo "[bootstrap] Done!"
  '';
in
{
  # Path to the CA certificate for client configuration.
  caCertPath = "${certs}/ca.pem";

  # The certs derivation (for direct access if needed).
  inherit certs;

  # NixOS module for a Kanidm server test node with bootstrap.
  # Usage: imports = [ (kanidmTest.module {}) ];
  module = { port ? 8443 }: { config, ... }: {
    services.kanidm = {
      package = pkgs.kanidm_1_7;
      server.enable = true;
      server.settings = {
        origin = "https://kanidm:${toString port}";
        domain = "kanidm";
        bindaddress = "0.0.0.0:${toString port}";
        tls_chain = "${certs}/cert.pem";
        tls_key = "${certs}/key.pem";
      };
    };

    networking.firewall.allowedTCPPorts = [ port ];

    systemd.services.kanidm-bootstrap = {
      description = "Bootstrap Kanidm with Hearth test data";
      after = [ "kanidm.service" ];
      wants = [ "kanidm.service" ];
      wantedBy = [ "multi-user.target" ];
      path = [ pkgs.curl pkgs.jq pkgs.openssl ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = bootstrapScript;
      };
    };

    virtualisation.memorySize = 1024;
  };
}

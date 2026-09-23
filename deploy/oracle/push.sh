#!/usr/bin/env bash
# Copies the aarch64 meshd binary to the VM and starts the mirror with a fresh token. Usage: push.sh <ip> [token]
set -euo pipefail
IP="${1:?public ip}"; TOKEN="${2:-$(openssl rand -hex 16)}"
BIN="$(dirname "$0")/../../desktop/target/aarch64-unknown-linux-gnu/release/meshd"
[[ -x "$BIN" ]] || { echo "build first: cargo zigbuild --release --target aarch64-unknown-linux-gnu -p meshd"; exit 1; }
scp -o StrictHostKeyChecking=accept-new "$BIN" "ubuntu@$IP:/tmp/meshd"
ssh "ubuntu@$IP" "sudo install -m 0755 /tmp/meshd /opt/meshai/meshd && echo MESHAI_MIRROR_TOKEN=$TOKEN | sudo tee /etc/meshai/mirror.env >/dev/null && sudo chmod 600 /etc/meshai/mirror.env && sudo systemctl daemon-reload && sudo systemctl enable --now meshd-mirror && sleep 1 && curl -s localhost:8080/api/state | head -c 200"
echo
echo "mirror: http://$IP:8080/admin"
echo "laptop: MESHAI_PUSH_TO=http://$IP:8080 MESHAI_PUSH_TOKEN=$TOKEN desktop/target/release/meshd serve"

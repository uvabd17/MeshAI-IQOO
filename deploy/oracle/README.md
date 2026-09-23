# Oracle Cloud deployment — MeshAI mirror (internet-facing, read-only admin)

What goes to the cloud: **only the admin panel in mirror mode**. The laptop's `meshd serve --push-to https://<vm>:8080 --push-token <T>` pushes its live state (devices, plan, run, timing rows) every 2 s; the cloud `meshd mirror` serves `/admin` from that snapshot. Inference, model files and the RPC data plane never leave the mesh (D002/D009).

Shape: `VM.Standard.A1.Flex` (Ampere, Always Free: up to 4 OCPU / 24 GB in the tenancy), region `ap-hyderabad-1`, AD `TDfW:AP-HYDERABAD-1-AD-1`, compartment `nativesubs-prod` (only compartment in the tenancy).

## Steps (run only after explicit approval — `up.sh` creates billable-looking resources even within Always Free)

1. Build the ARM binary: `cargo zigbuild --release --target aarch64-unknown-linux-gnu -p meshd` (desktop/).
2. `deploy/oracle/up.sh` — creates VCN + subnet + internet gateway + security list (22, 8080), launches the A1 instance with `cloud-init.yaml`, prints its public IP.
3. `deploy/oracle/push.sh <ip>` — scp the binary + `admin` is embedded; install the systemd unit with the mirror token; enable + start.
4. On the laptop: `MESHAI_PUSH_TO=http://<ip>:8080 MESHAI_PUSH_TOKEN=<T> meshd serve`.
5. Open `http://<ip>:8080/admin` from anywhere.

TLS: put the VM behind Cloudflare Tunnel or Caddy (`caddy reverse-proxy --from mesh.example.com --to :8080`) once a domain exists; the token is sent as a bearer header, so plain HTTP is for the hackathon LAN/demo only.

Tear-down: `deploy/oracle/down.sh` terminates the instance and deletes the network.

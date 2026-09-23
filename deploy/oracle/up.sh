#!/usr/bin/env bash
# Creates the network + an Always-Free A1 instance for the MeshAI mirror. Idempotent-ish (names are checked first).
# REQUIRES explicit approval before running. Prints the public IP at the end.
set -euo pipefail
COMP="${OCI_COMPARTMENT_ID:?set OCI_COMPARTMENT_ID (compartment OCID, e.g. nativesubs-prod)}"
AD="${OCI_AD:-TDfW:AP-HYDERABAD-1-AD-1}"
NAME="${MESHAI_VM_NAME:-meshai-mirror}"
SSH_PUB="${SSH_PUB:-$HOME/.ssh/id_ed25519.pub}"
q() { oci "$@" --output json; }

vcn=$(q network vcn list -c "$COMP" --display-name "$NAME-vcn" --query 'data[0].id' --raw-output 2>/dev/null || true)
if [[ -z "$vcn" || "$vcn" == "null" ]]; then
  vcn=$(q network vcn create -c "$COMP" --display-name "$NAME-vcn" --cidr-block 10.42.0.0/16 --dns-label meshai --wait-for-state AVAILABLE --query 'data.id' --raw-output)
  ig=$(q network internet-gateway create -c "$COMP" --vcn-id "$vcn" --is-enabled true --display-name "$NAME-ig" --wait-for-state AVAILABLE --query 'data.id' --raw-output)
  rt=$(q network vcn get --vcn-id "$vcn" --query 'data."default-route-table-id"' --raw-output)
  oci network route-table update --rt-id "$rt" --route-rules "[{\"destination\":\"0.0.0.0/0\",\"networkEntityId\":\"$ig\"}]" --force >/dev/null
  sl=$(q network vcn get --vcn-id "$vcn" --query 'data."default-security-list-id"' --raw-output)
  oci network security-list update --security-list-id "$sl" --force --ingress-security-rules '[
    {"protocol":"6","source":"0.0.0.0/0","tcpOptions":{"destinationPortRange":{"min":22,"max":22}}},
    {"protocol":"6","source":"0.0.0.0/0","tcpOptions":{"destinationPortRange":{"min":8080,"max":8080}}},
    {"protocol":"6","source":"0.0.0.0/0","tcpOptions":{"destinationPortRange":{"min":443,"max":443}}}]' \
    --egress-security-rules '[{"protocol":"all","destination":"0.0.0.0/0"}]' >/dev/null
  subnet=$(q network subnet create -c "$COMP" --vcn-id "$vcn" --display-name "$NAME-subnet" --cidr-block 10.42.1.0/24 --dns-label mesh --wait-for-state AVAILABLE --query 'data.id' --raw-output)
else
  subnet=$(q network subnet list -c "$COMP" --vcn-id "$vcn" --query 'data[0].id' --raw-output)
fi
echo "vcn=$vcn subnet=$subnet"

image=$(q compute image list -c "$COMP" --operating-system "Canonical Ubuntu" --operating-system-version "22.04" --shape "VM.Standard.A1.Flex" --sort-by TIMECREATED --sort-order DESC --query 'data[0].id' --raw-output)
inst=$(q compute instance list -c "$COMP" --display-name "$NAME" --lifecycle-state RUNNING --query 'data[0].id' --raw-output 2>/dev/null || true)
if [[ -z "$inst" || "$inst" == "null" ]]; then
  inst=$(q compute instance launch -c "$COMP" --availability-domain "$AD" --shape "VM.Standard.A1.Flex" --shape-config '{"ocpus":1,"memoryInGBs":6}' \
    --image-id "$image" --subnet-id "$subnet" --assign-public-ip true --display-name "$NAME" \
    --ssh-authorized-keys-file "$SSH_PUB" --user-data-file "$(dirname "$0")/cloud-init.yaml" \
    --wait-for-state RUNNING --query 'data.id' --raw-output)
fi
ip=$(q compute instance list-vnics --instance-id "$inst" --query 'data[0]."public-ip"' --raw-output)
echo "instance=$inst public_ip=$ip"
echo "$ip" > "$(dirname "$0")/.last-ip"

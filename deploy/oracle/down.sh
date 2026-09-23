#!/usr/bin/env bash
# Terminates the mirror instance and deletes its network. Asks before doing anything.
set -euo pipefail
COMP="${OCI_COMPARTMENT_ID:?}"; NAME="${MESHAI_VM_NAME:-meshai-mirror}"
read -r -p "Terminate $NAME and delete $NAME-vcn in $COMP? [y/N] " a; [[ "$a" == "y" ]] || exit 1
inst=$(oci compute instance list -c "$COMP" --display-name "$NAME" --lifecycle-state RUNNING --query 'data[0].id' --raw-output)
[[ -n "$inst" && "$inst" != "null" ]] && oci compute instance terminate --instance-id "$inst" --force --wait-for-state TERMINATED
vcn=$(oci network vcn list -c "$COMP" --display-name "$NAME-vcn" --query 'data[0].id' --raw-output)
if [[ -n "$vcn" && "$vcn" != "null" ]]; then
  for s in $(oci network subnet list -c "$COMP" --vcn-id "$vcn" --query 'data[].id' --raw-output | tr -d '[]", '); do oci network subnet delete --subnet-id "$s" --force --wait-for-state TERMINATED; done
  for g in $(oci network internet-gateway list -c "$COMP" --vcn-id "$vcn" --query 'data[].id' --raw-output | tr -d '[]", '); do oci network internet-gateway delete --ig-id "$g" --force --wait-for-state TERMINATED; done
  oci network vcn delete --vcn-id "$vcn" --force --wait-for-state TERMINATED
fi
echo "done"

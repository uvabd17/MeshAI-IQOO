"""
Decide where a model runs.

Three possible answers:
  SINGLE  the model fits on one device, so run it there and use nobody else
  SPLIT   the model fits nowhere alone, so cut it into contiguous layer
          ranges across devices
  REFUSE  it fits nowhere at all, and we say how much memory was missing

Rules that come from measurement, not taste:
  * Adding a device to a model that already fits makes it SLOWER, never
    faster (prima.cpp, arXiv 2504.08791, measured no gain at 8B and 14B).
    So we never split when we do not have to.
  * A link with a round trip over ~60 ms makes decoding painful, because
    every single token waits for one round trip. Shared Wi-Fi measured
    65 to 154 ms in our own tests, enterprise Wi-Fi 250 ms at p99
    (Sui et al., MobiSys 2016). Such a device is refused.
  * A hot or nearly flat phone is refused: throughput falls in steps as
    phones throttle (MELTing Point, MobiCom 2024).
  * Layers must stay in contiguous runs, host first. Every extra boundary
    costs one more network crossing per token.
  * The embeddings and the output head stay on the host. Moving the head
    would mean sending the full vocabulary scores per token instead of one
    small hidden state.
"""

from __future__ import annotations

from dataclasses import dataclass, field

MB = 1024 ** 2
GB = 1024 ** 3

# policy thresholds, all justified above
MAX_RTT_MS = 60.0
MIN_BATTERY_PCT = 20
MAX_THERMAL = 0.95          # 1.0 means no headroom left
COMPUTE_RESERVE = 256 * MB  # scratch buffers the engine allocates per device


@dataclass
class Device:
    id: str
    name: str
    usable_bytes: int          # free memory MINUS the safety headroom
    is_local: bool = False     # the laptop running meshd
    can_host: bool = True      # can it run llama-server, not just help
    rtt_p95_ms: float = 0.0    # measured link round trip, 0 for local
    thermal: float = 0.0       # 0 cool, 1 no headroom left
    battery_pct: int = 100
    charging: bool = True
    bench_tps: float = 0.0     # measured speed, used only to rank hosts
    online: bool = True


@dataclass
class Share:
    device_id: str
    device_name: str
    first_layer: int
    last_layer: int            # inclusive
    bytes: int                 # weights plus this device's cache share
    is_host: bool = False

    @property
    def n_layers(self) -> int:
        return self.last_layer - self.first_layer + 1


@dataclass
class Plan:
    mode: str                          # "single" | "split" | "refuse"
    model_path: str
    n_ctx: int
    host_id: str = ""
    shares: list = field(default_factory=list)      # list[Share]
    refusals: list = field(default_factory=list)    # list[(device_name, reason)]
    reason: str = ""
    needed_bytes: int = 0
    available_bytes: int = 0

    @property
    def ok(self) -> bool:
        return self.mode in ("single", "split")

    def describe(self) -> str:
        if self.mode == "refuse":
            return (f"cannot run: needs {self.needed_bytes/GB:.2f} GB, "
                    f"devices offer {self.available_bytes/GB:.2f} GB")
        if self.mode == "single":
            s = self.shares[0]
            return f"runs on {s.device_name} alone, all {s.n_layers} layers"
        parts = ", ".join(
            f"{s.device_name} layers {s.first_layer} to {s.last_layer}"
            for s in self.shares)
        return f"split across {len(self.shares)} devices: {parts}"


def eligibility(d: Device) -> str | None:
    """Return the reason this device cannot take part, or None if it can."""
    if not d.online:
        return "offline"
    if not d.is_local and d.rtt_p95_ms > MAX_RTT_MS:
        return (f"link too slow ({d.rtt_p95_ms:.0f} ms round trip, "
                f"limit {MAX_RTT_MS:.0f} ms): every token would wait for it")
    if d.thermal > MAX_THERMAL:
        return "too hot: speed would drop mid answer"
    if d.battery_pct < MIN_BATTERY_PCT and not d.charging:
        return f"battery {d.battery_pct}% and not charging"
    if d.usable_bytes <= COMPUTE_RESERVE:
        return "not enough free memory to hold anything"
    return None


def _capacity(d: Device) -> int:
    return max(0, d.usable_bytes - COMPUTE_RESERVE)


def plan(model, devices: list[Device], n_ctx: int = 4096,
         prefer_host: str = "") -> Plan:
    """
    model: a gguf.Model
    devices: everything currently paired, including the laptop itself
    """
    kv_per_layer = (model.kv_bytes_per_token * n_ctx) / max(model.n_layers, 1)
    fixed = model.non_layer_bytes            # embeddings and head, host only
    needed = model.weight_bytes + model.kv_bytes(n_ctx)

    usable, refusals = [], []
    for d in devices:
        why = eligibility(d)
        if why:
            refusals.append((d.name, why))
        else:
            usable.append(d)

    if not usable:
        return Plan("refuse", model.path, n_ctx, refusals=refusals,
                    reason="no usable device", needed_bytes=needed)

    hosts = [d for d in usable if d.can_host]
    if prefer_host:
        hosts = [d for d in hosts if d.id == prefer_host] or hosts

    # --- can one device hold everything? then do that, and use nobody else
    # rank by measured speed, then prefer the local machine on a tie so a
    # model does not get shipped to a phone for no reason
    for d in sorted(hosts, key=lambda x: (-x.bench_tps, not x.is_local)):
        if _capacity(d) >= needed:
            share = Share(d.id, d.name, 0, model.n_layers - 1, needed, True)
            others = [(o.name, "not needed: the model already fits on "
                       f"{d.name}, another device would only add waiting")
                      for o in usable if o.id != d.id]
            return Plan("single", model.path, n_ctx, d.id, [share],
                        refusals + others,
                        reason=f"fits on {d.name} with "
                               f"{(_capacity(d)-needed)/GB:.2f} GB to spare")

    # --- otherwise split. the host keeps the fixed parts plus a prefix
    host = None
    for d in sorted(hosts, key=lambda x: (-_capacity(x), not x.is_local)):
        if _capacity(d) >= fixed + kv_per_layer:      # room for the head + 1 layer
            host = d
            break
    if host is None:
        return Plan("refuse", model.path, n_ctx, refusals=refusals,
                    reason="no device can hold the embeddings and output head",
                    needed_bytes=fixed,
                    available_bytes=max((_capacity(d) for d in usable), default=0))

    helpers = sorted((d for d in usable if d.id != host.id),
                     key=lambda x: -_capacity(x))
    order = [host] + helpers

    shares, layer, room_total = [], 0, 0
    for i, d in enumerate(order):
        room = _capacity(d) - (fixed if d.id == host.id else 0)
        room_total += max(room, 0)
        taken, used = 0, 0
        while layer < model.n_layers:
            cost = model.layer_bytes[layer] + kv_per_layer
            if used + cost > room:
                break
            used += cost
            taken += 1
            layer += 1
        if taken:
            shares.append(Share(
                d.id, d.name, layer - taken, layer - 1,
                int(used + (fixed if d.id == host.id else 0)),
                is_host=(d.id == host.id)))
        elif d.id == host.id:
            # host holds the head but no layers: legal, llama.cpp keeps the
            # head on the host and offloads every block
            shares.append(Share(d.id, d.name, 0, -1, fixed, is_host=True))
        if layer >= model.n_layers:
            break

    if layer < model.n_layers:
        return Plan("refuse", model.path, n_ctx, refusals=refusals,
                    reason=f"{model.n_layers - layer} of {model.n_layers} "
                           "layers have nowhere to go",
                    needed_bytes=needed, available_bytes=room_total)

    return Plan("split", model.path, n_ctx, host.id, shares, refusals,
                reason=f"does not fit any single device, cut across "
                       f"{len(shares)} devices")


if __name__ == "__main__":
    import sys
    import gguf

    m = gguf.read(sys.argv[1] if len(sys.argv) > 1
                  else __import__("os").path.expanduser(
                      "~/models/Qwen3-0.6B-Q8_0.gguf"))
    print(m.summary(), "\n")

    laptop = Device("local", "this laptop", usable_bytes=int(4.5 * GB),
                    is_local=True, bench_tps=12)
    phone = Device("p1", "phone", usable_bytes=int(3.0 * GB),
                   rtt_p95_ms=4, bench_tps=20)
    slow = Device("p2", "phone on venue wifi", usable_bytes=int(3.0 * GB),
                  rtt_p95_ms=120)

    for title, devs, ctx in [
        ("laptop alone", [laptop], 4096),
        ("laptop plus a fast phone", [laptop, phone], 4096),
        ("laptop plus a phone on bad wifi", [laptop, slow], 4096),
        ("tiny laptop, model does not fit",
         [Device("local", "this laptop", usable_bytes=int(0.8 * GB),
                 is_local=True), phone], 4096),
    ]:
        p = plan(m, devs, ctx)
        print(f"--- {title}")
        print("   ", p.describe())
        print("   ", p.reason)
        for name, why in p.refusals:
            print(f"    refused {name}: {why}")
        print()

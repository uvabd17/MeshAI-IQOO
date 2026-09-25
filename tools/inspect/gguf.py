"""
Read the header of a GGUF model file.

We never load the weights. We only need to know, before anything runs:
  - how many layers ("blocks") the model has
  - how many bytes each layer occupies on disk
  - how many bytes the parts that cannot move occupy (embeddings, output head)
  - how much memory the conversation cache (KV cache) costs per token

Those four numbers are what the planner needs to decide whether a model fits
on one device, or how to cut it across several.

Layer sizes are derived from the DIFFERENCE between consecutive tensor
offsets rather than from the quantisation type. That way a new quantisation
format (MXFP4, IQ4, whatever ships next month) needs no change here.

Format reference: ggml-org/llama.cpp/docs/gguf.md
"""

from __future__ import annotations

import os
import re
import struct
from dataclasses import dataclass, field

MAGIC = b"GGUF"

# value type ids used in the metadata section
(UINT8, INT8, UINT16, INT16, UINT32, INT32, FLOAT32, BOOL, STRING, ARRAY,
 UINT64, INT64, FLOAT64) = range(13)

_FIXED = {
    UINT8: ("<B", 1), INT8: ("<b", 1),
    UINT16: ("<H", 2), INT16: ("<h", 2),
    UINT32: ("<I", 4), INT32: ("<i", 4), FLOAT32: ("<f", 4),
    BOOL: ("<?", 1),
    UINT64: ("<Q", 8), INT64: ("<q", 8), FLOAT64: ("<d", 8),
}

BLOCK_RE = re.compile(r"^blk\.(\d+)\.")


class GGUFError(Exception):
    pass


class _Reader:
    """Sequential reader over the header bytes."""

    def __init__(self, f):
        self.f = f

    def raw(self, n: int) -> bytes:
        b = self.f.read(n)
        if len(b) != n:
            raise GGUFError("file ended inside the header")
        return b

    def fixed(self, vtype: int):
        fmt, size = _FIXED[vtype]
        return struct.unpack(fmt, self.raw(size))[0]

    def string(self) -> str:
        n = self.fixed(UINT64)
        if n > 64 * 1024 * 1024:
            raise GGUFError("implausible string length in header")
        return self.raw(n).decode("utf-8", "replace")

    def value(self, vtype: int):
        if vtype in _FIXED:
            return self.fixed(vtype)
        if vtype == STRING:
            return self.string()
        if vtype == ARRAY:
            inner = self.fixed(UINT32)
            count = self.fixed(UINT64)
            # long arrays (token lists) are skipped: we never need them
            if count > 100_000:
                if inner in _FIXED:
                    self.raw(_FIXED[inner][1] * count)
                elif inner == STRING:
                    for _ in range(count):
                        self.string()
                else:
                    raise GGUFError(f"cannot skip array of type {inner}")
                return f"<{count} items>"
            return [self.value(inner) for _ in range(count)]
        raise GGUFError(f"unknown metadata value type {vtype}")


@dataclass
class Tensor:
    name: str
    dims: tuple
    dtype: int
    offset: int
    size: int = 0          # filled in afterwards, from the next offset


@dataclass
class Model:
    path: str
    file_bytes: int
    arch: str
    n_layers: int
    layer_bytes: list = field(default_factory=list)   # bytes per block, in order
    non_layer_bytes: int = 0        # embeddings, output head, norms
    kv_bytes_per_token: int = 0     # conversation cache cost, f16
    n_ctx_train: int = 0            # longest context the model was trained for
    n_embd: int = 0
    metadata: dict = field(default_factory=dict)

    @property
    def weight_bytes(self) -> int:
        return sum(self.layer_bytes) + self.non_layer_bytes

    def kv_bytes(self, n_ctx: int) -> int:
        return self.kv_bytes_per_token * n_ctx

    def total_bytes(self, n_ctx: int) -> int:
        """Weights plus conversation cache. The number that must fit."""
        return self.weight_bytes + self.kv_bytes(n_ctx)

    def summary(self) -> str:
        gb = 1024 ** 3
        return (f"{os.path.basename(self.path)}: {self.arch}, "
                f"{self.n_layers} layers, weights {self.weight_bytes/gb:.2f} GB, "
                f"fixed parts {self.non_layer_bytes/gb:.2f} GB, "
                f"cache {self.kv_bytes_per_token/1024:.0f} KB per token")


def _meta_int(meta: dict, arch: str, *suffixes, default=0) -> int:
    for s in suffixes:
        for key in (f"{arch}.{s}", s):
            if key in meta and isinstance(meta[key], int):
                return meta[key]
    return default


def read(path: str) -> Model:
    """Read one GGUF file's header and return the numbers the planner needs."""
    file_bytes = os.path.getsize(path)
    with open(path, "rb") as f:
        r = _Reader(f)
        if r.raw(4) != MAGIC:
            raise GGUFError("not a GGUF file")
        version = r.fixed(UINT32)
        if version not in (2, 3):
            raise GGUFError(f"unsupported GGUF version {version}")
        n_tensors = r.fixed(UINT64)
        n_kv = r.fixed(UINT64)

        meta = {}
        for _ in range(n_kv):
            key = r.string()
            vtype = r.fixed(UINT32)
            meta[key] = r.value(vtype)

        tensors = []
        for _ in range(n_tensors):
            name = r.string()
            n_dims = r.fixed(UINT32)
            dims = tuple(r.fixed(UINT64) for _ in range(n_dims))
            dtype = r.fixed(UINT32)
            offset = r.fixed(UINT64)
            tensors.append(Tensor(name, dims, dtype, offset))

        # tensor data starts after the header, padded to the alignment
        align = meta.get("general.alignment", 32) or 32
        here = f.tell()
        data_start = (here + align - 1) // align * align

    # size of each tensor = distance to the next one; the last runs to the end
    tensors.sort(key=lambda t: t.offset)
    for i, t in enumerate(tensors):
        end = tensors[i + 1].offset if i + 1 < len(tensors) else file_bytes - data_start
        t.size = max(0, end - t.offset)

    arch = meta.get("general.architecture", "unknown")
    n_layers = _meta_int(meta, arch, "block_count")

    layer_bytes = [0] * max(n_layers, 0)
    non_layer = 0
    for t in tensors:
        m = BLOCK_RE.match(t.name)
        if m:
            idx = int(m.group(1))
            if idx >= len(layer_bytes):        # header lied, grow the list
                layer_bytes.extend([0] * (idx + 1 - len(layer_bytes)))
            layer_bytes[idx] += t.size
        else:
            non_layer += t.size
    if not n_layers:
        n_layers = len(layer_bytes)

    # conversation cache: per layer, keys and values, one entry per attention
    # head that stores them, at 2 bytes each (f16, llama.cpp's default)
    n_embd = _meta_int(meta, arch, "embedding_length")
    n_head = _meta_int(meta, arch, "attention.head_count")
    n_head_kv = _meta_int(meta, arch, "attention.head_count_kv", default=n_head)
    head_k = _meta_int(meta, arch, "attention.key_length",
                       default=(n_embd // n_head if n_head else 0))
    head_v = _meta_int(meta, arch, "attention.value_length",
                       default=(n_embd // n_head if n_head else 0))
    kv_per_token = n_layers * n_head_kv * (head_k + head_v) * 2

    return Model(
        path=path,
        file_bytes=file_bytes,
        arch=arch,
        n_layers=n_layers,
        layer_bytes=layer_bytes,
        non_layer_bytes=non_layer,
        kv_bytes_per_token=kv_per_token,
        n_ctx_train=_meta_int(meta, arch, "context_length"),
        n_embd=n_embd,
        metadata={k: v for k, v in meta.items() if not k.startswith("tokenizer.")},
    )


if __name__ == "__main__":
    import sys
    for p in sys.argv[1:]:
        m = read(p)
        print(m.summary())
        print(f"  trained context: {m.n_ctx_train}, hidden size: {m.n_embd}")
        print(f"  needs at 4k context: {m.total_bytes(4096)/1024**3:.2f} GB")
        if m.layer_bytes:
            mb = 1024 ** 2
            print(f"  per layer: {min(m.layer_bytes)/mb:.0f} to "
                  f"{max(m.layer_bytes)/mb:.0f} MB")

# iOS terrain frame governor

The iOS client adjusts optional terrain work to presentation cadence. A single late frame does
not reduce work: overload must accumulate for 200 ms before one budget step is removed. Further
reductions wait at least 750 ms. A step is restored after two seconds of headroom, and changes
are limited to one step per 750 ms. It uses the effective FPS limit and monitor
refresh rate (60 Hz fallback), CPU frame work excluding presentation waits, and GPU timings
already available asynchronously. It adds no GPU readback, fence wait, or per-frame allocation.
Normal VSync waiting is not overload. Gaps over 250 ms reset timing history.

Upload budgets range from 256 KiB to 768 KiB and at most 0.25–0.75 ms at 60 Hz; faster refresh
rates get a smaller time allowance. Explicit chunk count and byte limits still apply. The
nearest section always makes progress, even when one mesh exceeds the budget. Far-terrain
uploads receive a separate quarter of the adaptive time allowance (at most 0.19 ms at
60 Hz) instead of a fixed 2 ms, so they continue progressing even while near chunks load.

Mesh admission counts running jobs and completed uploads, caps them at 6–14 sections, and
reserves memory using the largest observed mesh allocation (at least 1 MiB). Allocated
capacity includes source, packed, and auxiliary buffers. Completed meshes waiting for upload
count against a 16 MiB allowance. An iOS memory warning lowers that to 4 MiB and keeps the
lowest work budget for five seconds of active frame samples. These are admission estimates,
not limits on total RAM: already running jobs finish, individual sections may exceed the
estimate, and loaded chunks, renderer buffers, mesh reuse pools and other systems retain
memory separately. Render distance and world simulation do not change.

## Validation

Behavior tests live in `game/frame_governor.rs` and `streaming/mesh_budget.rs`. They cover
isolated late-frame filtering, gradual recovery, GPU overload, memory warnings across suspension, refresh
changes, allocation backpressure, rebuild admission, and progress after oversized meshes.
The existing upload-budget tests cover progress for an oversized first section.

Run the repository self-check on a Rust/Python development host and compile the iOS target;
the governor integration is iOS-specific and desktop tests alone cannot check it:

```sh
python3 tools/pr_ready.py --quick
cargo check --locked --target aarch64-apple-ios -p horizon-client --bin horizon-client
```

For an on-device A/B comparison, use identical options and a disposable world at render
distance 20. Record at least ten minutes of the same route through dense forests, including
a warm repeat, rapid turns, and teleports. Compare p95/p99 frame time, frames exceeding twice
the target interval, resident memory, pending uploads, and time until terrain fills. Repeat
with a 30 FPS cap and at the device's native refresh. A slower terrain fill is the expected
tradeoff under pressure. CPU/GPU submission and presentation stalls can still happen; this
controller limits additional terrain load but does not remove those synchronization points.

No on-device improvement or Rust test result has been measured in the editing environment,
which has neither Cargo/rustc nor Python available.

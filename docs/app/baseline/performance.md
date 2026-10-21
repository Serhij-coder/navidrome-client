# P00.07 performance protocol and candidate budgets

No original runtime measurements exist yet. All limits below are numerical candidate release budgets, chosen to bound the P01 experiments. They are not calibrated or verified. P00 remains open until original samples and full hardware/display records exist and these candidates are calibrated; P01 must not declare success against uncalibrated numbers.

Development host observed 2026-10-21: AMD Ryzen 7 7435HS, 8 cores/16 threads, 16,039,136 KiB physical RAM; x86_64 Nobara Linux 44 KDE, kernel `7.2.9-200.nobara.fc44.x86_64`. GPU model/driver, compositor session, display size/scale/refresh, audio device/driver and power/thermal state are unrecorded. Browser automation returned no available surfaces. This incomplete host record is not the release reference matrix. Windows/macOS/aarch64/mobile observations are absent; see [ADR-002](../adr/002-target-matrix.md).

| Desktop metric | Candidate limit on declared 60 Hz reference hardware | Workload |
| --- | --- | --- |
| Cold startup to interactive library | p95 <=2500 ms | 10 launches with cold OS cache or fresh VM, persisted 100k catalog, no server delay in local metric |
| Warm startup | p95 <=1000 ms | 30 launches after one warmup, same catalog/profile |
| Total RSS, all app/audio/helper processes | steady <=400 MiB, peak <=650 MiB | 100k rows with bounded artwork cache, playback; sum processes, record shared-page double counting |
| Idle visible CPU | <=1% of one logical CPU, <=20 wakeups/s | 60 s quiescent after 30 s settling; no playback/visualizer |
| Idle minimized/tray CPU | <=0.5% of one logical CPU, <=5 wakeups/s | Same interval; zero scheduled UI frames while quiescent |
| Playback CPU excluding visualization | <=5% of one logical CPU | Local lossless fixture; visible and minimized separately, include engine |
| Idle GPU | <=1% engine utilization | Visible/minimized/tray, clocks/utilization recorded; unsupported telemetry is unmeasured |
| Scrolling/sort/filter frame time | p95 <=16.7 ms, p99 <=33.3 ms | 100k table, selection/grouping/artwork + playback; at least 6000 frames |
| Input to visible paint | p95 <=50 ms, p99 <=100 ms | 100 keyboard/selection/drag actions; timestamp input and presented frame |
| Queue command application | p95 <=20 ms, p99 <=50 ms | 10k/100k queue, insert/remove/reorder/shuffle; application acknowledgement, separate paint/network |
| Sort/filter completion | p95 <=250 ms / <=150 ms | 100k rows, worker task; UI remains responsive; record cancellation and peak allocation |
| Gapless boundary discontinuity | absolute <=20 ms | 30 joins of generated test tones; record audible gap/overlap using loopback capture |
| Crossfade duration error | absolute <=50 ms | 1 s and 5 s transitions at 0.5x/1x/2x, both styles, 30 joins each |
| Reconnect | p95 <=5000 ms after server is reachable | 30 controlled outages, distinguish discovery/backoff/auth/network from local UI latency |
| Visualizer incremental cost | <=15% of one CPU, <=20% GPU, <=150 MiB RSS | Representative preset + playback, same frame budget; capture/backend/bridge costs included |
| Hidden visualizer work | no rendering/capture when no consumer requires it | Minimize/tray/engine switch; playback/reporting stay alive |

Record raw samples, sample counts, median/p95/p99 by nearest-rank, maxima and failures; never calculate a p99 claim from one sample. Save cold and warm separately. Use monotonic clocks, stable fixture hashes, release build/dependency versions and quiet local servers. Network setup and stream-start latency are separate samples from local command/frame work. CPU percentage uses one logical CPU as 100%; disclose multicore totals and tools. Sum all child/helper RSS, not just the UI process. Sample process lifetime, crash/reconnect and a 30-minute playback/scroll retention run to detect growth.

Run 10k and 100k synthetic library/queue fixtures in visible, unfocused, minimized and tray-only states, with idle, playback, artwork churn, sort/filter, visualization and reconnect workloads. Generate pages with [p00-library.py](../../../tools/app/p00-library.py); record page/count/offset and generated content hashes. Generated rows carry no playable audio. Create owned test-tone files and disposable server libraries for audio measurements; keep real libraries and profiles untouched. Measure original and later Rust under matching hardware, refresh, power, drivers, dataset and cache conditions.

Browser candidates are separate: warm interactive shell p95 <=2000 ms, scroll p95 <=16.7 ms/p99 <=33.3 ms at 60 Hz, input p95 <=100 ms, incremental tab memory <=300 MiB after loading 100k metadata. Record browser/version/codec, network/cache and foreground/background throttling. Browser process memory shared with other tabs is unsuitable without an isolated browser profile. These numbers also need P00 calibration; they do not substitute for native budgets.

Use per-run JSON/CSV artifacts with hardware/OS/driver/display/audio/build/fixture identifiers, raw timestamps, process tree, workload/state, counters and failures. A missing GPU counter or audio loopback is a gap, not zero cost. Do not extrapolate Linux results to Windows/macOS or one CPU architecture to another. Save the calibration decision and changed limits before P01 evaluation.

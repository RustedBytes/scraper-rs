## Summary

**v2 is much faster for small-to-mid `parse_scaling`, but slower for large `parse_scaling`.**
Compared with v0, v2 improves `2–128 KiB` parsing by roughly **3.3× to 5.0×**, but regresses at `512 KiB–8 MiB` by about **10–38%**. v0’s baseline parse-scaling numbers are in the uploaded log. 

### `parse_scaling`: v2 vs v0

|    Size | v0 midpoint | v2 midpoint |              Result |
| ------: | ----------: | ----------: | ------------------: |
|   2 KiB |   48.280 µs |    9.612 µs | **v2 5.02× faster** |
|   8 KiB |   190.75 µs |   58.368 µs | **v2 3.27× faster** |
|  32 KiB |   737.90 µs |   160.98 µs | **v2 4.58× faster** |
| 128 KiB |    2.975 ms |   645.94 µs | **v2 4.61× faster** |
| 512 KiB |   11.997 ms |   16.515 ms | **v2 37.7% slower** |
|   2 MiB |   52.844 ms |   58.327 ms | **v2 10.4% slower** |
|   8 MiB |   215.71 ms |   246.86 ms | **v2 14.4% slower** |

The cutoff is very clear: **v2 wins hard up to 128 KiB**, then **loses from 512 KiB onward**. That suggests a fixed-cost or algorithmic improvement for smaller documents, but worse scaling or a worse large-input path.

## `sync_async`: important changes

### Big wins in v2

| Benchmark                            |        v0 |        v2 |           Result |
| ------------------------------------ | --------: | --------: | ---------------: |
| `sync_select/small`                  |  2.039 µs |  1.350 µs | **1.51× faster** |
| `sync_select/medium`                 | 82.705 µs | 34.162 µs | **2.42× faster** |
| `sync_select/large`                  |  1.180 ms | 549.49 µs | **2.15× faster** |
| `async_spawn_blocking_select/small`  | 14.240 µs |  6.784 µs | **2.10× faster** |
| `async_spawn_blocking_select/medium` | 304.78 µs | 59.523 µs | **5.12× faster** |
| `async_spawn_blocking_select/large`  |  3.567 ms | 685.13 µs | **5.21× faster** |

The biggest improvement is clearly **`async_spawn_blocking_select`**, especially medium/large inputs. That path is now around **5× faster**.

### Regressions in v2

| Benchmark                  |        v0 |        v2 |           Result |
| -------------------------- | --------: | --------: | ---------------: |
| `sync_select_first/medium` | 16.182 µs | 38.781 µs | **2.40× slower** |
| `sync_first/medium`        | 16.126 µs | 40.334 µs | **2.50× slower** |
| `sync_select_first/large`  | 172.80 µs | 587.24 µs | **3.40× slower** |
| `sync_first/large`         | 172.12 µs | 578.64 µs | **3.36× slower** |

This is the main concern: **the “first” variants regressed badly for medium and large inputs**. In v0, `sync_first` / `sync_select_first` were extremely fast on medium and large inputs; in v2 they are much closer to full `sync_select`.

That likely means v2 lost some early-exit behavior, does more traversal before returning the first match, or changed the selector path so that “first” no longer avoids most of the work.

### XPath mostly unchanged

| Benchmark                    |                                     Result |
| ---------------------------- | -----------------------------------------: |
| `sync_xpath/small`           |                            v2 ~2.8% faster |
| `sync_xpath/medium`          |                            v2 ~1.8% slower |
| `sync_xpath/large`           |                            v2 ~3.9% slower |
| `sync_xpath_first/*`         | roughly flat, within ~1–2.5% slower/faster |
| `async_spawn_blocking_xpath` |           v2 faster: ~7–17% depending size |

XPath did **not** materially change. The async XPath path improved somewhat, but not dramatically.

## Verdict

**v2 is a mixed result.**

It is a strong improvement if your priority is:

* small-to-medium parsing,
* regular `sync_select`,
* async `spawn_blocking` selector performance.

But it is a regression if your priority is:

* large `parse_scaling`,
* `sync_first`,
* `sync_select_first`,
* preserving early-exit behavior on medium/large documents.

The most suspicious change is this:

> `sync_first/large`: **172.12 µs → 578.64 µs**, about **3.36× slower**.

I would investigate whether v2 accidentally made “first” perform a near-full selection/traversal before returning.

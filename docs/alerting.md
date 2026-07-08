# Alerting: Redis rate limiter fail-open errors

## What this covers

`RedisRateLimiter` fails open on any Redis error: `check()` returns `true` (allows
the request) and `remaining_attempts()` returns the max, rather than blocking
requests because Redis is briefly unavailable. Each occurrence is counted in-process
(see `RedisRateLimiter::redis_error_count` / `error_counter_handle`) and, as of #36,
published as a Prometheus metric:

```
rate_limiter_redis_errors_total{scope="login"}
rate_limiter_redis_errors_total{scope="refresh"}
```

This is a cumulative counter (never decreases while the process is running), polled
and re-published every 15 seconds from the in-process atomic counter maintained by
`RedisRateLimiter`. It resets to 0 on process restart.

## Scraping it

`GET /metrics` on the service exposes this (and any other `metrics`-crate-recorded
metrics) in Prometheus exposition format. Add a scrape target for it in your
Prometheus config, e.g.:

```yaml
scrape_configs:
  - job_name: app-home-services
    static_configs:
      - targets: ["app-home-services:3000"]
```

`/metrics` is not authenticated. Like any Prometheus scrape endpoint, it should only
be reachable from inside your monitoring network/namespace, not exposed publicly.

## The alert rule (`prometheus/alerts.yml`)

```yaml
expr: increase(rate_limiter_redis_errors_total[5m]) > 0
```

### Why the threshold starts at `> 0`

We don't yet have a baseline for how often *transient* Redis errors (a brief network
blip, a Redis failover, a deploy-time restart) happen in normal operation for this
deployment. Starting at the most sensitive possible threshold means:

- We won't miss a real, sustained Redis outage by having picked too high a number
  before we had any data.
- We *will* see false positives from routine blips at first -- that's expected and
  is the point: it's how we build the baseline.

### How to raise it later

Once the alert has been live for a while (a couple of weeks is a reasonable amount of
time to capture routine restarts/deploys/network blips):

1. Look at how often `RedisRateLimiterFailingOpen` fired for reasons that turned out
   to be routine noise (a deploy, a known brief Redis maintenance window) rather than
   a real problem.
2. Pick a new threshold comfortably above the peak of that routine noise -- e.g. if
   the worst routine blip you saw was 3 errors in 5 minutes, moving to
   `increase(rate_limiter_redis_errors_total[5m]) > 5` gives some margin without
   losing sensitivity to genuine outages.
3. Update the `expr` in `prometheus/alerts.yml` and note the date/reasoning in that
   change's commit message or PR description, so the next person adjusting it has
   the same context this document is trying to give you now.

Prefer raising the threshold gradually (re-evaluate again after another few weeks)
over jumping straight to a large number based on a guess.

# Alerting: Redis fail-open errors (rate limiter and access token blacklist)

## What this covers

### Redis rate limiter

`RedisRateLimiter` fails open on any Redis error: `check()` returns `true` (allows
the request) and `remaining_attempts()` returns the max, rather than blocking
requests because Redis is briefly unavailable. Each occurrence is counted in-process
(see `RedisRateLimiter::redis_error_count` / `error_counter_handle`) and, as of #36,
published as a Prometheus metric:

```text
rate_limiter_redis_errors_total{scope="login"}
rate_limiter_redis_errors_total{scope="refresh"}
```

This is a cumulative counter (never decreases while the process is running), polled
and re-published every 15 seconds from the in-process atomic counter maintained by
`RedisRateLimiter`. It resets to 0 on process restart.

### Redis access token blacklist

`RedisAccessTokenBlacklist` (see #88) also fails open on any Redis error: the
`AuthenticatedUser` extractor treats an unavailable revocation list as "not
revoked", so revoked access tokens keep validating until the outage clears -- an
availability-over-strictness choice made deliberately (the same posture as the rate
limiter). Each occurrence is counted in-process (see
`RedisAccessTokenBlacklist::redis_error_count` / `error_counter_handle`) and
published by `main.rs` as:

```text
access_token_blacklist_redis_errors_total
```

Same polling cadence (every 15 seconds) and same restart-resets-to-0 semantics as
the rate limiter counter.

### Durable-revocation outbox backlog

Since #140, revocations that Redis rejects are journaled in Postgres
(`access_token_revocation_outbox`) and retried by a background flush worker, so a
Redis outage can delay -- but never silently drop -- a logout's revocation. The
worker republishes the current backlog on every sweep (default every 5 seconds,
see `REVOCATION_FLUSH_INTERVAL_SECONDS`) as:

```text
access_token_revocation_outbox_pending
```

Unlike the two error counters above, this is a **gauge** (it goes up and down as
the backlog drains), and it is `0` by definition on the in-memory blacklist
backend (`REDIS_URL` unset), which never journals. A sustained non-zero value
means Redis has been down long enough that logged-out tokens are keeping their
freshly-revoked status unenforced on *other* instances (this instance still
rejects them immediately via an in-memory pending set).

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

## The alert rules (`prometheus/alerts.yml`)

Each metric has its own alert rule, in its own rule group:

```yaml
groups:
  - name: app-home-services-rate-limiter
    rules:
      - alert: RedisRateLimiterFailingOpen
        expr: increase(rate_limiter_redis_errors_total[5m]) > 0
        for: 1m
        labels:
          severity: warning
  - name: app-home-services-access-token-blacklist
    rules:
      - alert: RedisAccessTokenBlacklistFailingOpen
        expr: increase(access_token_blacklist_redis_errors_total[5m]) > 0
        for: 1m
        labels:
          severity: warning
      - alert: AccessTokenRevocationBacklogAccumulating
        expr: access_token_revocation_outbox_pending > 0
        for: 5m
        labels:
          severity: warning
```

(Trimmed to the structural parts; see `prometheus/alerts.yml` for the full
`annotations` on each alert.)

Both error-counter alerts start at the same deliberately low `> 0` threshold (see
below); the blacklist one is arguably the more important of the two to notice,
since a failing-open revocation list is a security degradation (revoked tokens
keep working), while a failing-open rate limiter is only a brute-force defense
weakening.

The third alert (`AccessTokenRevocationBacklogAccumulating`) is different in
kind: it doesn't fire on a blip, it fires on a *sustained* `> 0` outbox backlog
(5 minutes). Redis being down for a moment produces one or two failed revokes
that flush within seconds and never trips it; a Redis outage long enough to leave
the flush worker behind is what accumulates a backlog and fires it.

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

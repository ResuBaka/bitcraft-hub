# Leaderboard metric buckets

Bucket widths can be overridden in the API configuration (restart required):

```toml
[leaderboard.bucket_widths]
level = 50
# Optional overrides:
# global_experience = 10000
# xp_per_hour = 1000
# time_played = 3600
# time_signed_in = 3600
# skill_1 = 1000
```

Each override is a positive integer and selects fixed-width buckets starting at
zero: width 5 produces 0–4, 5–9, 10–14, etc. Width 1 produces a separate bucket
for every value. Negative scores are counted in the zero bucket.

Without an override, `level` uses width 50. Other boards retain the existing tiered
XP buckets. Skill overrides use `skill_<id>` and also apply to boards created
after startup. `profession_experience` and `adventure_experience` are supported
as well.

The level leaderboard stores **total levels across skills**. With 19 skills capped
at 130, the maximum total is 2,470. The default width gives 50 possible buckets
(0–49 through 2,450–2,499), with roughly 11 buckets covering the current top-100
range of 1,294–1,800. Totals are not capped, so future level/skill increases remain
visible. Only nonempty buckets are exported. Prometheus metric names and labels
remain unchanged.

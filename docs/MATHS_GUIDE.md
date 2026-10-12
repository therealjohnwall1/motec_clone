# Custom maths — quick guide (B3)

## The Excel mental model (yes, it's like Excel)

Imagine a spreadsheet where **each row is a timestamp** and each column is a
channel. A math channel is one formula filled down every row:

| t (s) | 'G Force Lat DAQ' | 'G Force Long DAQ' | G_Combined (your formula) |
|-------|-------------------|--------------------|---------------------------|
| 0.00  | 0.07              | 0.10               | `sqrt(0.07^2 + 0.10^2)` = 0.122 |
| 0.01  | 0.08              | 0.09               | `sqrt(0.08^2 + 0.09^2)` = 0.120 |
| …     | …                 | …                  | … |

That is exactly what happens: your expression is evaluated once per
timestamp. Operators and functions apply row-by-row.

## Rates and points (your question)

Logged channels don't share a rate: G-forces are 100 Hz, TPS 100 Hz,
`Bat Volts Dash` 25 Hz, `Dash Temp` 1 Hz. A math channel needs **one**
timeline, so:

- **Output rate** defaults to the **fastest referenced channel**
  (e.g. combining two 100 Hz channels gives a 100 Hz result), capped at
  1000 Hz. You can force another rate in the `Hz` box (1–1000).
- **Length** spans the longest input: points = duration × rate + 1
  (your log: ~1063 s × 100 Hz ≈ 106,300 points).
- **Looking up a slower channel at time t** = linear interpolation between
  its two nearest samples — the "VLOOKUP in time". A 1 Hz temp channel read
  at t = 0.37 s blends its 0.3 s and 0.4 s samples.
- **Short inputs hold their last value** past their end (documented, matches
  `--eval` behavior); before t = 0 the first value is used.

Consequence: forcing 100 Hz on an expression of 1 Hz channels just draws
straight lines between the 1 Hz points. No fake detail is invented.

## Syntax

- Channels with spaces/symbols go in **single quotes**: `'Steering Wheel Angle'`.
  Single-word names may skip quotes. Matching ignores case.
- Numbers: `3.6`, `0.05`, `1e3`. Operators: `+ - * / ^` (right-assoc),
  comparisons `== != > < >= <=` (give 1/0), logic `&& || !` or `AND OR NOT`.
- `=` alone is rejected (you meant `==`).

## Functions

| fn | meaning |
|----|---------|
| `abs sqrt sin cos tan exp ln floor ceil` | row-wise, as expected |
| `pow(x,y)` `min(x,y)` `max(x,y)` | two-arg row-wise |
| `choose(cond, if_true, if_false)` | gating, e.g. `choose('Brake Pressure Front' > 5, 1, 0)` |
| `smooth(series, n)` | trailing moving average over n **output** samples (causal) |

## Try these on your log (paste into `=` box)

- Combined G: `sqrt('G Force Lat DAQ'^2 + 'G Force Long DAQ'^2)`
- Throttle 0–1: `'TPS0Percent0FF' / 100`
- Smoothed steering: `smooth('Steering Wheel Angle', 20)`
- Braking flag: `choose('Brake Pressure Front' > 5, 1, 0)`
- Speed sanity: `'GPS Speed'` vs wheel speeds (overlay both)

## GUI steps

1. Load the `.ld`. 2. Open **Maths channels**. 3. Fill name / `=` expression /
   unit / Hz (blank = auto). 4. **Add** — errors name the problem (unknown
   channel + full list, bad syntax with position). 5. Tick its checkbox to plot
   (prefixed `MATH`), click it for min/max/rate in the inspector.

## Headless (for scripts/tests)

`motec-gui --eval <file.ld> '<expr>' [rate_hz]` prints rate, n, min/max,
first 10 rows. Exit 1 + channel list on unknown names.

## Limits in B3 (honest)

Linear interpolation for every input (no step/hold mode for flags yet);
`smooth` window counts output samples, not seconds; `x/0` gives inf, not an
error; no `integrate`/`derivative` yet; no saved user constants yet
(engine supports them, UI pending); maths live in-session only (file
import/export = B5).

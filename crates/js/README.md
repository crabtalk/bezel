# bezel-js

What JavaScript does with numbers, strings, regular expressions and dates, where Rust's own answer differs, and ports of d3-format, d3-time and d3-time-format. No JavaScript engine and no GUI toolkit; builds for `wasm32-unknown-unknown`.

| Module | What it holds |
| --- | --- |
| `number` | `Number`: `String(x)` as `Display`, `toFixed`, `toExponential`, `toPrecision`, `Number(text)` as `From<&str>`; `parseFloat`, `parseInt`, `ToInt32`, `ToIntegerOrInfinity`, truth |
| `math` | `Math.round`, `Math.sign`, `Math.hypot`, `Math.max`, `Math.min`, `Math.pow` |
| `string` | `\s`, `\w` and line terminators, `trim`, lengths, offsets and order in UTF-16 units, the default sort, `replace` |
| `regex` | `Regex`: JavaScript's syntax and matching rules, `new RegExp`'s errors, `replace` with `$` patterns, and `Lazy` for a `static` |
| `date` | `Date` and `LocalZone`: time values, fields in the local zone or UTC, `Date.UTC`, `Date.now`, V8's `Date.parse` |
| `time` | d3-time intervals and d3-scale's time ticks, in a `LocalZone` |
| `format` | d3-format's `format`, `formatPrefix`, `formatSpecifier` and precisions, en-US |
| `time_format` | d3-time-format's formatting and parsing, en-US, in local time and UTC |

## Tests

```sh
cargo nextest run -p bezel-js
```

`tests/format.rs` and `tests/time_format.rs` check every case in `tests/d3/*.json`, written by the pinned d3-format and d3-time-format with local time in America/New_York. To regenerate them:

```sh
cd tests/d3 && npm install && npm run fixtures
```

// Runs number and time specifiers through the pinned d3-format and
// d3-time-format, point runs through d3-shape's curves and sectors through its
// arc, values through d3-array's ticks, d3-scale's scales, d3-interpolate and
// vega-scale's schemes and band scales, and writes each case and its output to
// `format.json`, `time_format.json`, `shape.json` and `scale.json` beside this
// file. Local time is America/New_York.
//
//   npm install && npm run fixtures
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import {
  format,
  formatPrefix,
  formatSpecifier,
  precisionFixed,
  precisionPrefix,
  precisionRound,
} from 'd3-format';
import { timeFormat, timeParse, utcFormat, utcParse } from 'd3-time-format';
import * as shape from 'd3-shape';
import * as array from 'd3-array';
import * as d3time from 'd3-time';
import * as d3 from 'd3-scale';
import * as interpolate from 'd3-interpolate';
import { scale as vegaScale, scheme, interpolateRange, quantizeInterpolator } from 'vega-scale';
import { continuous as rampPalettes, discrete as listPalettes } from './node_modules/vega-scale/src/palettes.js';

if (process.env.TZ !== 'America/New_York') throw new Error('run with TZ=America/New_York');

const dir = import.meta.dirname;

// A number as text `Number(text)` reads back exactly, `-0` included.
const text = (x) => (Object.is(x, -0) ? '-0' : String(x));

const values = [
  0, -0, 1, -1, 0.5, 2.5, -2.5, 0.1 + 0.2, -2 / 3, 0.001234, 1e-7, 4.2e-30, 123.456, -123.456,
  999.9999, 12345, 1234567.891, 2 ** 53 + 2, 1e21, 1.2345678901234567e25, 5e-324, NaN, Infinity,
  -Infinity,
];

const types = ['', 'e', 'f', 'g', 'r', 's', '%', 'p', 'b', 'o', 'd', 'x', 'X', 'c', 'n', 'E', 'y'];
const specifiers = [];
for (const type of types) {
  for (const precision of ['', '.0', '.1', '.3', '.21']) {
    for (const trim of ['', '~']) specifiers.push(precision + trim + type);
  }
  for (const flags of [',', '+', '(', ' ', '$', '#', '010,', '^12', '=+12', '*<12,', '(,.2']) {
    specifiers.push(flags + type);
  }
}
specifiers.push('$,.2f', '+$,.2f', '($,.2f', '020,d', '020,.3f', '_^9d', '#b', '#o', '#X', '.2~s', ',.0%', '+.1%', ' 8.3e', '.99f', '.6e', '>12', '0=12', '08,', '-^+15,.2f', '1,d', '2,d', '3,d', '4,d', '05,d', '06,d');

const number = [];
for (const specifier of specifiers) {
  const f = format(specifier);
  for (const x of values) number.push([specifier, text(x), f(x)]);
}

const prefix = [];
for (const specifier of [',.0', '.2', '.3~', '$,.2', '+08.1', ',.0s', '.0e', '']) {
  for (const value of [1e-30, 1e-6, 0.00042, 1, 1300, 1e6, 2.5e9, 1e30, 0, -1e6]) {
    const f = formatPrefix(specifier, value);
    for (const x of [0, 0.00042, 1.5, 1300, -42000, 1234567, 5e30, NaN]) {
      prefix.push([specifier, text(value), text(x), f(x)]);
    }
  }
}

const steps = [0, 1, -1, 0.1, 0.25, 0.001, 0.0015, 1e-9, 1e3, 1e5, 123, 2.5e-7, NaN, Infinity];
const maxima = [0, 1, 1.01, 1e3, 1e6, 1e-3, -1e6, NaN];
const num = (x) => (Number.isFinite(x) ? x : null);
const precision = [];
for (const step of steps) {
  precision.push(['fixed', text(step), null, num(precisionFixed(step))]);
  for (const value of maxima) {
    precision.push(['prefix', text(step), text(value), num(precisionPrefix(step, value))]);
    precision.push(['round', text(step), text(value), num(precisionRound(step, value))]);
  }
}

const specifier = [];
for (const s of [...specifiers, '', 'xx', '.f', '01', '0', '<', '><', '0<', ' <', ':^', '00', '=', '$$', '.2.2f', '1.e', '%', '~', 'Z']) {
  let out;
  try {
    out = formatSpecifier(s).toString();
  } catch {
    out = null;
  }
  specifier.push([s, out]);
}

writeFileSync(
  join(dir, 'format.json'),
  JSON.stringify({ number, prefix, precision, specifier }, null, 0).replaceAll('],[', '],\n[') + '\n',
);

// Instants on calendar and ISO-week boundaries, either side of New York's
// transitions, early and late years, and an invalid date.
const utc = (...fields) => Date.UTC(...fields);
const year = (y, ...rest) => new Date(utc(2000, ...rest)).setUTCFullYear(y);
const instants = [
  0, -1, 1, utc(2023, 0, 15, 9, 5, 7, 89), utc(2023, 11, 31, 23, 59, 59, 999), utc(2024, 1, 29, 12),
  utc(2004, 11, 31), utc(2005, 0, 1, 12), utc(2005, 0, 2, 12), utc(2008, 11, 29, 12), utc(2009, 11, 31, 12),
  utc(2010, 0, 3, 12), utc(2010, 0, 4, 12), utc(2012, 0, 1, 12), utc(2015, 0, 1, 4), utc(2016, 0, 1, 5),
  utc(2020, 11, 31, 12), utc(2021, 0, 3, 12), utc(2026, 0, 1, 12),
  utc(2023, 2, 12, 6, 59), utc(2023, 2, 12, 7), utc(2023, 10, 5, 5, 30), utc(2023, 10, 5, 6, 30),
  utc(2023, 5, 15, 0, 0), utc(2023, 5, 15, 12, 0), utc(2023, 5, 15, 13, 0), utc(1969, 11, 31, 23, 59, 59),
  year(5, 5, 6, 7), year(50, 0, 1, 12), year(99, 11, 31, 12), year(-44, 2, 15), year(12345, 6, 7),
  year(1900, 0, 1, 12), year(1950, 6, 1), utc(2038, 0, 19, 3, 14, 7), NaN,
];

const directives = 'aAbBcdefgGHIjLmMpqQsSuUVwWxXyYZ%';
const formats = [];
for (const c of directives) {
  formats.push('%' + c);
  if ('cdefgGHIjLmMSUVWyY'.includes(c)) for (const p of '-_0') formats.push('%' + p + c);
}
formats.push(
  '%Y-%m-%d', '%b %d', '%Y-%m', '%Y-%m-%dT%H:%M:%S.%LZ', '%A, %B %e, %Y', '%I:%M %p', '%H:%M:%S',
  '%a %d %b %Y %H:%M:%S %Z', '%% %y %q %k', 'plain', '%', 'trailing %', '%-', '50%%', '%é',
);

const time = [];
for (const specifier of formats) {
  const local = timeFormat(specifier);
  const universal = utcFormat(specifier);
  for (const t of instants) {
    const input = Number.isNaN(t) ? null : t;
    time.push([specifier, input, local(new Date(t)), universal(new Date(t))]);
  }
}

const parses = [
  ['%Y-%m-%d', ['2023-01-15', '2023-1-5', '2023-02-30', '0050-01-01', '50-01-01', '2023-01-15x', '2023-01', '', '12023-01-01', ' 2023-01-15', '2023- 1- 5']],
  ['%Y-%m-%dT%H:%M:%S%Z', ['2023-03-12T02:30:00-0500', '2023-03-12T02:30:00Z', '2023-03-12T02:30:00+05:30', '2023-03-12T02:30:00+05', '2023-03-12T02:30:00+0', '2023-03-12T02:30:00x-05']],
  ['%Y-%m-%dT%H:%M:%S.%LZ', ['2023-01-15T09:05:07.089Z', '2023-01-15T09:05:07.89Z']],
  ['%Y-%m-%d %H:%M', ['2023-03-12 02:30', '2023-11-05 01:30', '2023-11-05 00:59', '2023-06-15 24:00', '2023-06-15 23:61']],
  ['%m/%d/%Y', ['1/2/2023', '01/02/2023', '13/40/2023']],
  ['%m/%d/%y', ['1/2/68', '1/2/69', '1/2/00', '1/2/2']],
  ['%b %d', ['Jan 05', 'jan 5', 'JAN 31', 'Sept 1', 'Sep 1']],
  ['%B %d, %Y', ['January 5, 2023', 'september 9, 1999', 'Janu 5, 2023']],
  ['%a %b %d %Y', ['Sun Jan 15 2023', 'Mon Jan 15 2023', 'sun jan 15 2023']],
  ['%A, %B %e, %Y', ['Sunday, January 15, 2023', 'Sunday, January  5, 2023']],
  ['%I:%M %p', ['12:30 am', '12:30 PM', '1:05 pm', '11:59 AM', '13:00 PM']],
  ['%H%M%S', ['093007', '9307']],
  ['%Q', ['1673773507089', '0', '-5', '86400000000000000']],
  ['%s', ['1673773507', '0']],
  ['%s.%L', ['1673773507.089']],
  ['%Y %j', ['2023 1', '2023 060', '2024 060', '2023 400']],
  ['%Y %U %w', ['2023 00 0', '2023 02 3', '2024 52 6']],
  ['%Y %W %u', ['2023 00 1', '2023 02 3', '2023 01 7']],
  ['%Y %U %a', ['2023 10 Fri']],
  ['%Y %U', ['2023 10']],
  ['%Y %W', ['2023 10']],
  ['%G %V', ['2023 1', '2020 53', '2021 53', '2023 0', '2023 54']],
  ['%G-W%V-%u', ['2009-W53-7', '2015-W01-1']],
  ['%Y %V %a', ['2004 53 Fri', '2010 1 Mon']],
  ['%Y Q%q', ['2023 Q1', '2023 Q3', '2023 Q5']],
  ['%Y Q%q %m', ['2023 Q3 01']],
  ['%Y-%m-%d %H:%M:%S.%f', ['2023-01-15 09:05:07.089123', '2023-01-15 09:05:07.5']],
  ['%c', ['1/15/2023, 9:05:07 AM', '1/15/2023, 9:05:07 PM']],
  ['%x', ['1/15/2023', '01/15/2023']],
  ['%X', ['9:05:07 AM', '12:00:00 AM']],
  ['%y-%m-%d %%', ['23-01-15 %', '23-01-15 x']],
  ['%-d/%_m/%0Y', ['5/ 1/2023', '5/1/2023']],
  ['%e', ['5', ' 5']],
  ['%Y%', ['2023%']],
  ['%Y %k', ['2023 x']],
  ['%Y %p', ['2023 PM']],
  ['%Y-%m-%d', [' 2023-01-15', '2023- 1-5']],
];

const parse = [];
for (const [specifier, inputs] of parses) {
  const local = timeParse(specifier);
  const universal = utcParse(specifier);
  const out = (d) => (d === null ? null : Number.isNaN(+d) ? 'NaN' : +d);
  for (const input of inputs) parse.push([specifier, input, out(local(input)), out(universal(input))]);
}

writeFileSync(
  join(dir, 'time_format.json'),
  JSON.stringify({ time, parse }, null, 0).replaceAll('],[', '],\n[') + '\n',
);

const runs = [
  [[0, 0]],
  [[0, 0], [10, 20]],
  [[0, 0], [10, 20], [30, 5]],
  [[0, 0], [10, 20], [30, 5], [45, 40], [60, 10], [61, 10], [80, -15]],
  [[0, 10], [5, 30], [5, 30], [20, 0], [20, 25], [40, 30]],
  [[3, 2], [1, 8], [6, 9], [12, 4], [9, -1]],
];
const curves = [
  ['Linear', shape.curveLinear], ['LinearClosed', shape.curveLinearClosed],
  ['Step', shape.curveStep], ['StepBefore', shape.curveStepBefore], ['StepAfter', shape.curveStepAfter],
  ['Basis', shape.curveBasis], ['BasisOpen', shape.curveBasisOpen], ['BasisClosed', shape.curveBasisClosed],
  ['Bundle', shape.curveBundle, 0.85], ['Bundle', shape.curveBundle.beta(0.3), 0.3],
  ['Cardinal', shape.curveCardinal, 0], ['Cardinal', shape.curveCardinal.tension(0.4), 0.4],
  ['CardinalOpen', shape.curveCardinalOpen.tension(0.2), 0.2], ['CardinalClosed', shape.curveCardinalClosed.tension(0.7), 0.7],
  ['CatmullRom', shape.curveCatmullRom, 0.5], ['CatmullRom', shape.curveCatmullRom.alpha(1), 1],
  ['CatmullRom', shape.curveCatmullRom.alpha(0), 0],
  ['CatmullRomOpen', shape.curveCatmullRomOpen, 0.5], ['CatmullRomClosed', shape.curveCatmullRomClosed, 0.5],
  ['MonotoneX', shape.curveMonotoneX], ['MonotoneY', shape.curveMonotoneY], ['Natural', shape.curveNatural],
];
const curve = [];
for (const [name, factory, parameter] of curves) {
  for (const run of runs) {
    const line = shape.line().curve(factory)(run);
    // Bundle draws lines alone.
    const area = name === 'Bundle' ? null : shape.area().x((p) => p[0]).y0((p) => p[1] - 7).y1((p) => p[1]).curve(factory)(run);
    curve.push([name, parameter ?? null, run, line, area]);
  }
}

// Each sector: inner and outer radius, start and end angle, pad angle,
// corner radius.
const sectors = [];
for (const [inner, outer] of [[0, 100], [40, 100], [100, 40], [0, 0], [99, 100]]) {
  for (const [start, end] of [[0, 1], [0.5, 2.5], [1, 0], [0, 3.5], [0, 2 * Math.PI], [2, 2.001], [-1, 5]]) {
    for (const pad of [0, 0.04, 0.5]) {
      for (const corner of [0, 4, 30, 200]) sectors.push([inner, outer, start, end, pad, corner]);
    }
  }
}
const arc = sectors.map(([inner, outer, start, end, pad, corner]) => [
  inner, outer, start, end, pad, corner,
  shape.arc().cornerRadius(corner)({ innerRadius: inner, outerRadius: outer, startAngle: start, endAngle: end, padAngle: pad }),
]);

writeFileSync(
  join(dir, 'shape.json'),
  JSON.stringify({ curve, arc }, null, 0)
    .replaceAll('],["', '],\n["')
    .replace(/(["l]\]),\[(-?\d)/g, '$1,\n[$2') + '\n',
);

// A number as JSON: NaN and the infinities as strings `Number` reads back.
const figure = (x) => (Number.isFinite(x) ? x : String(x));
const figures = (xs) => Array.from(xs, figure);

const spans = [
  [0, 1, 10], [0, 97, 5], [0, 0.3, 3], [0, 25000, 2], [-3.7, 42.1, 7], [1e-7, 3e-6, 5], [100, 0, 4],
  [0, 0, 5], [1, 1, 3], [0, 1, 0], [-1e9, 1e9, 9], [0.1, 0.4, 0.5], [3, 4, 1.5], [123.456, 789.012, 12],
  [-0.004, -0.0001, 6], [1e15, 1e16, 3], [NaN, 1, 5], [0, Infinity, 5],
];
const ticks = spans.map(([start, stop, count]) => [
  figure(start), figure(stop), count,
  figures(array.ticks(start, stop, count)),
  figure(array.tickIncrement(start, stop, count)),
  figure(array.tickStep(start, stop, count)),
  figures(array.nice(start, stop, count)),
]);

// Each continuous scale: its kind and parameter, domain, range, clamp and
// round; where each input lands and each output inverts to; ticks at each
// count and the domain nice at each.
const inputs = [-1000, -10, -1, -0.5, 0, 0.001, 0.5, 1, 2.5, 3, 7, 10, 42, 99.9, 100, 250, 1e4, NaN];
const counts = [1, 2, 5, 10, 17];
const configs = [
  ['linear', null, [0, 1], [0, 100]], ['linear', null, [0, 97], [400, 0]], ['linear', null, [10, -10], [0, 1]],
  ['linear', null, [0, 50, 100], [0, 10, 100]], ['linear', null, [100, 50, 0], [0, 10, 100]], ['linear', null, [3, 3], [0, 10]],
  ['linear', null, [0, 10, 20, 30], [0, 1]], ['linear', null, [-3.7, 42.1], [0, 300]], ['linear', null, [1e-7, 3e-6], [0, 1]],
  ['log', 10, [1, 1000], [0, 1]], ['log', 10, [1000, 1], [0, 300]], ['log', 10, [-1000, -1], [0, 1]], ['log', 10, [0.5, 70], [0, 1]],
  ['log', 2, [1, 100], [0, 1]], ['log', Math.E, [1, 100], [0, 1]], ['log', 3, [2, 500], [0, 1]], ['log', 10, [3, 3.5], [0, 1]],
  ['log', 10, [1, 1e9], [0, 1]], ['log', 10, [0.001, 0.2], [10, 0]],
  ['pow', 2, [0, 10], [0, 100]], ['pow', 0.5, [0, 100], [0, 1]], ['pow', 3, [-5, 5], [0, 1]], ['pow', 1, [0, 1], [0, 1]],
  ['pow', 0.33, [2, 9], [5, 0]], ['symlog', 1, [-100, 100], [0, 1]], ['symlog', 10, [0, 1e4], [0, 300]], ['symlog', 0.5, [-2, 7], [0, 1]],
];
const continuous = [];
for (const [kind, parameter, domain, range] of configs) {
  for (const [clamp, round] of [[false, false], [true, false], [false, true]]) {
    const make = () => {
      const s = kind === 'linear' ? d3.scaleLinear() : kind === 'log' ? d3.scaleLog().base(parameter)
        : kind === 'pow' ? d3.scalePow().exponent(parameter) : d3.scaleSymlog().constant(parameter);
      s.domain(domain).range(range).clamp(clamp);
      if (round) s.interpolate(interpolate.interpolateRound);
      return s;
    };
    const s = make();
    continuous.push({
      kind, parameter, domain, range, clamp, round,
      at: inputs.map((x) => [figure(x), figure(s(x))]),
      invert: round ? [] : range.concat([-5, 0.25, 50, 1000]).map((y) => [figure(y), figure(s.invert(y))]),
      ticks: counts.map((count) => [count, figures(s.ticks(count))]),
      nice: counts.map((count) => [count, figures(make().nice(count).domain())]),
    });
  }
}

const day = 864e5;
const times = [
  [Date.UTC(2024, 0, 10), Date.UTC(2024, 11, 20)], [Date.UTC(2020, 5, 1), Date.UTC(2024, 5, 1)],
  [Date.UTC(2024, 2, 5, 9, 10), Date.UTC(2024, 2, 5, 12, 10)], [Date.UTC(2023, 2, 11), Date.UTC(2023, 2, 14)],
  [Date.UTC(2023, 10, 4), Date.UTC(2023, 10, 6)], [0, 1500], [Date.UTC(1990, 0, 1), Date.UTC(2030, 0, 1)],
  [Date.UTC(2024, 0, 1), Date.UTC(2024, 0, 1) + 90 * day], [Date.UTC(2024, 0, 1), Date.UTC(2024, 0, 1) + 20 * day],
  [Date.UTC(2024, 4, 3, 1, 2, 3, 4), Date.UTC(2024, 4, 3, 1, 2, 9, 7)], [Date.UTC(1000, 0, 1), Date.UTC(2000, 0, 1)],
  [Date.UTC(2024, 11, 20), Date.UTC(2024, 0, 10)],
];
const calendar = [];
for (const utc of [true, false]) {
  for (const domain of times) {
    const make = () => (utc ? d3.scaleUtc() : d3.scaleTime()).domain(domain);
    calendar.push({
      utc, domain,
      ticks: counts.concat([40]).map((count) => [count, make().ticks(count).map(Number)]),
      nice: counts.map((count) => [count, make().nice(count).domain().map(Number)]),
      every: [['minute', 15], ['hour', 6], ['day', 1], ['day', 2], ['week', 1], ['week', 2], ['month', 1], ['month', 3], ['year', 1], ['year', 5]].map(([unit, step]) => {
        const name = (utc ? 'utc' : 'time') + (unit === 'week' ? 'Sunday' : unit[0].toUpperCase() + unit.slice(1));
        return [unit, step, make().nice(d3time[name].every(step)).domain().map(Number)];
      }),
    });
  }
}

const band = [];
for (const type of ['band', 'point']) {
  for (const count of [0, 1, 2, 5, 12]) {
    for (const range of [[0, 300], [300, 0], [17, 461.5]]) {
      for (const [inner, outer, align, round] of [[0, 0, 0.5, false], [0.1, 0.05, 0.5, false], [0.2, 0.5, 0, true], [1, 0.5, 1, false], [0.25, 0.125, 0.5, true], [-1, 2, 0.5, false]]) {
        const s = vegaScale(type)().domain(Array.from({ length: count }, (_, i) => i)).range(range).round(round).align(align);
        if (type === 'band') s.paddingInner(inner).paddingOuter(outer); else s.padding(outer);
        const probes = [-1, 0, 5, 37.5, 150, 299.9, 300, 461.5, 500];
        band.push({
          type, count, range, inner: type === 'band' ? inner : 1, outer, align, round,
          step: figure(s.step()), bandwidth: figure(s.bandwidth()),
          at: Array.from({ length: count }, (_, i) => figure(s(i))),
          invert: probes.map((p) => [p, s.invert(p) ?? null]),
        });
      }
    }
  }
}

const discretizing = [];
const probes = [-5, 0, 1, 2.5, 3, 10, 33, 50, 51, 70, 99, 100, 1e3, NaN];
for (const [values, count] of [[[1, 2, 3, 4, 5, 6, 7, 8, 9, 10], 4], [[3, 1, 4, 1, 5, 9, 2, 6, NaN, 5], 3], [[50], 5], [[], 3], [[0, 100], 1]]) {
  const s = d3.scaleQuantile().domain(values).range(Array.from({ length: count }, (_, i) => i));
  discretizing.push({ kind: 'quantile', values: figures(values), count, at: probes.map((x) => [figure(x), s(x) ?? null]) });
}
for (const [domain, count] of [[[0, 100], 4], [[30, 70], 5], [[10, 0], 3], [[0, 1], 1]]) {
  const s = d3.scaleQuantize().domain(domain).range(Array.from({ length: count }, (_, i) => i));
  discretizing.push({ kind: 'quantize', domain, count, at: probes.map((x) => [figure(x), s(x) ?? null]) });
}
for (const [domain, count] of [[[30, 70], 3], [[0, 50, 100], 4], [[0, 50, 100], 2], [[10], 5]]) {
  const s = d3.scaleThreshold().domain(domain).range(Array.from({ length: count }, (_, i) => i));
  discretizing.push({ kind: 'threshold', domain, count, at: probes.map((x) => [figure(x), s(x) ?? null]) });
}
for (const [bins, count] of [[[0, 10, 20, 30, 40], 4], [[0, 10, 20, 30, 40], 2], [[5, 50], 3]]) {
  const s = vegaScale('bin-ordinal')().domain(bins).range(Array.from({ length: count }, (_, i) => i));
  discretizing.push({ kind: 'bin-ordinal', bins, count, at: probes.map((x) => [figure(x), s(x) ?? null]) });
}

const sequential = [];
for (const [kind, parameter, domain] of [
  ['linear', null, [0, 100]], ['linear', null, [100, 0]], ['linear', null, [5, 5]], ['log', 10, [1, 1000]], ['log', 10, [-1000, -1]],
  ['pow', 2, [0, 10]], ['sqrt', null, [0, 100]], ['symlog', 1, [-100, 100]],
  ['linear', null, [0, 30, 100]], ['linear', null, [100, 30, 0]], ['linear', null, [0, 0, 100]], ['log', 10, [1, 10, 1000]],
  ['pow', 2, [-10, 0, 10]], ['symlog', 1, [-100, 0, 1000]],
]) {
  for (const clamp of [false, true]) {
    const diverging = domain.length === 3;
    const name = (diverging ? 'scaleDiverging' : 'scaleSequential') + (kind === 'linear' ? '' : kind[0].toUpperCase() + kind.slice(1));
    const s = d3[name]((t) => t).domain(domain).clamp(clamp);
    if (kind === 'log') s.base(parameter);
    if (kind === 'pow') s.exponent(parameter);
    if (kind === 'symlog') s.constant(parameter);
    sequential.push({ kind, parameter, domain, clamp, at: inputs.map((x) => [figure(x), figure(s(x))]) });
  }
}

const colors = ['#cfe1f2', '#0a4a90', 'steelblue', 'brown', 'rgba(255, 0, 0, 0.25)', 'hsl(300, 50%, 20%)', 'white', 'black', '#808080', 'transparent'];
const methods = [['rgb', 1], ['rgb', 2.2], ['rgb', 0.5], ['hsl', null], ['hsl-long', null], ['lab', null], ['hcl', null], ['hcl-long', null], ['cubehelix', 1], ['cubehelix', 3], ['cubehelix-long', 1], ['cubehelix-long', 0.5]];
const ts = [0, 0.1, 0.25, 0.5, 0.75, 0.9, 1, -0.2, 1.3];
const between = [];
for (const [method, gamma] of methods) {
  const name = 'interpolate' + method.split('-').map((s) => s[0].toUpperCase() + s.slice(1)).join('');
  const f = gamma == null ? interpolate[name] : interpolate[name].gamma(gamma);
  for (let i = 0; i < colors.length; i++) {
    for (const j of [(i + 1) % colors.length, (i + 4) % colors.length]) {
      const g = f(colors[i], colors[j]);
      between.push([method, gamma ?? 1, colors[i], colors[j], ts.map((t) => g(t))]);
    }
  }
  const p = interpolate.piecewise(f, ['#440154', 'white', '#21918c', 'orange']);
  between.push([method, gamma ?? 1, 'piecewise', null, ts.map((t) => p(t))]);
}

const schemes = [];
for (const name of [...Object.keys(listPalettes), ...Object.keys(rampPalettes)].sort()) {
  const s = scheme(name);
  if (typeof s !== 'function') {
    schemes.push({ name, colors: s, samples: [] });
    continue;
  }
  const samples = [];
  for (const [extent, reverse] of [[null, false], [null, true], [[0.2, 0.9], false], [[1, 0], false], [[0.25, 0.75], true]]) {
    const f = extent || reverse ? interpolateRange(s, reverse ? (extent || [0, 1]).slice().reverse() : extent) : s;
    samples.push([extent, reverse, ts.map((t) => f(t))]);
  }
  const quantized = [1, 3, 5, 9].map((count) => [count, quantizeInterpolator(s, count), quantizeInterpolator(interpolateRange(s, [0.2, 0.8]), count)]);
  schemes.push({ name, colors: null, samples, quantized });
}

writeFileSync(
  join(dir, 'scale.json'),
  JSON.stringify({ ticks, continuous, time: calendar, band, discretizing, sequential, between, schemes }, null, 0)
    .replaceAll('],[', '],\n[').replaceAll('},{', '},\n{') + '\n',
);

console.log({ ticks: ticks.length, continuous: continuous.length, time: calendar.length, band: band.length, discretizing: discretizing.length, sequential: sequential.length, between: between.length, schemes: schemes.length });
console.log({ curve: curve.length, arc: arc.length, number: number.length, prefix: prefix.length, precision: precision.length, specifier: specifier.length, time: calendar.length, parse: parse.length });

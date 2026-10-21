// Runs number and time specifiers through the pinned d3-format and
// d3-time-format, point runs through d3-shape's curves and sectors through its
// arc, and writes each
// case and d3's output to `format.json`, `time_format.json` and `shape.json`
// beside this file. Local time is America/New_York.
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

console.log({ curve: curve.length, arc: arc.length, number: number.length, prefix: prefix.length, precision: precision.length, specifier: specifier.length, time: time.length, parse: parse.length });

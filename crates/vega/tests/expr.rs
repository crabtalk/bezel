use d3::time::{date, date::LocalZone};
use std::cell::RefCell;
use vega::{
    expr::{Context, Error, Expr},
    value::Value,
};

const SECOND: f64 = 1000.0;
const MINUTE: f64 = 60.0 * SECOND;
const HOUR: f64 = 60.0 * MINUTE;
const DAY: f64 = 24.0 * HOUR;

fn object(fields: &[(&str, Value)]) -> Value {
    Value::Object(
        fields
            .iter()
            .map(|(key, value)| ((*key).into(), value.clone()))
            .collect(),
    )
}

/// Two rows: `x` 1 and `null`, `name` "Ann" and "Bob".
fn rows() -> [Value; 2] {
    [
        object(&[("x", 1.0.into()), ("name", "Ann".into())]),
        object(&[("x", Value::Null), ("name", "Bob".into())]),
    ]
}

/// `source` on `datum`, a bare name read from `params`.
fn eval_on(source: &str, datum: &Value, params: &[(&str, Value)]) -> Result<Value, Error> {
    let param = |name: &str| {
        params
            .iter()
            .find(|(param, _)| *param == name)
            .map(|(_, value)| value.clone())
            .unwrap_or_default()
    };
    let data = |name: &str| match name {
        "rows" => rows().to_vec(),
        _ => Vec::new(),
    };
    let context = Context {
        datum,
        param: &param,
        scale: &|_| None,
        data: &data,
        random: &RefCell::default(),
    };
    source.parse::<Expr>()?.eval(&context)
}

fn try_eval(source: &str, index: usize, params: &[(&str, Value)]) -> Result<Value, Error> {
    eval_on(source, &rows()[index], params)
}

fn eval(source: &str) -> Value {
    try_eval(source, 0, &[]).unwrap_or_else(|error| panic!("{source}: {error}"))
}

fn show(source: &str) -> String {
    eval(source).to_string()
}

fn num(source: &str) -> f64 {
    match eval(source) {
        Value::Number(n) => n,
        other => panic!("{source} is {other:?}, not a number"),
    }
}

fn yes(source: &str) -> bool {
    match eval(source) {
        Value::Bool(b) => b,
        other => panic!("{source} is {other:?}, not a boolean"),
    }
}

fn utc(year: i64, month: i64, day: i64) -> f64 {
    date::days_from_civil(year, month, day) as f64 * DAY
}

/// `new Date(year, month, day, hours, minutes)` in the host's zone, month
/// from 0.
fn local(year: i32, month: i32, day: i32, hours: i32, minutes: i32) -> f64 {
    let [y, m, d, h, min] = [year, month, day, hours, minutes].map(f64::from);
    LocalZone::default().local(y, m, d, h, min, 0.0, 0.0).time()
}

#[test]
fn operators_bind_by_javascript_precedence() {
    assert_eq!(num("1 + 2 * 3"), 7.0);
    assert_eq!(num("(1 + 2) * 3"), 9.0);
    assert_eq!(num("2 - 3 - 4"), -5.0);
    assert_eq!(num("5 % 3 * 2"), 4.0);
    assert_eq!(num("-2 * 3"), -6.0);
    assert_eq!(num("- -2"), 2.0);
    assert_eq!(num("!0 + 1"), 2.0);
    assert_eq!(num("8 >> 1 + 1"), 2.0);
    assert_eq!(num("1 | 2 & 3"), 3.0);
    assert_eq!(num("6 ^ 3 & 1"), 7.0);
    assert!(yes("1 < 2 == true"));
    assert!(yes("1 + 1 === 2 && 3 > 2"));
    assert!(yes("true || false && false"));
    assert_eq!(num("1 ? 2 : 3 ? 4 : 5"), 2.0);
    assert_eq!(num("0 ? 1 : 0 ? 2 : 3"), 3.0);
    assert_eq!(num("1 || 0 ? 7 : 8"), 7.0);
    assert_eq!(show("1 + 2 + '3'"), "33");
    assert_eq!(show("'1' + 2 + 3"), "123");
}

#[test]
fn bitwise_operators_work_on_32_bit_integers() {
    assert_eq!(num("~5"), -6.0);
    assert_eq!(num("-1 >>> 28"), 15.0);
    assert_eq!(num("-16 >> 2"), -4.0);
    assert_eq!(num("1 << 31"), -2147483648.0);
    assert_eq!(num("1 << 32"), 1.0);
    assert_eq!(num("4294967297 | 0"), 1.0);
    assert_eq!(num("NaN | 0"), 0.0);
    assert_eq!(num("5.7 & 7"), 5.0);
}

#[test]
fn arithmetic_coerces_like_javascript() {
    assert_eq!(num("'5' * '2'"), 10.0);
    assert_eq!(num("'5' - 2"), 3.0);
    assert_eq!(num("true + true"), 2.0);
    assert_eq!(num("null + 1"), 1.0);
    assert!(num("undefined + 1").is_nan());
    assert_eq!(show("[1, 2] + ''"), "1,2");
    assert_eq!(show("[1, [2, null]] + 1"), "1,2,1");
    assert_eq!(show("{} + ''"), "[object Object]");
    assert_eq!(show("1 / 0"), "Infinity");
    assert_eq!(show("-1 / 0"), "-Infinity");
    assert_eq!(num("-7 % 3"), -1.0);
    assert_eq!(num("+''"), 0.0);
    assert_eq!(num("+' 12 '"), 12.0);
    assert_eq!(num("+'0x1f'"), 31.0);
    assert_eq!(num("+'1e3'"), 1000.0);
    assert_eq!(num("+'-Infinity'"), f64::NEG_INFINITY);
    assert_eq!(num("+'.5'"), 0.5);
    assert!(num("+'abc'").is_nan());
    assert!(num("+'inf'").is_nan());
    assert!(num("+'1 2'").is_nan());
    assert_eq!(num("+[]"), 0.0);
    assert_eq!(num("+[7]"), 7.0);
    assert!(num("+[1, 2]").is_nan());
}

#[test]
fn numbers_print_as_javascript_prints_them() {
    assert_eq!(show("0.1 + 0.2"), "0.30000000000000004");
    assert_eq!(show("1e21"), "1e+21");
    assert_eq!(show("1e20"), "100000000000000000000");
    assert_eq!(show("123e-20"), "1.23e-18");
    assert_eq!(show("0.000001"), "0.000001");
    assert_eq!(show("1e-7"), "1e-7");
    assert_eq!(show("-1.5"), "-1.5");
    assert_eq!(show("-0"), "0");
    assert_eq!(show("2 / 3"), "0.6666666666666666");
    assert_eq!(show("NaN"), "NaN");
    assert_eq!(show("MAX_VALUE"), "1.7976931348623157e+308");
    assert_eq!(show("MIN_VALUE"), "5e-324");
}

#[test]
fn equality_is_loose_or_strict() {
    assert!(yes("1 == '1'"));
    assert!(yes("0 == ''"));
    assert!(yes("null == undefined"));
    assert!(!yes("null == 0"));
    assert!(!yes("NaN == NaN"));
    assert!(yes("[1] == 1"));
    assert!(yes("true == '1'"));
    assert!(!yes("true == 'true'"));
    assert!(yes("'a' == ['a']"));
    assert!(!yes("[1] == [1]"));
    assert!(!yes("1 === '1'"));
    assert!(yes("'a' === 'a'"));
    assert!(yes("1 !== '1'"));
    assert!(yes("1 != 2"));
    assert!(!yes("null === undefined"));
}

#[test]
fn comparison_orders_strings_or_numbers() {
    assert!(yes("'b' > 'a'"));
    assert!(yes("'10' < '9'"));
    assert!(!yes("'10' < 9"));
    assert!(yes("null >= 0"));
    assert!(!yes("undefined < 1"));
    assert!(!yes("NaN <= NaN"));
    assert!(yes("'B' < 'a'"));
    assert!(yes("[2] > 1"));
}

#[test]
fn logic_returns_an_operand() {
    assert_eq!(show("'' || 'x'"), "x");
    assert_eq!(show("'0' && 'y'"), "y");
    assert_eq!(eval("0 || null"), Value::Null);
    assert_eq!(num("0 && null.x"), 0.0);
    assert_eq!(num("1 || null.x"), 1.0);
    assert_eq!(num("[] ? 1 : 2"), 1.0);
    assert_eq!(num("NaN ? 1 : 2"), 2.0);
    assert!(yes("!''"));
    assert!(!yes("!{}"));
}

#[test]
fn literals() {
    assert_eq!(num("0xff"), 255.0);
    assert_eq!(num("0o17"), 15.0);
    assert_eq!(num("0b101"), 5.0);
    assert_eq!(num("017"), 15.0);
    assert_eq!(num(".5"), 0.5);
    assert_eq!(num("1.5e3"), 1500.0);
    assert_eq!(num("2E-1"), 0.2);
    assert_eq!(show(r"'a\nb'"), "a\nb");
    assert_eq!(show(r#""A\x42\t""#), "AB\t");
    assert_eq!(show(r#""it's""#), "it's");
    assert_eq!(show(r"'\'q\''"), "'q'");
    assert_eq!(num(r#"length("\u{1F600}")"#), 2.0);
    assert_eq!(num(r#"length("😀")"#), 2.0);
    assert_eq!(show(r#""😀""#), "😀");
    assert_eq!(eval("null"), Value::Null);
    assert_eq!(eval("undefined"), Value::Undefined);
    assert_eq!(show("[1, 'a', true, null,]"), "1,a,true,");
    assert_eq!(num("{a: 1, 'b c': 2, 3: 4}['b c']"), 2.0);
    assert_eq!(num("{a: 1, a: 2}.a"), 2.0);
    assert_eq!(num("{1.5: 3}['1.5']"), 3.0);
    assert_eq!(show("/a+/gi"), "/a+/gi");
    assert_eq!(show("4 / 2 / 1"), "2");
}

#[test]
fn constants() {
    assert_eq!(num("PI"), std::f64::consts::PI);
    assert_eq!(num("E"), std::f64::consts::E);
    assert_eq!(num("LN2"), std::f64::consts::LN_2);
    assert_eq!(num("LN10"), std::f64::consts::LN_10);
    assert_eq!(num("LOG2E"), std::f64::consts::LOG2_E);
    assert_eq!(num("LOG10E"), std::f64::consts::LOG10_E);
    assert_eq!(num("SQRT2"), std::f64::consts::SQRT_2);
    assert_eq!(num("SQRT1_2"), std::f64::consts::FRAC_1_SQRT_2);
    assert_eq!(num("MAX_VALUE"), f64::MAX);
    assert!(num("NaN").is_nan());
}

#[test]
fn datum_reads_arrays_and_objects_in_values() {
    let info = Value::Object(
        [
            ("size".into(), Value::Number(3.0)),
            ("tags".into(), vec!["p".into(), "q".into()].into()),
        ]
        .into(),
    );
    let rows = [
        object(&[("info", info.clone()), ("x", 1.0.into())]),
        object(&[("info", Value::Null), ("x", 2.0.into())]),
    ];
    let eval = |source: &str, index: usize| eval_on(source, &rows[index], &[]);
    assert_eq!(eval("datum.info", 0), Ok(info.clone()));
    assert_eq!(eval("datum.info.tags[1]", 0), Ok("q".into()));
    assert_eq!(eval("isArray(datum.info.tags)", 0), Ok(true.into()));
    assert_eq!(eval("datum.info", 1), Ok(Value::Null));
    assert!(
        eval("datum.info.size", 1).is_err(),
        "null has no properties"
    );
    assert_eq!(eval("datum", 0).unwrap().get(&"info".into()), Ok(info));
    assert_eq!(
        eval("[datum.x, {a: datum.x}]", 1).unwrap().to_string(),
        "2,[object Object]"
    );
}

#[test]
fn datum_reads_the_row() {
    assert_eq!(num("datum.x * 2"), 2.0);
    assert_eq!(show("datum['name']"), "Ann");
    assert_eq!(show("datum['na' + 'me']"), "Ann");
    assert_eq!(eval("datum.missing"), Value::Undefined);
    assert_eq!(num("datum.name.length"), 3.0);
    assert_eq!(show("datum.name[1]"), "n");
    assert_eq!(eval("datum.name.nothing"), Value::Undefined);
    assert_eq!(num("datum.x > 0 && datum.name != 'Bob' ? 1 : 0"), 1.0);
    assert_eq!(try_eval("datum.x", 1, &[]), Ok(Value::Null));
    assert_eq!(try_eval("datum.name", 1, &[]), Ok("Bob".into()));
    assert_eq!(try_eval("isValid(datum.x)", 1, &[]), Ok(false.into()));
    assert_eq!(show("datum"), "[object Object]");
    assert_eq!(num("datum['x'] + datum.x"), 2.0);
}

#[test]
fn bare_names_are_params() {
    let params = [("width", Value::Number(100.0)), ("label", "w".into())];
    assert_eq!(try_eval("width / 2", 0, &params), Ok(Value::Number(50.0)));
    assert_eq!(try_eval("label + width", 0, &params), Ok("w100".into()));
    assert_eq!(try_eval("height", 0, &params), Ok(Value::Undefined));
}

#[test]
fn members_of_values() {
    assert_eq!(num("[1, 2, 3][1]"), 2.0);
    assert_eq!(num("[1, 2, 3]['2']"), 3.0);
    assert_eq!(num("[1, 2].length"), 2.0);
    assert_eq!(eval("[1, 2][5]"), Value::Undefined);
    assert_eq!(eval("[1, 2]['01']"), Value::Undefined);
    assert_eq!(num("{a: {b: 5}}.a.b"), 5.0);
    assert_eq!(show("'abc'[1]"), "b");
    assert_eq!(eval("(5).length"), Value::Undefined);
    assert_eq!(show("/x/g.source"), "x");
    assert!(matches!(try_eval("null.x", 0, &[]), Err(Error::Runtime(_))));
    assert!(matches!(
        try_eval("datum.missing.x", 0, &[]),
        Err(Error::Runtime(_))
    ));
}

#[test]
fn parse_errors() {
    let syntax = |source: &str| {
        assert!(
            matches!(source.parse::<Expr>(), Err(Error::Syntax { .. })),
            "{source} parsed"
        );
    };
    syntax("1 +");
    syntax("");
    syntax("1 2");
    syntax("a = 1");
    syntax("a += 1");
    syntax("x++");
    syntax("--x");
    syntax("this");
    syntax("new Date()");
    syntax("typeof x");
    syntax("'abc");
    syntax("(1");
    syntax("[1, 2");
    syntax("{a 1}");
    syntax("datum.x.y()");
    syntax("3in");
    syntax("1 @ 2");
    syntax("a.1");
    syntax("if(1, 2)");
    syntax("/(?<=a)b/");
    syntax("/abc");
    syntax("/a/q");
    assert_eq!(
        "foo(1)".parse::<Expr>().unwrap_err(),
        Error::Function("foo".to_string())
    );
    for name in ["vlSelectionTest", "modify"] {
        assert_eq!(
            format!("{name}(1)").parse::<Expr>().unwrap_err(),
            Error::Function(name.to_string())
        );
    }
    let Err(Error::Syntax { at, .. }) = "1 + * 2".parse::<Expr>() else {
        panic!()
    };
    assert_eq!(at, 4);
}

#[test]
fn type_checks() {
    assert!(yes("isArray([1])"));
    assert!(!yes("isArray('a')"));
    assert!(yes("isBoolean(false)"));
    assert!(!yes("isDate(now())"));
    assert!(yes("isDefined(null)"));
    assert!(!yes("isDefined(datum.missing)"));
    assert!(yes("isNumber(NaN)"));
    assert!(!yes("isNumber('1')"));
    assert!(yes("isObject({})"));
    assert!(yes("isObject([])"));
    assert!(!yes("isObject(null)"));
    assert!(yes("isRegExp(/a/)"));
    assert!(yes("isString('')"));
    assert!(!yes("isValid(null)"));
    assert!(!yes("isValid(undefined)"));
    assert!(!yes("isValid(NaN)"));
    assert!(yes("isValid(0)"));
    assert!(yes("isValid('')"));
}

#[test]
fn coercions() {
    assert!(!yes("toBoolean('false')"));
    assert!(!yes("toBoolean('0')"));
    assert!(!yes("toBoolean(0)"));
    assert!(yes("toBoolean('x')"));
    assert!(yes("toBoolean(1)"));
    assert_eq!(eval("toBoolean('')"), Value::Null);
    assert_eq!(eval("toBoolean(null)"), Value::Null);
    assert_eq!(eval("toNumber('')"), Value::Null);
    assert_eq!(num("toNumber('3.5')"), 3.5);
    assert_eq!(num("toNumber(true)"), 1.0);
    assert!(num("toNumber('x')").is_nan());
    assert_eq!(show("toString(12)"), "12");
    assert_eq!(show("toString([1, 2])"), "1,2");
    assert_eq!(eval("toString(undefined)"), Value::Null);
    assert_eq!(num("toDate('2020-01-01')"), utc(2020, 1, 1));
    assert_eq!(num("toDate(5)"), 5.0);
    assert!(num("toDate('nope')").is_nan());
    assert_eq!(eval("toDate(null)"), Value::Null);
}

#[test]
fn if_evaluates_one_branch() {
    assert_eq!(show("if(datum.x > 0, 'pos', 'neg')"), "pos");
    assert_eq!(num("if(true, 1, null.x)"), 1.0);
    assert_eq!(num("if('', null.x, 2)"), 2.0);
}

#[test]
fn math() {
    assert_eq!(num("abs(-2)"), 2.0);
    assert_eq!(num("ceil(1.2)"), 2.0);
    assert_eq!(num("floor(-1.2)"), -2.0);
    assert_eq!(num("round(2.5)"), 3.0);
    assert_eq!(num("round(-2.5)"), -2.0);
    assert_eq!(num("round(0.49999999999999994)"), 0.0);
    assert_eq!(num("round('3.7')"), 4.0);
    assert_eq!(num("clamp(5, 0, 3)"), 3.0);
    assert_eq!(num("clamp(-1, 0, 3)"), 0.0);
    assert_eq!(num("max(1, '5', 3)"), 5.0);
    assert_eq!(num("max()"), f64::NEG_INFINITY);
    assert_eq!(num("min()"), f64::INFINITY);
    assert_eq!(num("min(4, 2, 8)"), 2.0);
    assert!(num("min(1, NaN)").is_nan());
    assert!(num("max(1, 'x')").is_nan());
    assert_eq!(num("pow(2, 10)"), 1024.0);
    assert!(num("pow(1, NaN)").is_nan());
    assert!(num("pow(1, 1 / 0)").is_nan());
    assert_eq!(num("pow(NaN, 0)"), 1.0);
    assert_eq!(num("hypot(3, 4)"), 5.0);
    assert_eq!(num("hypot(NaN, 1 / 0)"), f64::INFINITY);
    assert_eq!(num("atan2(1, 1)"), std::f64::consts::FRAC_PI_4);
    assert_eq!(num("log(E)"), 1.0);
    assert_eq!(num("exp(0)"), 1.0);
    assert_eq!(num("sqrt(16)"), 4.0);
    assert_eq!(num("sin(0) + cos(0) + tan(0)"), 1.0);
    assert_eq!(num("asin(1)"), std::f64::consts::FRAC_PI_2);
    assert_eq!(num("acos(1) + atan(0)"), 0.0);
    assert!(yes("isNaN('abc')"));
    assert!(!yes("isNaN('12')"));
    assert!(yes("isFinite('12')"));
    assert!(!yes("isFinite(1 / 0)"));
    for _ in 0..100 {
        let r = num("random()");
        assert!((0.0..1.0).contains(&r));
    }
    // vega-statistics' `randomLCG(123456789)`, drawn in turn.
    assert_eq!(num("random()"), 0.1374794059141909);
    assert!(yes("random() != random()"));
}

#[test]
fn data_sets_and_distributions() {
    assert_eq!(num("length(data('rows'))"), 2.0);
    assert_eq!(show("data('rows')[1].name"), "Bob");
    assert_eq!(num("length(data('none'))"), 0.0);
    assert_eq!(num("indata('rows', 'name', 'Ann')"), 1.0);
    assert!(matches!(
        eval("indata('rows', 'name', 'Cy')"),
        Value::Undefined
    ));
    assert_eq!(num("quantileNormal(0.5)"), 0.0);
    assert_eq!(num("cumulativeNormal(0)"), 0.5);
    assert_eq!(num("quantileUniform(0.25, 0, 8)"), 2.0);
    assert_eq!(num("densityUniform(3, 10)"), 0.1);
    assert!(num("quantileUniform(2)").is_nan());
    let sample = num("sampleUniform(2, 4)");
    assert!((2.0..4.0).contains(&sample));
    assert_eq!(show("merge({a: 1}, {b: 2}, {a: 3}).a"), "3");
    assert_eq!(
        show("timeUnitSpecifier(['year', 'month', 'date'])"),
        "%Y-%m-%d"
    );
    assert_eq!(show("timeUnitSpecifier(['month', 'hours'])"), "%b %H:00");
}

#[test]
fn dates_compose_and_split_in_local_time_and_utc() {
    assert_eq!(num("datetime(2020, 0, 31)"), local(2020, 0, 31, 0, 0));
    assert_eq!(num("datetime(2020, 12, 1)"), local(2021, 0, 1, 0, 0));
    assert_eq!(num("datetime(2020, 1, 30)"), local(2020, 2, 1, 0, 0));
    assert_eq!(num("datetime(2020, 0, 0)"), local(2019, 11, 31, 0, 0));
    assert_eq!(num("datetime(99, 0)"), local(1999, 0, 1, 0, 0));
    assert_eq!(num("datetime(1000)"), 1000.0);
    assert_eq!(num("datetime('2020-02-03T04:05')"), local(2020, 1, 3, 4, 5));
    assert!(num("datetime(2020, 'x')").is_nan());
    assert_eq!(
        num("utc(2020, 0, 1, 13, 45, 30, 250)"),
        utc(2020, 1, 1) + 13.0 * HOUR + 45.0 * MINUTE + 30.0 * SECOND + 250.0
    );
    assert_eq!(num("utc(2020)"), utc(2020, 1, 1));
    assert_eq!(num("utc(2020, -1)"), utc(2019, 12, 1));
    assert!(num("utc()").is_nan());

    for (prefix, at) in [
        ("", "datetime(2020, 0, 31, 13, 45, 30, 250)"),
        ("utc", "utc(2020, 0, 31, 13, 45, 30, 250)"),
    ] {
        let get = |field: &str| num(&format!("{prefix}{field}({at})"));
        assert_eq!(get("year"), 2020.0, "{prefix}");
        assert_eq!(get("quarter"), 1.0, "{prefix}");
        assert_eq!(get("month"), 0.0, "{prefix}");
        assert_eq!(get("date"), 31.0, "{prefix}");
        assert_eq!(get("day"), 5.0, "{prefix}");
        assert_eq!(get("dayofyear"), 31.0, "{prefix}");
        assert_eq!(get("week"), 4.0, "{prefix}");
        assert_eq!(get("hours"), 13.0, "{prefix}");
        assert_eq!(get("minutes"), 45.0, "{prefix}");
        assert_eq!(get("seconds"), 30.0, "{prefix}");
        assert_eq!(get("milliseconds"), 250.0, "{prefix}");
    }
    assert_eq!(num("time('2020-01-01')"), utc(2020, 1, 1));
    assert_eq!(
        num("timezoneoffset(0)"),
        LocalZone::default().date(0.0).timezone_offset()
    );
    assert_eq!(num("dayofyear(datetime(2020, 2, 1))"), 61.0);
    assert_eq!(num("utcdayofyear('2020-03-01')"), 61.0);
    assert_eq!(num("utcquarter('2020-12-31')"), 4.0);
    assert_eq!(num("utcweek('2020-01-04')"), 0.0);
    assert_eq!(num("utcweek('2020-01-05')"), 1.0);
    assert_eq!(num("utcweek('2017-01-01')"), 1.0);
    assert_eq!(num("week(datetime(2017, 0, 1))"), 1.0);
    assert_eq!(num("utcday('1969-12-28')"), 0.0);
    assert_eq!(num("utcyear(-1)"), 1969.0);
    assert_eq!(num("utchours(-1)"), 23.0);
    assert!(num("year('nope')").is_nan());
    assert!(num("month(undefined)").is_nan());
    assert_eq!(num("utcyear(null)"), 1970.0);
    assert!(matches!(
        "utctime(0)".parse::<Expr>(),
        Err(Error::Function(_))
    ));
    let now = num("now()");
    let system = web_time_now();
    assert!((now - system).abs() < 60_000.0);
}

fn web_time_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64
}

#[test]
fn time_offsets_and_sequences_follow_d3_intervals() {
    assert_eq!(
        num("timeOffset('month', datetime(2021, 0, 31))"),
        local(2021, 2, 3, 0, 0)
    );
    assert_eq!(
        num("utcOffset('day', utc(2021, 0, 1), -1)"),
        utc(2020, 12, 31)
    );
    assert_eq!(
        num("timeOffset('quarter', datetime(2020, 1, 15))"),
        local(2020, 3, 15, 0, 0)
    );
    assert_eq!(
        num("timeOffset('quarter', datetime(2020, 3, 15), 2)"),
        local(2020, 9, 15, 0, 0)
    );
    assert_eq!(
        num("utcOffset('quarter', utc(2020, 3, 15), -1)"),
        utc(2020, 1, 15)
    );
    assert_eq!(
        num("timeOffset('year', datetime(2020, 1, 29))"),
        local(2021, 2, 1, 0, 0)
    );
    assert_eq!(
        num("timeOffset('week', datetime(2020, 0, 1), 2)"),
        local(2020, 0, 15, 0, 0)
    );
    assert_eq!(num("timeOffset('hours', 0, 1.9)"), HOUR);
    assert_eq!(num("timeOffset('minutes', 0, 2)"), 2.0 * MINUTE);
    assert_eq!(num("timeOffset('seconds', 0, 3)"), 3.0 * SECOND);
    assert_eq!(num("timeOffset('milliseconds', 0, 4)"), 4.0);
    assert!(matches!(
        try_eval("timeOffset('decade', 0)", 0, &[]),
        Err(Error::Runtime(_))
    ));

    let days = eval("timeSequence('day', datetime(2020, 0, 1), datetime(2020, 0, 4))");
    assert_eq!(
        days,
        Value::from(vec![
            local(2020, 0, 1, 0, 0).into(),
            local(2020, 0, 2, 0, 0).into(),
            local(2020, 0, 3, 0, 0).into()
        ])
    );
    let months = eval("utcSequence('month', utc(2020, 0, 15), utc(2020, 4, 1))");
    assert_eq!(
        months,
        Value::from(vec![
            utc(2020, 2, 1).into(),
            utc(2020, 3, 1).into(),
            utc(2020, 4, 1).into()
        ])
    );
    let weeks = eval("timeSequence('week', datetime(2020, 0, 1), datetime(2020, 0, 20))");
    assert_eq!(
        weeks,
        Value::from(vec![
            local(2020, 0, 5, 0, 0).into(),
            local(2020, 0, 12, 0, 0).into(),
            local(2020, 0, 19, 0, 0).into()
        ])
    );
    let quarters = eval("timeSequence('quarter', datetime(2020, 1, 1), datetime(2021, 0, 1))");
    assert_eq!(
        quarters,
        Value::from(vec![
            local(2020, 3, 1, 0, 0).into(),
            local(2020, 6, 1, 0, 0).into(),
            local(2020, 9, 1, 0, 0).into()
        ])
    );
    assert_eq!(
        num("length(utcSequence('hours', 0, utc(1970, 0, 2), 6))"),
        4.0
    );
    assert_eq!(num("length(timeSequence('day', 10, 0))"), 0.0);
    assert_eq!(num("length(timeSequence('day', 0, 1e9, 0))"), 0.0);
}

#[test]
fn arrays() {
    assert_eq!(show("extent([3, null, 1, NaN, 2])"), "1,3");
    assert_eq!(show("extent(['b', 'a'])"), "a,b");
    assert_eq!(
        eval("extent([])"),
        Value::from(vec![Value::Undefined, Value::Undefined])
    );
    assert_eq!(show("clampRange([8, 12], 0, 10)"), "6,10");
    assert_eq!(show("clampRange([12, 8], 0, 10)"), "6,10");
    assert_eq!(show("clampRange([-5, 20], 0, 10)"), "0,10");
    assert_eq!(show("clampRange([2, 3], 0, 10)"), "2,3");
    assert_eq!(num("indexof([1, 2, 3], 2)"), 1.0);
    assert_eq!(num("indexof([1, 2, 3], '2')"), -1.0);
    assert_eq!(num("indexof([NaN], NaN)"), -1.0);
    assert_eq!(num("lastindexof([1, 2, 1], 1)"), 2.0);
    assert!(yes("inrange(5, [10, 0])"));
    assert!(yes("inrange(10, [0, 5, 10])"));
    assert!(!yes("inrange(11, [0, 10])"));
    assert_eq!(show("join([1, null, 'a'], '-')"), "1--a");
    assert_eq!(show("join([1, 2])"), "1,2");
    assert_eq!(num("length([1, 2])"), 2.0);
    assert_eq!(num("lerp([0, 10], 0.25)"), 2.5);
    assert_eq!(num("lerp([0, 5, 10], 1)"), 10.0);
    assert_eq!(num("peek([1, 2, 3])"), 3.0);
    assert_eq!(eval("peek([])"), Value::Undefined);
    assert_eq!(show("pluck([{a: {b: 1}}, {a: {b: 2}}], 'a.b')"), "1,2");
    assert_eq!(show("pluck([{a: {b: 1}}], \"a['b']\")"), "1");
    assert_eq!(show("pluck([{'a.b': 3}], 'a\\\\.b')"), "3");
    assert_eq!(show("pluck([[4, 5]], '[1]')"), "5");
    assert_eq!(show("reverse([1, 2, 3])"), "3,2,1");
    assert_eq!(show("sequence(3)"), "0,1,2");
    assert_eq!(show("sequence(1, 2, 0.5)"), "1,1.5");
    assert_eq!(show("sequence(3, 0, -1)"), "3,2,1");
    assert_eq!(show("sequence(0, 1, 0)"), "");
    assert_eq!(show("sequence(5, 1)"), "");
    assert_eq!(show("slice([1, 2, 3, 4], 1, -1)"), "2,3");
    assert_eq!(show("slice([1, 2, 3], -2)"), "2,3");
    assert_eq!(show("sort([3, 1, 2])"), "1,2,3");
    assert_eq!(show("sort(['b', 'c', 'a'])"), "a,b,c");
    assert_eq!(
        eval("sort([2, undefined, 1])"),
        Value::from(vec![1.0.into(), 2.0.into(), Value::Undefined])
    );
    assert_eq!(num("span([2, 5, 9])"), 7.0);
    assert_eq!(num("span(null)"), 0.0);
    assert_eq!(num("span(['a', 'b'])"), 0.0);
    for call in [
        "peek(null)",
        "reverse('ab')",
        "sort(1)",
        "join(null)",
        "length(null)",
        "indexof(1, 1)",
    ] {
        assert!(
            matches!(try_eval(call, 0, &[]), Err(Error::Runtime(_))),
            "{call}"
        );
    }
}

#[test]
fn strings() {
    assert_eq!(num("indexof('hello', 'l')"), 2.0);
    assert_eq!(num("lastindexof('hello', 'l')"), 3.0);
    assert_eq!(num("indexof('hello', 'z')"), -1.0);
    assert_eq!(num("indexof('😀a', 'a')"), 2.0);
    assert_eq!(num("length('😀')"), 2.0);
    assert_eq!(show("lower('AbC')"), "abc");
    assert_eq!(show("upper('straße')"), "STRASSE");
    assert_eq!(show("pad('7', 3, '0', 'left')"), "007");
    assert_eq!(show("pad('ab', 5) + '|'"), "ab   |");
    assert_eq!(show("pad('a', 4, '*', 'center')"), "*a**");
    assert_eq!(show("pad('abc', 2)"), "abc");
    assert_eq!(show("pad(5, 3, 0)"), "5  ");
    assert_eq!(num("parseFloat('2.5abc')"), 2.5);
    assert_eq!(num("parseFloat('  -.5e1x')"), -5.0);
    assert_eq!(num("parseFloat('Infinityx')"), f64::INFINITY);
    assert!(num("parseFloat('x')").is_nan());
    assert_eq!(num("parseInt('42px')"), 42.0);
    assert_eq!(num("parseInt('-0x1A')"), -26.0);
    assert_eq!(num("parseInt('101', 2)"), 5.0);
    assert_eq!(num("parseInt('3.9')"), 3.0);
    assert!(num("parseInt('z', 37)").is_nan());
    assert!(num("parseInt('')").is_nan());
    assert_eq!(show("replace('a-b-c', '-', '+')"), "a+b-c");
    assert_eq!(show("replace('a-b-c', /-/g, '+')"), "a+b+c");
    assert_eq!(show("replace('a-b-c', /-/, '+')"), "a+b-c");
    assert_eq!(
        show(r"replace('John Smith', /(\w+)\s(\w+)/, '$2, $1')"),
        "Smith, John"
    );
    assert_eq!(show("replace('abc', 'b', '[$&]')"), "a[b]c");
    assert_eq!(show("replace('abc', 'b', '$`$\\'$$')"), "aac$c");
    assert_eq!(show("replace('abc', /(?<x>b)/, '$<x>$<x>')"), "abbc");
    assert_eq!(show("replace('abc', /(b)/, '$2$1$0')"), "a$2b$0c");
    assert_eq!(show("replace('aaa', /a/g, '$10')"), "$10$10$10");
    assert_eq!(show("replace('abc', /(b)/, '$10')"), "ab0c");
    assert_eq!(show("split('a,b,,c', ',')"), "a,b,,c");
    assert_eq!(num("length(split('a,b,,c', ','))"), 4.0);
    assert_eq!(show("split('a,b,c', ',', 2)"), "a,b");
    assert_eq!(num("length(split('abc', ''))"), 3.0);
    assert_eq!(num("length(split('abc'))"), 1.0);
    assert_eq!(num("length(split('', ','))"), 1.0);
    assert_eq!(show("join(split('a1b2c', /(\\d)/), '|')"), "a|1|b|2|c");
    assert_eq!(show("join(split('abc', /(?:)/), '|')"), "a|b|c");
    assert_eq!(show("join(split('a, b ,c', /\\s*,\\s*/), '|')"), "a|b|c");
    assert_eq!(num("length(split('', /x*/))"), 0.0);
    assert_eq!(show("substring('hello', 3, 1)"), "el");
    assert_eq!(show("substring('hello', -2)"), "hello");
    assert_eq!(show("substring('hello', 1, NaN)"), "h");
    assert_eq!(show("slice('hello', -3)"), "llo");
    assert_eq!(show("slice('hello', 1, 3)"), "el");
    assert_eq!(show("trim('  x \\n')"), "x");
    assert_eq!(show("truncate('hello world', 8)"), "hello w…");
    assert_eq!(show("truncate('hello world', 8, 'left')"), "…o world");
    assert_eq!(show("truncate('hello world', 8, 'center')"), "hell…rld");
    assert_eq!(
        show("truncate('hello world', 7, 'right', '...')"),
        "hell..."
    );
    assert_eq!(show("truncate('hi', 8)"), "hi");
}

#[test]
fn formats_are_d3s() {
    assert_eq!(show("format(1234.5, ',.2f')"), "1,234.50");
    assert_eq!(show("format(-0.5, '.0%')"), "\u{2212}50%");
    assert_eq!(show("format(1500, 's')"), "1.50000k");
    assert!(matches!(
        try_eval("format(1, 'bogus')", 0, &[]),
        Err(Error::Runtime(_))
    ));
    let day = utc(2024, 3, 5);
    assert_eq!(
        show(&format!("utcFormat({day}, '%b %d, %Y')")),
        "Mar 05, 2024"
    );
    assert_eq!(num("utcParse('2024-03-05', '%Y-%m-%d')"), day);
    assert_eq!(eval("utcParse('nope', '%Y')"), Value::Null);
    let noon = local(2024, 2, 5, 12, 0);
    assert_eq!(show(&format!("timeFormat({noon}, '%I %p')")), "12 PM");
    assert_eq!(num("timeParse('2024-03-05 12', '%Y-%m-%d %H')"), noon);
}

#[test]
fn regular_expressions() {
    assert!(yes("test(/^a/i, 'Abc')"));
    assert!(!yes("test(/^a/, 'Abc')"));
    assert!(yes("test(regexp('b+', 'g'), 'abbc')"));
    assert!(yes("test('^\\\\d+$', '123')"));
    assert!(yes("test(/a.c/s, 'a\\nc')"));
    assert!(!yes("test(/a.c/, 'a\\nc')"));
    assert!(yes("test(/^b$/m, 'a\\nb')"));
    assert!(yes("test(/[/]/, 'a/b')"));
    assert!(yes("test(/a\\/b/, 'a/b')"));
    assert_eq!(show("regexp('a', 'g')"), "/a/g");
    assert_eq!(show("regexp()"), "/(?:)/");
    assert_eq!(show("toString(/a/g)"), "/a/g");
    assert!(yes("regexp(/x/) == '/x/'"));
    assert!(matches!(
        try_eval("regexp('a', 'q')", 0, &[]),
        Err(Error::Runtime(_))
    ));
    assert!(matches!(
        try_eval("regexp('(')", 0, &[]),
        Err(Error::Runtime(_))
    ));
}

#[test]
fn errors_print() {
    assert_eq!(
        Error::Function("foo".into()).to_string(),
        "unknown function `foo`"
    );
    let error = "1 +".parse::<Expr>().unwrap_err();
    assert!(error.to_string().ends_with("at 3"), "{error}");
}

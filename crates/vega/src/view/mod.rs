//! A Vega view: a spec's signals, data and scales, each computed when first
//! read, and the scenegraph of its marks, encoded, bounded and laid out as
//! Vega's dataflow does in one run.

use crate::{
    Error, bound,
    config::Config,
    encode::Encoder,
    expr::{Context, Expr},
    guide::{axis, truthy},
    layout, loader,
    scale::Scale,
    scene::{Item, Mark},
    spec::{self, Param, SignalRef},
    statistics::Random,
    transform::Tuple,
    value::Value,
};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
};

mod scale;

/// Reads a data set's `url` to its text; `None` where it cannot.
pub type Loader<'a> = &'a dyn Fn(&str) -> Option<String>;

struct SignalDef {
    value: Option<Json>,
    update: Option<Expr>,
}

/// A scenegraph and what places it, as Vega's renderers set them: the
/// view's size inside its padding, where the root group's origin sits in
/// it, the padding and the background.
#[derive(Clone, Debug)]
pub struct Drawing {
    pub scene: Mark,
    pub width: f64,
    pub height: f64,
    pub origin: [f64; 2],
    /// Top, right, bottom, left.
    pub padding: [f64; 4],
    pub background: Option<String>,
}

/// A Vega view over a spec.
pub struct View<'a> {
    spec: spec::Spec,
    config: Config,
    loader: Loader<'a>,
    signals: IndexMap<String, SignalDef>,
    signal_values: Vec<OnceCell<Value>>,
    data: IndexMap<String, spec::Data>,
    data_values: Vec<OnceCell<Result<Vec<Tuple>, Error>>>,
    /// The values of the signals transforms hold, by name, as their data
    /// sets compute them.
    transform_values: RefCell<HashMap<String, Value>>,
    /// The data set whose transform holds each such signal.
    transform_signals: HashMap<String, usize>,
    /// One more than the index of the data set being built; 0 for none. Every
    /// set before it is built.
    building: Cell<usize>,
    /// The last `tupleid` given out.
    tuple_id: Cell<u64>,
    /// The last number `identifier` gave out.
    identifier: Cell<f64>,
    random: RefCell<Random>,
    scales: IndexMap<String, spec::Scale>,
    scale_values: Vec<OnceCell<Result<Scale, Error>>>,
    /// The first error a lazily computed value met.
    error: RefCell<Option<Error>>,
    /// The bounds of the root group's content, from its origin.
    root_bounds: RefCell<Option<crate::scene::Bounds>>,
}

impl<'a> View<'a> {
    /// A view of `spec`, its urls read through `loader`.
    pub fn new(spec: spec::Spec, loader: Loader<'a>) -> Result<Self, Error> {
        let config = Config::default();
        let mut signals = IndexMap::new();
        let built_in = |name: &str| -> Option<Json> {
            let from_spec = match name {
                "background" => spec.background.clone(),
                "autosize" => spec.autosize.clone(),
                "padding" => spec.padding.clone(),
                "width" => spec.width.clone(),
                "height" => spec.height.clone(),
                _ => None,
            };
            from_spec
                .filter(|v| !v.is_null())
                .or_else(|| config.0.get(name).cloned())
        };
        let padding = |value: Option<Json>| -> Json {
            let number = |v: Option<&Json>| v.and_then(Json::as_f64).unwrap_or(0.0);
            match value {
                Some(Json::Object(o)) if o.contains_key("signal") => Json::Object(o),
                Some(Json::Object(o)) => json!({
                    "top": number(o.get("top")),
                    "bottom": number(o.get("bottom")),
                    "left": number(o.get("left")),
                    "right": number(o.get("right")),
                }),
                v => {
                    let p = v.as_ref().and_then(Json::as_f64).unwrap_or(0.0);
                    json!({"top": p, "bottom": p, "left": p, "right": p})
                }
            }
        };
        let autosize = |value: Option<Json>| match value {
            Some(Json::Object(o)) => Json::Object(o),
            Some(Json::String(t)) => json!({"type": t}),
            _ => json!({"type": "pad"}),
        };
        for (name, value) in [
            ("background", built_in("background")),
            ("autosize", Some(autosize(built_in("autosize")))),
            ("padding", Some(padding(built_in("padding")))),
            (
                "width",
                Some(built_in("width").filter(truthy).unwrap_or(json!(0))),
            ),
            (
                "height",
                Some(built_in("height").filter(truthy).unwrap_or(json!(0))),
            ),
        ] {
            let def = match value {
                Some(Json::Object(o)) if o.contains_key("signal") => SignalDef {
                    value: None,
                    update: Some(parse(o["signal"].as_str().unwrap_or_default())?),
                },
                value => SignalDef {
                    value,
                    update: None,
                },
            };
            signals.insert(name.to_string(), def);
        }
        for signal in &spec.signals {
            let update = signal.update.as_deref().map(parse).transpose()?;
            match signals.get_mut(&signal.name) {
                Some(def) => {
                    if signal.value.is_some() {
                        def.value = signal.value.clone();
                    }
                    if update.is_some() {
                        def.update = update;
                    }
                }
                None => {
                    signals.insert(
                        signal.name.clone(),
                        SignalDef {
                            value: signal.value.clone(),
                            update,
                        },
                    );
                }
            }
        }
        let mut data = IndexMap::new();
        let mut transform_signals = HashMap::new();
        for (index, set) in spec.data.iter().enumerate() {
            if data.insert(set.name.clone(), set.clone()).is_some() {
                return Err(Error::Spec(format!(
                    "Duplicate data set name: \"{}\"",
                    set.name
                )));
            }
            for transform in &set.transform {
                if let Some(signal) = transform.signal() {
                    transform_signals.insert(signal.to_string(), index);
                }
            }
        }
        let mut scales = IndexMap::new();
        for s in &spec.scales {
            if scales.insert(s.name.clone(), s.clone()).is_some() {
                return Err(Error::Spec(format!(
                    "Duplicate scale or projection name: \"{}\"",
                    s.name
                )));
            }
        }
        Ok(Self {
            signal_values: (0..signals.len()).map(|_| OnceCell::new()).collect(),
            data_values: (0..data.len()).map(|_| OnceCell::new()).collect(),
            transform_values: RefCell::default(),
            transform_signals,
            building: Cell::new(0),
            tuple_id: Cell::new(0),
            identifier: Cell::new(0.0),
            random: RefCell::default(),
            scale_values: (0..scales.len()).map(|_| OnceCell::new()).collect(),
            spec,
            config,
            loader,
            signals,
            data,
            scales,
            error: RefCell::new(None),
            root_bounds: RefCell::new(None),
        })
    }

    fn fail(&self, error: Error) {
        self.error.borrow_mut().get_or_insert(error);
    }

    /// The signal named `name`; `undefined` where there is none.
    pub fn signal(&self, name: &str) -> Value {
        let Some((index, _, def)) = self.signals.get_full(name) else {
            return self.transform_signal(name);
        };
        self.signal_values[index]
            .get_or_init(|| {
                let value = def.value.as_ref().map(Value::from).unwrap_or_default();
                match &def.update {
                    Some(update) => match self.eval(update, &Value::Undefined) {
                        Ok(value) => value,
                        Err(error) => {
                            self.fail(error);
                            Value::Undefined
                        }
                    },
                    None => value,
                }
            })
            .clone()
    }

    /// The scale named `name`, configured; `None` where there is none or it
    /// failed, its error kept for the scenegraph.
    pub fn scale(&self, name: &str) -> Option<&Scale> {
        match self.try_scale(name)? {
            Ok(scale) => Some(scale),
            Err(error) => {
                self.fail(error);
                None
            }
        }
    }

    /// The scale named `name`, or why it could not be configured; `None`
    /// where the spec has no such scale.
    pub fn try_scale(&self, name: &str) -> Option<Result<&Scale, Error>> {
        let (index, _, def) = self.scales.get_full(name)?;
        Some(
            self.scale_values[index]
                .get_or_init(|| self.build_scale(def))
                .as_ref()
                .map_err(Clone::clone),
        )
    }

    /// The names of the top-level scales, in the spec's order.
    pub fn scale_names(&self) -> impl Iterator<Item = &str> {
        self.scales.keys().map(String::as_str)
    }

    /// The names of the top-level data sets, in the spec's order.
    pub fn data_names(&self) -> impl Iterator<Item = &str> {
        self.data.keys().map(String::as_str)
    }

    /// The tuples of the data set named `name`. Data sets compute in the
    /// spec's order, each before any after it.
    pub fn data(&self, name: &str) -> Result<&[Tuple], Error> {
        let index = self
            .data
            .get_index_of(name)
            .ok_or_else(|| Error::Spec(format!("Undefined data set name: \"{name}\"")))?;
        for i in 0..=index {
            let def = &self.data[i];
            if self.data_values[i].get().is_none() && self.building.get() == i + 1 {
                return Err(Error::Spec(format!(
                    "Data set \"{}\" reads itself.",
                    def.name
                )));
            }
            let tuples = self.data_values[i].get_or_init(|| {
                let outer = self.building.replace(i + 1);
                let tuples = self.build_data(def);
                self.building.set(outer);
                tuples
            });
            if i == index {
                return tuples.as_deref().map_err(Clone::clone);
            }
        }
        unreachable!("the data set is in range")
    }

    /// The value of a signal a transform holds; `undefined` where none
    /// does.
    fn transform_signal(&self, name: &str) -> Value {
        if let Some(value) = self.transform_values.borrow().get(name) {
            return value.clone();
        }
        let Some(&index) = self.transform_signals.get(name) else {
            return Value::Undefined;
        };
        if let Err(error) = self.data(self.data.get_index(index).expect("in range").0) {
            self.fail(error);
        }
        self.transform_values
            .borrow()
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    /// vega-dataflow's `ingest`: `datum` with the next `tupleid`.
    pub(crate) fn ingest(&self, datum: Value) -> Tuple {
        self.tuple_id.set(self.tuple_id.get() + 1);
        Tuple {
            id: self.tuple_id.get(),
            datum,
        }
    }

    /// The next of the view's random numbers, which Vega's `random` draws.
    pub(crate) fn random(&self) -> f64 {
        self.random.borrow_mut().draw()
    }

    /// The datums of the data set named `name`, as the expression `data`
    /// reads them; none where it fails.
    fn datums(&self, name: &str) -> Vec<Value> {
        match self.data(name) {
            Ok(tuples) => tuples.iter().map(|t| t.datum.clone()).collect(),
            Err(error) => {
                self.fail(error);
                Vec::new()
            }
        }
    }

    /// The next number `identifier` gives out.
    pub(crate) fn next_identifier(&self) -> f64 {
        self.identifier.set(self.identifier.get() + 1.0);
        self.identifier.get()
    }

    /// A parameter's value: as written, or its signal's, read as `T`.
    pub(crate) fn param<T: Clone + DeserializeOwned>(&self, param: &Param<T>) -> Result<T, Error> {
        match param {
            Param::Value(value) => Ok(value.clone()),
            Param::Signal(signal) => {
                let value = self.signal_ref(signal)?;
                serde_json::from_value(Json::from(&value))
                    .map_err(|e| Error::Spec(format!("{}: {e}", signal.signal)))
            }
        }
    }

    /// A parameter's value as JavaScript holds it: as written, or its
    /// signal's.
    pub(crate) fn param_value(
        &self,
        param: &Param<impl Clone + Into<Json>>,
    ) -> Result<Value, Error> {
        match param {
            Param::Value(value) => Ok(Value::from(&value.clone().into())),
            Param::Signal(signal) => self.signal_ref(signal),
        }
    }

    /// A `{signal}` reference's value.
    pub(crate) fn signal_ref(&self, signal: &SignalRef) -> Result<Value, Error> {
        self.eval(&self.parse(&signal.signal)?, &Value::Undefined)
    }

    /// An expression, parsed.
    pub(crate) fn parse(&self, code: &str) -> Result<Expr, Error> {
        parse(code)
    }

    /// `expr` on `datum`, reading this view's signals and scales.
    pub(crate) fn eval(&self, expr: &Expr, datum: &Value) -> Result<Value, Error> {
        let param = |name: &str| self.signal(name);
        let scale = |name: &str| self.scale(name);
        let data = |name: &str| self.datums(name);
        let context = Context {
            datum,
            param: &param,
            scale: &scale,
            data: &data,
            random: &self.random,
        };
        expr.eval(&context).map_err(Error::Eval)
    }

    /// A JSON property: a literal, or `{signal}` evaluated.
    fn property(&self, value: &Json) -> Result<Value, Error> {
        match value {
            Json::Object(o) if o.contains_key("signal") => {
                let expr = parse(o["signal"].as_str().unwrap_or_default())?;
                self.eval(&expr, &Value::Undefined)
            }
            value => Ok(Value::from(value)),
        }
    }

    /// vega-parser's data pipeline: the source's tuples, run through each
    /// transform. A set derived from others copies their tuples, with ids of
    /// its own, where a transform modifies them before any generates new
    /// ones.
    fn build_data(&self, def: &spec::Data) -> Result<Vec<Tuple>, Error> {
        let format = def.format.clone().unwrap_or_default();
        let mut tuples: Vec<Tuple> = if let Some(values) = &def.values {
            loader::values(values, &format)?
                .into_iter()
                .map(|d| self.ingest(ingest(d)))
                .collect()
        } else if let Some(url) = &def.url {
            let text = (self.loader)(url).ok_or_else(|| Error::Load(url.clone()))?;
            loader::read(&text, &format)?
                .into_iter()
                .map(|d| self.ingest(ingest(d)))
                .collect()
        } else if let Some(sources) = &def.source {
            let mut tuples = Vec::new();
            for source in sources.items() {
                tuples.extend_from_slice(self.data(source)?);
            }
            let mut modifies = false;
            for transform in &def.transform {
                let metadata = transform.metadata();
                if metadata.generates {
                    break;
                }
                modifies |= metadata.modifies;
            }
            if modifies {
                tuples = tuples.into_iter().map(|t| self.ingest(t.datum)).collect();
            }
            tuples
        } else {
            Vec::new()
        };
        for transform in &def.transform {
            let (out, value) = transform.run(self, tuples)?;
            tuples = out;
            if let (Some(signal), Some(value)) = (transform.signal(), value) {
                self.transform_values
                    .borrow_mut()
                    .insert(signal.to_string(), value);
            }
        }
        Ok(tuples)
    }

    /// The scenegraph: the root group mark and everything in it.
    pub fn scene(&self) -> Result<Mark, Error> {
        let mut root_encode = self.spec.encode.clone().unwrap_or_default();
        let mut enter = spec::Block::new();
        enter.insert("x".into(), value_entry(json!(0)));
        enter.insert("y".into(), value_entry(json!(0)));
        enter.extend(root_encode.get("enter").cloned().unwrap_or_default());
        let mut update = spec::Block::new();
        update.insert("width".into(), signal_entry("width"));
        update.insert("height".into(), signal_entry("height"));
        update.extend(root_encode.get("update").cloned().unwrap_or_default());
        root_encode.insert("enter".into(), enter);
        root_encode.insert("update".into(), update);
        let root = spec::Mark {
            type_: "group".into(),
            name: Some("root".into()),
            role: Some("frame".into()),
            style: self.spec.style.clone(),
            encode: Some(root_encode),
            axes: self.spec.axes.clone(),
            marks: self.spec.marks.clone(),
            ..spec::Mark::default()
        };
        let mut guides = HashMap::new();
        let mut mark = self.mark(&root, &mut guides, true)?;
        mark.interactive = true;
        match self.error.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(mark),
        }
    }

    /// The scenegraph placed as `autosize` `pad` places it: the view grown
    /// to hold what overhangs the root group.
    pub fn drawing(&self) -> Result<Drawing, Error> {
        let scene = self.scene()?;
        let autosize = self.signal("autosize");
        let kind = autosize.get(&"type".into()).unwrap_or_default().to_string();
        if kind != "pad" {
            return Err(Error::Unsupported(format!("autosize {kind}")));
        }
        let root = &scene.items[0];
        let (width, height) = (
            root.or_zero("width").max(0.0),
            root.or_zero("height").max(0.0),
        );
        let bounds = self.root_bounds.borrow().unwrap_or_default();
        let left = (-bounds.x1).ceil().max(0.0);
        let top = (-bounds.y1).ceil().max(0.0);
        let right = (bounds.x2 - width).ceil().max(0.0);
        let bottom = (bounds.y2 - height).ceil().max(0.0);
        let padding = self.signal("padding");
        let side = |name: &str| padding.get(&name.into()).map_or(0.0, |v| v.to_number());
        let background = match self.signal("background") {
            Value::String(color) => Some(color.to_string()),
            _ => None,
        };
        Ok(Drawing {
            width: width + left + right,
            height: height + top + bottom,
            origin: [left, top],
            padding: [side("top"), side("right"), side("bottom"), side("left")],
            background,
            scene,
        })
    }

    /// A mark's items, encoded and bounded, and for a group its content,
    /// laid out where it is a view.
    fn mark(
        &self,
        def: &spec::Mark,
        guides: &mut HashMap<String, Vec<Value>>,
        root: bool,
    ) -> Result<Mark, Error> {
        if !matches!(def.type_.as_str(), "group" | "rect" | "rule" | "text") {
            return Err(Error::Unsupported(format!("a {} mark", def.type_)));
        }
        let group = def.type_ == "group";
        let role = role(def);
        let tuples: Vec<Value> = match def.from.as_ref().and_then(|f| f.data.as_deref()) {
            None => vec![Value::Object(Vec::new().into())],
            Some(name) => match guides.get(name) {
                Some(tuples) => tuples.clone(),
                None => self.data(name)?.iter().map(|t| t.datum.clone()).collect(),
            },
        };
        let style: Vec<String> = def
            .style
            .as_ref()
            .map(|s| s.items().to_vec())
            .unwrap_or_default();
        let encoder = Encoder::new(
            &def.encode.clone().unwrap_or_default(),
            &def.type_,
            &role,
            &style,
            &self.config,
        )?;
        let signal = |name: &str| self.signal(name);
        let scale = |name: &str| self.scale(name);
        let mut items = Vec::with_capacity(tuples.len());
        for datum in tuples {
            let mut item = Item {
                datum,
                ..Item::default()
            };
            let data = |name: &str| self.datums(name);
            encoder.apply("enter", &mut item, &signal, &scale, &data, &self.random)?;
            encoder.apply("update", &mut item, &signal, &scale, &data, &self.random)?;
            items.push(item);
        }
        let mut mark = Mark {
            marktype: def.type_.clone(),
            name: def.name.clone(),
            role: role.clone(),
            interactive: def.interactive != Some(false),
            clip: def.clip == Some(true),
            zindex: def.zindex.unwrap_or(0.0),
            aria: def.aria,
            description: def.description.clone(),
            items,
            bounds: Default::default(),
        };
        if group {
            for item in &mut mark.items {
                let mut children = Vec::new();
                for (i, axis) in def.axes.iter().enumerate() {
                    children.push(self.axis(axis, i, guides)?);
                }
                for child in &def.marks {
                    children.push(self.mark(child, guides, false)?);
                }
                item.items = children;
            }
            if root || role == "scope" || role == "frame" {
                for item in &mut mark.items {
                    let bounds = layout::view(item);
                    if root {
                        *self.root_bounds.borrow_mut() = Some(bounds);
                    }
                }
            }
        }
        bound::mark(&mut mark);
        if let Some(overlap) = &def.overlap {
            layout::overlap(&mut mark, overlap, &|name| self.scale(name))?;
        }
        Ok(mark)
    }

    /// An axis's group mark, its data registered under names of its own.
    fn axis(
        &self,
        def: &spec::Axis,
        index: usize,
        guides: &mut HashMap<String, Vec<Value>>,
    ) -> Result<Mark, Error> {
        let scale = self
            .scale(&def.scale)
            .ok_or_else(|| Error::Spec(format!("Unrecognized scale name: \"{}\"", def.scale)))?;
        let band = self
            .scales
            .get(&def.scale)
            .is_some_and(|s| s.type_.as_deref() == Some("band"));
        let sources = axis::Sources {
            datum: format!(":axis:{index}:datum"),
            ticks: format!(":axis:{index}:ticks"),
        };
        let spec_json = axis::mark(def, &self.config, band, &sources);
        let mark: spec::Mark =
            serde_json::from_value(spec_json).map_err(|e| Error::Spec(e.to_string()))?;
        guides.insert(
            sources.datum.clone(),
            vec![axis_datum(def, &self.config, band)],
        );
        guides.insert(sources.ticks.clone(), self.ticks(def, scale)?);
        self.mark(&mark, guides, false)
    }

    /// vega-encode's `AxisTicks`.
    fn ticks(&self, def: &spec::Axis, scale: &Scale) -> Result<Vec<Value>, Error> {
        let count = match &def.tick_count {
            Some(count) => self.property(count)?,
            None => Value::Number(match &def.values {
                Some(Json::Array(values)) => values.len() as f64,
                _ => 10.0,
            }),
        };
        if !matches!(count, Value::Number(_)) {
            return Err(Error::Unsupported(
                "a tick count that is not a number".into(),
            ));
        }
        let count = count.to_number();
        let min_step = def
            .tick_min_step
            .as_ref()
            .map(|s| self.property(s))
            .transpose()?;
        let count = match min_step {
            Some(step) if !step.is_nullish() => scale.tick_count(count, Some(step.to_number())),
            _ => count,
        };
        if def.values.is_some() || def.format_type.is_some() {
            return Err(Error::Unsupported("axis values or format type".into()));
        }
        let format = def
            .format
            .as_ref()
            .map(|f| self.property(f))
            .transpose()?
            .map(|f| f.to_string());
        let format = scale
            .tick_format(count, format.as_deref())
            .map_err(Error::Spec)?;
        let values = scale.ticks(count);
        let last = (values.len().saturating_sub(1)).max(1) as f64;
        Ok(values
            .iter()
            .enumerate()
            .map(|(i, value)| {
                Value::Object(
                    vec![
                        ("index".into(), Value::Number(i as f64 / last)),
                        ("tickIndex".into(), Value::Number(i as f64)),
                        ("value".into(), value.clone()),
                        ("label".into(), Value::from(format.format(value))),
                    ]
                    .into(),
                )
            })
            .collect())
    }
}

/// The datum of an axis group: what parts it draws.
fn axis_datum(def: &spec::Axis, config: &Config, band: bool) -> Value {
    let json = axis::datum(def, config, band);
    Value::from(&json)
}

fn value_entry(value: Json) -> spec::Channel {
    spec::Channel::Entry(Box::new(spec::Entry {
        value: Some(value),
        ..spec::Entry::default()
    }))
}

fn signal_entry(signal: &str) -> spec::Channel {
    spec::Channel::Entry(Box::new(spec::Entry {
        signal: Some(signal.into()),
        ..spec::Entry::default()
    }))
}

/// vega-parser's mark `role`.
fn role(def: &spec::Mark) -> String {
    let role = def.role.clone().unwrap_or_default();
    if role.starts_with("axis") || role.starts_with("legend") || role.starts_with("title") {
        role
    } else if def.type_ == "group" && role != "frame" {
        "scope".into()
    } else if role.is_empty() {
        "mark".into()
    } else {
        role
    }
}

fn parse(code: &str) -> Result<Expr, Error> {
    code.parse().map_err(|e| Error::Expression(code.into(), e))
}

/// vega-dataflow's `ingest`: an object as it is, anything else as
/// `{data: value}`.
fn ingest(value: Value) -> Value {
    match value {
        Value::Object(_) => value,
        other => Value::Object(vec![("data".into(), other)].into()),
    }
}

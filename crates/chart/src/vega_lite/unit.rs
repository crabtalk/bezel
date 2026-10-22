//! A unit view: its mark and encoding read, and the layer it draws from the
//! rows the dataflow gives it.

use crate::{
    data::{Column, Data},
    model::{Chart, Domain, Kind, Mark, Range},
    transform::{Bins, Random},
    vega_lite::{
        encoding::{self, Def},
        field::{self, Derive},
        mark,
        sort::{self, DomainSort},
        union::Layered,
    },
};
use gpui::SharedString;
use serde_json::{Map, Value};

/// A unit read, and where the dataflow gives it its rows.
pub(crate) struct Unit {
    /// One layer, with no rows yet.
    pub(crate) chart: Chart,
    pub(crate) defs: Vec<Def>,
    pub(crate) mark: Mark,
    /// The fields its channels' ranges read.
    ranges: Vec<SharedString>,
    /// The index of its source.
    pub(crate) source: usize,
    /// The taps of its rows before its aggregate and as its marks draw them.
    pub(crate) raw: usize,
    pub(crate) main: usize,
}

impl Unit {
    /// `mark` drawn as `map` says, its `invalid` mode read with the spec's
    /// `config`. `None` for a mark or encoding this does not read.
    pub(crate) fn read(
        def: &Value,
        map: &Map<String, Value>,
        config: Option<&Value>,
    ) -> Option<Self> {
        let (mark, mut properties, map) = mark::read(def, map)?;
        let configured = |key: &str| config?.get(key)?.get("invalid");
        let name = def.get("type").unwrap_or(def).as_str()?;
        properties.invalid = mark::invalid(
            mark,
            def.get("invalid"),
            configured(name).or_else(|| configured("mark")),
        )?;
        let mut chart = Chart::new(mark, Data::new()).properties(properties);
        let (encoding, defs) = encoding::read(&map, Some(mark))?;
        let ranges = encoding
            .channels()
            .iter()
            .filter_map(|c| match &c.scale.range {
                Some(Range::Field(field)) => Some(field.clone()),
                _ => None,
            })
            .collect();
        chart.layers[0].encoding = encoding;
        Some(Self {
            chart,
            defs,
            mark,
            ranges,
            source: 0,
            raw: 0,
            main: 0,
        })
    }

    /// The fields of its source it reads, each with the type to read it as:
    /// a range field as the channel's values are, as text.
    pub(crate) fn reads(&self) -> Vec<(SharedString, Option<Kind>)> {
        self.defs
            .iter()
            .flat_map(Def::reads)
            .chain(
                self.ranges
                    .iter()
                    .map(|field| (field.clone(), Some(Kind::Nominal))),
            )
            .collect()
    }

    /// The layer it draws: `main` the rows its marks draw, `raw` its rows
    /// before the aggregate, which its sorts by an aggregate read, drawing
    /// from `random`, and `bins` each encoded bin's edges by the field of
    /// its starts. `None` for a sort Vega cannot run.
    pub(crate) fn layer(
        mut self,
        raw: Data,
        main: Data,
        bins: &dyn Fn(&str) -> Option<Bins>,
        random: &mut Random,
    ) -> Option<Layered> {
        let defs = &self.defs;
        let stacked = matches!(self.mark, Mark::Bar | Mark::Area)
            && defs.iter().any(|def| {
                sort::STACK_BY.contains(&def.channel.as_str())
                    && def.field.is_some()
                    && !matches!(def.derive, Derive::Aggregate { .. })
            });
        let mut sorts = Vec::with_capacity(defs.len());
        let layer = &mut self.chart.layers[0];
        for def in defs {
            let Some(sort) = &def.sort else {
                sorts.push(None);
                continue;
            };
            let sort = DomainSort::of(def, sort, stacked, defs)?;
            let order = sort.order(&raw, &sort::key(def)?, random);
            if let Some(channel) = encoding::scaled_mut(&mut layer.encoding, &def.channel) {
                channel.sort = order;
            }
            sorts.push(Some(sort));
        }
        for def in defs {
            let Derive::Bin(bin) = &def.derive else {
                continue;
            };
            let Some(edges) = bins(&bin.output[0]) else {
                continue;
            };
            let Some(channel) = encoding::scaled_mut(&mut layer.encoding, &def.channel) else {
                continue;
            };
            if channel.kind == Kind::Quantitative {
                channel.scale.bins = Some(edges);
            }
        }
        let source = bands(main, defs);
        let mut data = Data::new();
        let channels = layer.encoding.channels();
        let encoded = channels.iter().flat_map(|channel| {
            let spans = match &channel.scale.domain {
                Some(Domain::Unaggregated(fields)) => &fields[..],
                _ => &[],
            };
            channel.field().into_iter().chain(spans)
        });
        for field in encoded.chain(&self.ranges) {
            if data.column(field).is_some() {
                continue;
            }
            let column = match source.column(field) {
                Some(column) => column.clone(),
                None => Column::Number(vec![f64::NAN; source.len()].into()),
            };
            data = data.adopt(&source, field.clone(), column);
        }
        layer.data = data;
        Some(Layered {
            chart: self.chart,
            defs: self.defs,
            raw,
            sorts,
        })
    }
}

/// Where marks stand in the bins and steps `defs` place them in.
fn bands(mut data: Data, defs: &[Def]) -> Data {
    for def in defs {
        let (Some(p), Some([start, end])) = (def.band, def.edges()) else {
            continue;
        };
        let places: Vec<f64> = data
            .numbers(&start)
            .iter()
            .zip(data.numbers(&end))
            .map(|(start, end)| (1.0 - p) * start + p * end)
            .collect();
        data = data.number(field::band(&start, p), places);
    }
    data
}

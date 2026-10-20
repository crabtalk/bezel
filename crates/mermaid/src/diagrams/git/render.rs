//! Pele's `diagrams/git/render.ts`.

use super::GitGraphConfig;
use crate::geom::{Direction, Point, Size};
use std::{
    collections::HashMap,
    fmt::{self, Write as _},
};

use super::db::{CommitType, GitCommit, GitModel};
use crate::diagrams::Render;
use crate::{Config, Ctx, Error, Host};
use crate::{
    Rendered, js,
    svg::{
        builder::{EscText, fragment, num},
        root::svg_document,
        theme::{RADIUS, series_color},
    },
    text::{Label, Style},
};

const DOT: f64 = 7.0;
const STEP: f64 = 44.0;
const LANE: f64 = 44.0;
/// Shorter than a step, so a curve between lanes never runs through a commit
/// on a lane it crosses.
const CURVE: f64 = 36.0;
const GAP: f64 = 8.0;
const LABEL_GAP: f64 = 20.0;
const TAIL: f64 = 20.0;
const NOTCH: f64 = 7.0;
const TAG_GAP: f64 = 6.0;
const HALF: f64 = std::f64::consts::FRAC_1_SQRT_2;

struct View<'m> {
    commit: &'m GitCommit,
    index: usize,
    lane: usize,
    m: f64,
    label: Option<Label>,
    tags: Vec<Label>,
}

fn tag_width(tag: &Label) -> f64 {
    NOTCH + 4.0 + tag.width + 6.0
}

impl CommitType {
    fn class(self) -> &'static str {
        match self {
            CommitType::Normal => "normal",
            CommitType::Reverse => "reverse",
            CommitType::Highlight => "highlight",
            CommitType::Merge => "merge",
            CommitType::CherryPick => "cherry-pick",
        }
    }

    /// Whether Mermaid makes up the message of a commit of this kind.
    fn makes_up_message(self) -> bool {
        matches!(self, CommitType::Merge | CommitType::CherryPick)
    }

    /// The commit's mark, centred on `at`.
    fn symbol(self, at: Point, color: impl fmt::Display) -> impl fmt::Display {
        fragment(move |f| {
            let Point { x, y } = at;
            let centre = fragment(|f| write!(f, "cx=\"{}\" cy=\"{}\"", num(x), num(y)));
            match self {
                CommitType::Highlight => {
                    return write!(
                        f,
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"{color}\"/>",
                        num(x - DOT - 1.0),
                        num(y - DOT - 1.0),
                        js::Number(2.0 * DOT + 2.0),
                        js::Number(2.0 * DOT + 2.0)
                    );
                }
                CommitType::Merge => {
                    return write!(
                        f,
                        "<circle {centre} r=\"{}\" fill=\"var(--_bg)\" stroke=\"{color}\" stroke-width=\"2\"/><circle {centre} r=\"2.5\" fill=\"{color}\"/>",
                        js::Number(DOT - 1.0)
                    );
                }
                _ => {}
            }
            write!(
                f,
                "<circle {centre} r=\"{}\" fill=\"{color}\"/>",
                js::Number(DOT)
            )?;
            match self {
                CommitType::Reverse => write!(
                    f,
                    "<path d=\"M{}l6,6M{}l6,-6\" fill=\"none\" stroke=\"var(--_bg)\" stroke-width=\"1.5\" stroke-linecap=\"round\"/>",
                    Point::new(x - 3.0, y - 3.0),
                    Point::new(x - 3.0, y + 3.0)
                ),
                CommitType::CherryPick => write!(
                    f,
                    "<path d=\"M{}L{}L{}\" fill=\"none\" stroke=\"var(--_bg)\" stroke-linejoin=\"round\"/><circle cx=\"{}\" cy=\"{}\" r=\"1.8\" fill=\"var(--_bg)\"/><circle cx=\"{}\" cy=\"{}\" r=\"1.8\" fill=\"var(--_bg)\"/>",
                    Point::new(x - 2.6, y + 1.0),
                    Point::new(x + 0.8, y - 4.2),
                    Point::new(x + 2.6, y + 1.0),
                    num(x - 2.6),
                    num(y + 2.2),
                    num(x + 2.6),
                    num(y + 2.2)
                ),
                _ => Ok(()),
            }
        })
    }
}

impl Render for GitModel {
    fn render(&self, config: &Config, ctx: &Ctx<'_, impl Host>) -> Result<Rendered, Error> {
        let config = &config.git_graph;
        Ok(if self.direction != Direction::LeftToRight {
            self.draw(config, ctx, self.direction)
        } else {
            ctx.turn_to_fit(|down| {
                let direction = if down {
                    Direction::TopToBottom
                } else {
                    Direction::LeftToRight
                };
                self.draw(config, ctx, direction)
            })
        })
    }
}

/// The bounds of what is drawn.
struct Bounds {
    min: Point,
    max: Point,
}

impl Bounds {
    /// Grows to take in the box from `lo` to `hi`.
    fn grow(&mut self, lo: Point, hi: Point) {
        if lo.x < self.min.x {
            self.min.x = lo.x;
        }
        if lo.y < self.min.y {
            self.min.y = lo.y;
        }
        if hi.x > self.max.x {
            self.max.x = hi.x;
        }
        if hi.y > self.max.y {
            self.max.y = hi.y;
        }
    }

    /// Grows to take in the box of `size` centred on `centre`.
    fn grow_around(&mut self, centre: Point, size: Size) {
        self.grow(
            Point::new(centre.x - size.w / 2.0, centre.y - size.h / 2.0),
            Point::new(centre.x + size.w / 2.0, centre.y + size.h / 2.0),
        );
    }
}

impl GitModel {
    fn draw(
        &self,
        config: &GitGraphConfig,
        ctx: &Ctx<'_, impl Host>,
        direction: Direction,
    ) -> Rendered {
        let p = crate::PREFIX;
        let size = ctx.options.font_size;
        let small = js::round(size * 0.75);
        let measure = ctx.host;
        let pad = ctx.options.padding.unwrap_or(8.0);

        let show_branches = config.show_branches.unwrap_or(true);
        let show_commit_label = config.show_commit_label.unwrap_or(true);
        let rotate = config.rotate_commit_label.unwrap_or(true);
        let parallel = config.parallel_commits.unwrap_or(false);
        let vertical = direction.is_vertical();
        let flip = direction == Direction::BottomToTop;

        let mut lane_of: HashMap<&str, usize> = HashMap::new();
        for (index, name) in self.lanes.iter().enumerate() {
            lane_of.insert(name, index);
        }
        let lane_count = self.lanes.len();
        let medium = js::round(size * 0.875);
        // Mermaid writes branch names, tags, and commit ids as plain text, so
        // tags in them show as written.
        let names: Vec<Label> = self
            .lanes
            .iter()
            .map(|name| {
                Label::plain(
                    Some(if show_branches { name } else { "" }),
                    &*measure,
                    medium,
                    4000.0,
                    0,
                )
            })
            .collect();

        let mut views: Vec<View> = Vec::new();
        let mut by_id: HashMap<&str, usize> = HashMap::new();
        let mut sorted: Vec<&GitCommit> = self.commits.values().collect();
        sorted.sort_by_key(|commit| commit.seq);
        for commit in sorted {
            let labelled = show_commit_label
                && commit.kind != CommitType::CherryPick
                && (commit.kind != CommitType::Merge || commit.custom_id == Some(true));
            let mut tags = Vec::new();
            for tag in &commit.tags {
                let label = Label::plain(Some(tag), measure, small, 4000.0, 0);
                if !label.lines.is_empty() {
                    tags.push(label);
                }
            }
            by_id.insert(&commit.id, views.len());
            views.push(View {
                commit,
                index: views.len(),
                lane: lane_of.get(commit.branch.as_str()).copied().unwrap_or(0),
                m: 0.0,
                label: labelled
                    .then(|| Label::plain(Some(&commit.id), &*measure, small, 4000.0, 0)),
                tags,
            });
        }

        // Room each commit needs: along the main axis on either side, and across
        // it towards the previous lane (`lo`) and the next one (`hi`). Tags sit on
        // one side of a lane and ids on the other.
        let mut half = vec![0.0; views.len()];
        let mut tag_half = vec![0.0; views.len()];
        let mut lo = vec![DOT; lane_count];
        let mut hi = vec![DOT; lane_count];
        for (k, view) in views.iter().enumerate() {
            let mut tags_main = 0f64;
            let mut tags_cross = 0.0;
            for tag in &view.tags {
                let w = tag_width(tag);
                if vertical {
                    tags_main = tags_main.max(tag.height);
                    tags_cross += w + 4.0;
                } else {
                    tags_main = tags_main.max(w);
                    tags_cross += tag.height + 3.0;
                }
            }
            let mut label_main = 0.0;
            let mut label_cross = 0.0;
            if let Some(label) = &view.label
                && !label.lines.is_empty()
            {
                if rotate {
                    label_cross = 3.0 + HALF * (label.width + label.height);
                } else {
                    label_main = if vertical { label.height } else { label.width };
                    label_cross = 6.0 + if vertical { label.width } else { label.height };
                }
            }
            half[k] = DOT.max(tags_main / 2.0).max(label_main / 2.0);
            tag_half[k] = tags_main / 2.0;
            let tag_side = DOT
                + if view.tags.is_empty() {
                    0.0
                } else {
                    TAG_GAP + tags_cross
                };
            let label_side = DOT + label_cross;
            let lane = view.lane;
            lo[lane] = lo[lane].max(if vertical { label_side } else { tag_side });
            hi[lane] = hi[lane].max(if vertical { tag_side } else { label_side });
        }

        // In a left-to-right graph the tags stand over their commit. A connector
        // that comes down from a lane above joins the commit's lane this far
        // before the commit, clear of the tags, and the commit is placed far
        // enough along to leave room for that.
        let mut lead = vec![0.0; views.len()];
        if !vertical {
            for (k, view) in views.iter().enumerate() {
                if tag_half[k] == 0.0 {
                    continue;
                }
                for id in &view.commit.parents {
                    if let Some(&parent) = by_id.get(id.as_str())
                        && views[parent].lane < view.lane
                    {
                        lead[k] = tag_half[k] + 4.0;
                    }
                }
            }
        }
        let space = |p: usize, k: usize| -> f64 {
            STEP.max(half[p] + half[k] + GAP).max(if lead[k] > 0.0 {
                half[p] + CURVE + lead[k]
            } else {
                0.0
            })
        };

        for k in 0..views.len() {
            if !parallel {
                views[k].m = if k == 0 {
                    0.0
                } else {
                    views[k - 1].m + space(k - 1, k)
                };
                continue;
            }
            // Mermaid's `parallelCommits`: a commit sits one step after its
            // furthest parent, so branches that grow side by side share positions
            // instead of taking turns.
            let mut m = 0f64;
            for id in &views[k].commit.parents {
                let Some(&parent) = by_id.get(id.as_str()) else {
                    continue;
                };
                if views[parent].index >= k {
                    continue;
                }
                m = m.max(views[parent].m + space(views[parent].index, k));
            }
            views[k].m = m;
        }
        let mut end = 0f64;
        for view in &views {
            end = end.max(view.m);
        }

        let mut cross = vec![0.0; lane_count];
        for i in 0..lane_count {
            let name = &names[i];
            let reach = if name.lines.is_empty() {
                0.0
            } else if vertical {
                name.width / 2.0 + 8.0
            } else {
                name.height / 2.0 + 2.0
            };
            lo[i] = lo[i].max(reach);
            hi[i] = hi[i].max(reach);
            if i > 0 {
                cross[i] = cross[i - 1] + LANE.max(hi[i - 1] + lo[i] + GAP);
            }
        }

        // The point `m` along the history and `c` across it.
        let point = |m: f64, c: f64| {
            if vertical {
                Point::new(c, if flip { -m } else { m })
            } else {
                Point::new(m, c)
            }
        };

        let mut bounds = Bounds {
            min: Point::new(f64::INFINITY, f64::INFINITY),
            max: Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
        };

        let mut branches = String::new();
        if show_branches {
            for i in 0..lane_count {
                let name = &names[i];
                let color = series_color(i as i64);
                let c = cross[i];
                let w = name.width + 16.0;
                let h = name.height + 4.0;
                let along = if vertical { h } else { w };
                let from = if name.lines.is_empty() {
                    -LABEL_GAP
                } else {
                    -LABEL_GAP - along
                };
                let centre = point(from + along / 2.0, c);
                bounds.grow(point(-LABEL_GAP, c), point(-LABEL_GAP, c));
                bounds.grow(point(end + TAIL, c), point(end + TAIL, c));
                let _ = write!(
                    branches,
                    "<g class=\"{p}-branch\" data-id=\"{}\"><path class=\"{p}-lane\" d=\"M{}L{}\" stroke=\"{color}\" stroke-dasharray=\"2 4\"/>",
                    EscText(&self.lanes[i]),
                    point(-LABEL_GAP, c),
                    point(end + TAIL, c)
                );
                if !name.lines.is_empty() {
                    bounds.grow_around(centre, Size::new(w, h));
                    let _ = write!(
                        branches,
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{RADIUS}\" fill=\"{color}\"/>",
                        num(centre.x - w / 2.0),
                        num(centre.y - h / 2.0),
                        num(w),
                        num(h)
                    );
                    let _ = write!(
                        branches,
                        "{}",
                        name.svg(centre, crate::class!("label", "fill=\"var(--_bg)\""))
                    );
                }
                branches.push_str("</g>");
            }
        }

        // Positions of the commits on each lane, to tell whether a connector may
        // run along it.
        let mut taken: Vec<Vec<f64>> = vec![Vec::new(); lane_count];
        for view in &views {
            taken[view.lane].push(view.m);
        }
        if parallel {
            for list in &mut taken {
                list.sort_by(|a, b| js::compare(a - b));
            }
        }
        let busy = |lane: usize, from: f64, to: f64| -> bool {
            let list = &taken[lane];
            let mut low = 0;
            let mut high = list.len();
            while low < high {
                let mid = (low + high) >> 1;
                if list[mid] <= from {
                    low = mid + 1;
                } else {
                    high = mid;
                }
            }
            low < list.len() && list[low] < to
        };

        let mut curves = String::new();
        let mut lines = String::new();
        let mut d = String::new();
        for view in &views {
            let commit = view.commit;
            let c2 = cross[view.lane];
            let m2 = view.m;
            for (j, id) in commit.parents.iter().enumerate() {
                let Some(&parent) = by_id.get(id.as_str()) else {
                    continue;
                };
                if parent == view.index {
                    continue;
                }
                let parent = &views[parent];
                let c1 = cross[parent.lane];
                let m1 = parent.m;
                if parent.lane == view.lane {
                    let _ = write!(
                        lines,
                        "<path class=\"{p}-edge\" d=\"M{}L{}\" stroke=\"{}\"/>",
                        point(m1, c1),
                        point(m2, c2),
                        series_color(view.lane as i64)
                    );
                    continue;
                }
                let merged = j > 0 && commit.kind == CommitType::Merge;
                let picked = j > 0 && !merged;
                d.clear();
                let _ = write!(d, "M{}", point(m1, c1));
                if m2 <= m1 {
                    let mid = (m1 + m2) / 2.0;
                    let _ = write!(
                        d,
                        "C{} {} {}",
                        point(mid, c1),
                        point(mid, c2),
                        point(m2, c2)
                    );
                } else if !merged && !busy(view.lane, m1, m2) {
                    // Leaves the parent at once and runs along the child's lane.
                    let s = CURVE.min(m2 - m1);
                    let _ = write!(
                        d,
                        "C{} {} {}L{}",
                        point(m1 + s / 2.0, c1),
                        point(m1 + s / 2.0, c2),
                        point(m1 + s, c2),
                        point(m2, c2)
                    );
                } else {
                    // Turns in just before the child, or before its tags when it
                    // comes from their side.
                    let to = if parent.lane < view.lane && m2 - lead[view.index] > m1 {
                        m2 - lead[view.index]
                    } else {
                        m2
                    };
                    let mut s = CURVE.min(to - m1);
                    let mut along = c1;
                    if !merged || busy(parent.lane, m1, m2) {
                        // Neither lane is free to run along. Between two lanes,
                        // past the ids of one and before the tags of the next, a
                        // strip stays empty, and the connector takes that.
                        along = if c1 > c2 {
                            c2 + hi[view.lane] + GAP / 2.0
                        } else {
                            c2 - lo[view.lane] - GAP / 2.0
                        };
                        s = CURVE.min((to - m1) / 2.0);
                        let _ = write!(
                            d,
                            "C{} {} {}",
                            point(m1 + s / 2.0, c1),
                            point(m1 + s / 2.0, along),
                            point(m1 + s, along)
                        );
                    }
                    let _ = write!(
                        d,
                        "L{}C{} {} {}",
                        point(to - s, along),
                        point(to - s / 2.0, along),
                        point(to - s / 2.0, c2),
                        point(to, c2)
                    );
                    if to < m2 {
                        let _ = write!(d, "L{}", point(m2, c2));
                    }
                }
                let class = fragment(|f| {
                    if merged {
                        write!(f, " {p}-edge-merge")
                    } else if picked {
                        write!(f, " {p}-edge-cherry-pick")
                    } else {
                        Ok(())
                    }
                });
                let _ = write!(
                    curves,
                    "<path class=\"{p}-edge{class}\" d=\"{d}\" stroke=\"{}\"{}/>",
                    series_color(if merged { parent.lane } else { view.lane } as i64),
                    if picked {
                        " stroke-dasharray=\"4 3\""
                    } else {
                        ""
                    }
                );
            }
        }

        let mut commits = String::new();
        for view in &views {
            let commit = view.commit;
            let at = point(view.m, cross[view.lane]);
            let Point { x, y } = at;
            let kind = commit.custom_type.unwrap_or(commit.kind);
            bounds.grow(
                Point::new(x - DOT - 1.0, y - DOT - 1.0),
                Point::new(x + DOT + 1.0, y + DOT + 1.0),
            );
            let written = !commit.kind.makes_up_message() && !commit.message.is_empty();
            let merge = commit.kind == CommitType::Merge && kind != CommitType::Merge;
            let _ = write!(
                commits,
                "<g class=\"{p}-commit {p}-commit-{}{}\" data-id=\"{}\">",
                kind.class(),
                fragment(|f| if merge {
                    write!(f, " {p}-commit-merge")
                } else {
                    Ok(())
                }),
                EscText(&commit.id)
            );
            if written {
                let _ = write!(commits, "<title>{}</title>", EscText(&commit.message));
            }
            let _ = write!(
                commits,
                "{}",
                kind.symbol(at, series_color(view.lane as i64))
            );

            if let Some(label) = &view.label
                && !label.lines.is_empty()
            {
                let w = label.width;
                let h = label.height;
                let back = |centre: Point| {
                    fragment(move |f| {
                        write!(
                            f,
                            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\" fill=\"var(--_bg)\"/>",
                            num(centre.x - w / 2.0 - 2.0),
                            num(centre.y - h / 2.0 + 2.0),
                            num(w + 4.0),
                            num(h - 4.0)
                        )
                    })
                };
                if rotate {
                    // The text ends beside the commit and trails back the way the
                    // history came.
                    let k = (HALF * h) / 2.0;
                    let ax = if vertical { x - DOT - 3.0 - k } else { x + 3.0 };
                    let ay = if vertical { y } else { y + DOT + 3.0 + k };
                    let reach = HALF * w + k;
                    bounds.grow(
                        Point::new(ax - reach, if flip { ay - reach } else { ay - k }),
                        Point::new(ax + k, if flip { ay + k } else { ay + reach }),
                    );
                    let centre = Point::new(ax - w / 2.0, ay);
                    let _ = write!(
                        commits,
                        "<g class=\"{p}-commit-label\" transform=\"rotate({} {} {})\">{}{}</g>",
                        if flip { 45 } else { -45 },
                        num(ax),
                        num(ay),
                        back(centre),
                        label.svg(centre, "")
                    );
                } else {
                    let centre = if vertical {
                        Point::new(x - DOT - 6.0 - w / 2.0, y)
                    } else {
                        Point::new(x, y + DOT + 6.0 + h / 2.0)
                    };
                    bounds.grow_around(centre, Size::new(w, h));
                    let _ = write!(
                        commits,
                        "<g class=\"{p}-commit-label\">{}{}</g>",
                        back(centre),
                        label.svg(centre, "")
                    );
                }
            }

            // Beside the lane the tags form a row; above it they stack, the first
            // one furthest out.
            let mut offset = DOT + TAG_GAP;
            let order: Vec<usize> = if vertical {
                (0..view.tags.len()).collect()
            } else {
                (0..view.tags.len()).rev().collect()
            };
            for t in order {
                let tag = &view.tags[t];
                let w = tag_width(tag);
                let h = tag.height;
                let left = if vertical { x + offset } else { x - w / 2.0 };
                let cy = if vertical { y } else { y - offset - h / 2.0 };
                offset += if vertical { w + 4.0 } else { h + 3.0 };
                bounds.grow(
                    Point::new(left, cy - h / 2.0),
                    Point::new(left + w, cy + h / 2.0),
                );
                let _ = write!(
                    commits,
                    "<g class=\"{p}-tag\"><path d=\"M{},{}L{},{}H{}V{}H{}Z\" fill=\"var(--_s)\" stroke=\"var(--_b)\" stroke-linejoin=\"round\"/><circle cx=\"{}\" cy=\"{}\" r=\"1.5\" fill=\"var(--_b)\"/>{}</g>",
                    num(left),
                    num(cy),
                    num(left + NOTCH),
                    num(cy - h / 2.0),
                    num(left + w),
                    num(cy + h / 2.0),
                    num(left + NOTCH),
                    num(left + NOTCH),
                    num(cy),
                    tag.svg(
                        Point::new(left + NOTCH + 4.0 + tag.width / 2.0, cy),
                        " fill=\"var(--_fg)\""
                    )
                );
            }

            commits.push_str("</g>");
        }
        if bounds.min.x > bounds.max.x {
            bounds.grow(Point::default(), Point::default());
        }
        let Bounds {
            min: Point { x: min_x, y: min_y },
            max: Point { x: max_x, y: max_y },
        } = bounds;
        let title = Label::layout(
            self.titles.title.as_deref(),
            false,
            measure,
            size,
            4000.0,
            Style::BOLD,
        );
        let title_height = title.spaced_height(16.0);
        let inner = (max_x - min_x).max(title.width);
        let width = (inner + 2.0 * pad).ceil();
        let height = (max_y - min_y + title_height + 2.0 * pad).ceil();
        let dx = pad - min_x + (inner - (max_x - min_x)) / 2.0;
        let dy = pad + title_height - min_y;

        let title_attrs = crate::svg::builder::TITLE;
        let mut content = String::with_capacity(
            branches.len() + curves.len() + lines.len() + commits.len() + 512,
        );
        let _ = write!(
            content,
            "{}",
            title.svg(
                Point::new(width / 2.0, pad + title.height / 2.0),
                title_attrs
            )
        );
        let _ = write!(
            content,
            "<g transform=\"translate({},{})\">",
            num(dx),
            num(dy)
        );
        if !branches.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-branches\" fill=\"none\" font-size=\"{}\">{branches}</g>",
                js::Number(medium)
            );
        }
        if !curves.is_empty() || !lines.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-edges\" fill=\"none\" stroke-width=\"2\" stroke-linecap=\"round\">{curves}{lines}</g>"
            );
        }
        if !commits.is_empty() {
            let _ = write!(
                content,
                "<g class=\"{p}-commits\" font-size=\"{}\" fill=\"var(--_m)\">{commits}</g>",
                js::Number(small)
            );
        }
        content.push_str("</g>");

        Rendered {
            svg: svg_document(
                "gitGraph",
                width,
                height,
                size,
                ctx,
                self.titles.accessible(),
                &content,
            ),
            width,
            height,
            links: Vec::new(),
        }
    }
}

//! Resolves what the parser read into a model, once the whole diagram is
//! read: which ids are actors, use cases, boundaries, JSON nodes or edges, in
//! what order they appear, and whether the statements about them agree.
//! Ports Mermaid's UsecaseModelBuilder, with its messages. Pele's
//! `diagrams/usecase/db.ts`.

use crate::diagrams::common::db::acc_descr;
use indexmap::{IndexMap, IndexSet};

use super::{
    lexer::Source,
    types::{
        Actor, ActorType, Animation, Arrow, BoundaryType, ClassDef, Direction, EdgeOccurrence,
        GraphStatement, LabelType, MetadataOccurrence, Relationship, RelationshipType, Span,
        StatementKind, SymbolKind, SystemBoundary, UseCase, UseCaseShape, UsecaseJsonNode,
        UsecaseModel, UsecaseNote,
    },
};
use crate::{config::Value, error::Error};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DraftLabel {
    pub text: String,
    pub kind: LabelType,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Text(String),
    Bool(bool),
}

impl PropertyValue {
    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Bool(_) => None,
        }
    }

    fn bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            Self::Text(_) => None,
        }
    }

    /// The actor type a `type` value names.
    fn actor_type(&self) -> Option<ActorType> {
        match self.text()? {
            "normal" => Some(ActorType::Normal),
            "hollow" => Some(ActorType::Hollow),
            "awesome" => Some(ActorType::Awesome),
            _ => None,
        }
    }

    /// The boundary type a `type` value names.
    fn boundary_type(&self) -> Option<BoundaryType> {
        match self.text()? {
            "rect" => Some(BoundaryType::Rect),
            "package" => Some(BoundaryType::Package),
            _ => None,
        }
    }

    fn animation(&self) -> Option<Animation> {
        match self.text()? {
            "fast" => Some(Animation::Fast),
            "slow" => Some(Animation::Slow),
            _ => None,
        }
    }
}

/// `key_span` is also where an error about the property points.
#[derive(Clone, Debug, PartialEq)]
pub struct DraftProperty {
    pub key: String,
    pub value: PropertyValue,
    pub span: Span,
    pub key_span: Span,
    pub value_span: Span,
}

impl DraftProperty {
    pub fn occurrence(&self) -> MetadataOccurrence {
        MetadataOccurrence {
            key: self.key.clone(),
            span: self.span,
            key_span: self.key_span,
            value_span: self.value_span,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    Actor,
    Usecase,
}

impl ElementKind {
    fn symbol(self) -> SymbolKind {
        match self {
            Self::Actor => SymbolKind::Actor,
            Self::Usecase => SymbolKind::Usecase,
        }
    }
}

/// What every mention of an id carries: where it is, and whether the id was
/// made from a label.
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    pub id: String,
    pub label: DraftLabel,
    pub span: Span,
    pub generated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftElement {
    pub named: Named,
    pub kind: ElementKind,
    pub parent: Option<DraftBoundary>,
    pub shape: Option<UseCaseShape>,
    pub metadata: Option<Vec<DraftProperty>>,
    pub stereotype: Option<String>,
    pub classes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftBoundary {
    pub named: Named,
    pub classes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftJson {
    pub id: String,
    pub value: Value,
    pub property_order: IndexMap<String, Vec<String>>,
    pub span: Span,
    pub classes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftEndpoint {
    pub named: Named,
    /// Whether this mention declares the element.
    pub declares: bool,
    /// Whether it carries a `:::` suffix.
    pub classed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftRelationship {
    pub source: DraftEndpoint,
    pub target: DraftEndpoint,
    pub span: Span,
    pub id: Option<String>,
    pub id_span: Option<Span>,
    pub kind: RelationshipType,
    pub arrow_type: Arrow,
    pub label: Option<DraftLabel>,
    pub minlen: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftNote {
    pub target: String,
    pub span: Span,
    pub label: DraftLabel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftMetadata {
    pub target: String,
    pub span: Span,
    pub properties: Vec<DraftProperty>,
    /// The top-level statement it was written in, where that is a metadata
    /// statement.
    pub statement: Option<usize>,
}

impl DraftMetadata {
    /// The only kind of element the keys could belong to, for the error
    /// message.
    fn implied_kind(&self) -> Option<SymbolKind> {
        let mut possible: IndexSet<SymbolKind> = [
            SymbolKind::Actor,
            SymbolKind::Usecase,
            SymbolKind::Boundary,
            SymbolKind::Edge,
        ]
        .into();
        for property in &self.properties {
            match property.key.as_str() {
                "icon" => {
                    possible.clear();
                    possible.insert(SymbolKind::Actor);
                }
                "animate" | "animation" => {
                    possible.clear();
                    possible.insert(SymbolKind::Edge);
                }
                "type" => {
                    possible.clear();
                    if property.value.boundary_type().is_some() {
                        possible.insert(SymbolKind::Boundary);
                    } else if property.value.actor_type().is_some() {
                        possible.insert(SymbolKind::Actor);
                    }
                }
                "business" => {
                    possible.shift_remove(&SymbolKind::Boundary);
                    possible.shift_remove(&SymbolKind::Edge);
                }
                _ => return None,
            }
        }
        if possible.len() == 1 {
            possible.first().copied()
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftClasses {
    pub targets: Vec<(String, Span)>,
    pub classes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftStyle {
    pub target: String,
    pub span: Span,
    pub styles: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftClassDef {
    pub ids: Vec<String>,
    pub styles: Vec<String>,
}

/// Everything the parser read, in source order, before any of it is
/// resolved.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drafts {
    pub elements: Vec<DraftElement>,
    pub boundaries: Vec<DraftBoundary>,
    pub jsons: Vec<DraftJson>,
    pub relationships: Vec<DraftRelationship>,
    pub notes: Vec<DraftNote>,
    pub metadata: Vec<DraftMetadata>,
    pub classes: Vec<DraftClasses>,
    pub styles: Vec<DraftStyle>,
    pub class_defs: Vec<DraftClassDef>,
    pub direction: Option<Direction>,
    pub acc_title: String,
    pub acc_description: String,
}

struct Origin {
    kind: SymbolKind,
    span: Span,
    generated: bool,
    /// The label a generated id was derived from.
    label: Option<String>,
}

struct ElementState {
    named: Named,
    kind: ElementKind,
    parent: Option<DraftBoundary>,
    shape: Option<UseCaseShape>,
    stereotype: Option<String>,
    stereotype_span: Option<Span>,
    classes: Vec<String>,
    actor_type: Option<ActorType>,
    icon: Option<String>,
    business: Option<bool>,
}

struct BoundaryState {
    draft: DraftBoundary,
    kind: Option<BoundaryType>,
    members: Vec<String>,
}

fn push_unique(target: &mut Vec<String>, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let mut seen: IndexSet<String> = target.iter().cloned().collect();
    for value in values {
        if seen.insert(value.clone()) {
            target.push(value.clone());
        }
    }
}

/// The element a class or style statement names.
#[derive(Clone, Copy)]
enum Stylable {
    Actor,
    Usecase,
    Boundary,
    Json,
    Edge(usize),
}

struct Resolver<'a> {
    src: &'a Source,
    model: UsecaseModel,
    symbols: IndexMap<String, Origin>,
    elements: IndexMap<String, ElementState>,
    boundaries: IndexMap<String, BoundaryState>,
    /// Relationships by id, as indices into `model.relationships`.
    edges: IndexMap<String, usize>,
    /// The offset at which each id is first mentioned, which decides the
    /// order of the model.
    first: IndexMap<String, usize>,
}

impl Resolver<'_> {
    fn at(&self, span: Span) -> String {
        self.src.location_text(span[0], span[1])
    }

    fn error(&self, message: String, span: Span) -> Error {
        self.src.place(Error::semantic(message), span[0])
    }

    fn fail(&self, message: &str, span: Span) -> Error {
        self.error(format!("{message} at {}", self.at(span)), span)
    }

    fn conflict(
        &self,
        message: &str,
        current: Span,
        previous: Span,
        current_label: Option<&str>,
        previous_label: Option<&str>,
    ) -> Error {
        let suffix = |label: Option<&str>| match label {
            None => String::new(),
            Some(label) => format!(" (label \"{label}\")"),
        };
        self.error(
            format!(
                "{message} at {}{}; previous declaration at {}{}",
                self.at(current),
                suffix(current_label),
                self.at(previous),
                suffix(previous_label)
            ),
            current,
        )
    }

    fn invalid(&self, id: &str, kind: &str, property: &DraftProperty) -> Error {
        self.fail(
            &format!(
                "Metadata property '{}' is invalid for {kind} '{id}'",
                property.key
            ),
            property.key_span,
        )
    }

    fn mention(&mut self, id: &str, offset: usize) {
        match self.first.get(id) {
            Some(&previous) if offset >= previous => {}
            _ => {
                self.first.insert(id.to_string(), offset);
            }
        }
    }

    /// The ids, ordered by where each is first mentioned.
    fn in_order<'k>(&self, ids: impl Iterator<Item = &'k String>) -> Vec<String> {
        let mut ids: Vec<String> = ids.cloned().collect();
        ids.sort_by_key(|id| self.first.get(id).copied().unwrap_or(0));
        ids
    }

    fn register(&mut self, named: &Named, kind: SymbolKind) {
        self.symbols.insert(
            named.id.clone(),
            Origin {
                kind,
                span: named.span,
                generated: named.generated,
                label: Some(named.label.text.clone()),
            },
        );
    }

    /// An id means one thing in the whole diagram, and an id made from a
    /// label may be declared once.
    fn claim(&self, draft: &Named, kind: SymbolKind) -> Result<(), Error> {
        let Some(origin) = self.symbols.get(&draft.id) else {
            return Ok(());
        };
        let label = draft.generated.then_some(draft.label.text.as_str());
        let origin_label = if origin.generated {
            origin.label.as_deref()
        } else {
            None
        };
        if origin.kind != kind {
            return Err(self.conflict(
                &format!(
                    "ID '{}' is declared as both {} and {}",
                    draft.id,
                    origin.kind.name(),
                    kind.name()
                ),
                draft.span,
                origin.span,
                label,
                origin_label,
            ));
        }
        if origin.generated || draft.generated {
            return Err(self.conflict(
                &format!(
                    "Generated ID '{}' collides with another declaration",
                    draft.id
                ),
                draft.span,
                origin.span,
                label,
                origin_label,
            ));
        }
        Ok(())
    }

    fn unique(&mut self, id: &str, kind: SymbolKind, span: Span) -> Result<(), Error> {
        if let Some(previous) = self.symbols.get(id) {
            return Err(self.conflict(
                &format!(
                    "ID '{id}' is declared more than once ({} and {})",
                    previous.kind.name(),
                    kind.name()
                ),
                span,
                previous.span,
                None,
                if previous.generated {
                    previous.label.as_deref()
                } else {
                    None
                },
            ));
        }
        self.symbols.insert(
            id.to_string(),
            Origin {
                kind,
                span,
                generated: false,
                label: None,
            },
        );
        Ok(())
    }

    /// A metadata statement replaces what a declaration said; two
    /// declarations have to agree.
    fn actor_property(
        &self,
        state: &mut ElementState,
        property: &DraftProperty,
        replace: bool,
    ) -> Result<(), Error> {
        let key = property.key.as_str();
        let value = &property.value;
        let clash = |differs: bool| -> Result<(), Error> {
            if !replace && differs {
                return Err(self.conflict(
                    &format!("Actor '{}' has conflicting {key} metadata", state.named.id),
                    property.key_span,
                    state.named.span,
                    None,
                    None,
                ));
            }
            Ok(())
        };
        if key == "type"
            && let Some(kind) = value.actor_type()
        {
            clash(state.actor_type.is_some_and(|previous| previous != kind))?;
            state.actor_type = Some(kind);
        } else if key == "icon"
            && let Some(icon) = value.text()
        {
            clash(
                state
                    .icon
                    .as_deref()
                    .is_some_and(|previous| previous != icon),
            )?;
            state.icon = Some(icon.to_string());
        } else if key == "business"
            && let Some(business) = value.bool()
        {
            clash(state.business.is_some_and(|previous| previous != business))?;
            state.business = Some(business);
        } else {
            return Err(self.invalid(&state.named.id, "actor", property));
        }
        Ok(())
    }

    fn element_metadata(
        &self,
        state: &mut ElementState,
        properties: Option<&[DraftProperty]>,
        replace: bool,
    ) -> Result<(), Error> {
        for property in properties.unwrap_or_default() {
            if state.kind == ElementKind::Actor {
                self.actor_property(state, property, replace)?;
            } else if property.key == "business"
                && let Some(business) = property.value.bool()
            {
                if !replace && state.business.is_some_and(|previous| previous != business) {
                    return Err(self.conflict(
                        &format!(
                            "Use case '{}' has conflicting business metadata",
                            state.named.id
                        ),
                        property.key_span,
                        state.named.span,
                        None,
                        None,
                    ));
                }
                state.business = Some(business);
            } else {
                return Err(self.invalid(&state.named.id, "usecase", property));
            }
        }
        Ok(())
    }

    /// Runs `f` on the state of element `id`, taken out of the map for the
    /// while.
    fn with_element<R>(
        &mut self,
        id: &str,
        f: impl FnOnce(&Self, &mut ElementState) -> Result<R, Error>,
    ) -> Result<R, Error> {
        let Some(at) = self.elements.get_index_of(id) else {
            unreachable!("an element that was declared");
        };
        let (_, mut state) = self.elements.swap_remove_index(at).expect("present");
        let out = f(self, &mut state);
        let (last, _) = self.elements.insert_full(id.to_string(), state);
        self.elements.swap_indices(at, last);
        out
    }

    fn publish(&mut self, id: &str) {
        let state = &self.elements[id];
        let model = &mut self.model;
        let parent_id = state
            .parent
            .as_ref()
            .map(|parent| parent.named.id.clone())
            .filter(|id| !id.is_empty());
        let stereotype = state.stereotype.clone().filter(|s| !s.is_empty());
        if state.kind == ElementKind::Actor {
            model.use_cases.shift_remove(id);
            let styles = model
                .actors
                .get(id)
                .map(|actor| actor.styles.clone())
                .unwrap_or_default();
            let icon = state.icon.clone().filter(|icon| !icon.is_empty());
            model.actors.insert(
                id.to_string(),
                Actor {
                    id: id.to_string(),
                    label: state.named.label.text.clone(),
                    label_type: state.named.label.kind,
                    business: state.business.unwrap_or(false),
                    classes: state.classes.clone(),
                    kind: if icon.is_some() {
                        ActorType::Icon
                    } else {
                        state.actor_type.unwrap_or(ActorType::Normal)
                    },
                    styles,
                    icon,
                    stereotype,
                    parent_id,
                },
            );
        } else {
            model.actors.shift_remove(id);
            let styles = model
                .use_cases
                .get(id)
                .map(|use_case| use_case.styles.clone())
                .unwrap_or_default();
            model.use_cases.insert(
                id.to_string(),
                UseCase {
                    id: id.to_string(),
                    label: state.named.label.text.clone(),
                    label_type: state.named.label.kind,
                    business: state.business.unwrap_or(false),
                    classes: state.classes.clone(),
                    shape: state.shape.unwrap_or(UseCaseShape::Ellipse),
                    styles,
                    stereotype,
                    parent_id,
                },
            );
        }
    }

    fn declare_element(&mut self, draft: &DraftElement) -> Result<(), Error> {
        self.claim(&draft.named, draft.kind.symbol())?;
        let parent = draft
            .parent
            .clone()
            .filter(|parent| !parent.named.id.is_empty());
        if !self.elements.contains_key(&draft.named.id) {
            let mut state = ElementState {
                named: draft.named.clone(),
                kind: draft.kind,
                parent,
                shape: draft.shape,
                stereotype: draft.stereotype.clone(),
                stereotype_span: draft
                    .stereotype
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|_| draft.named.span),
                classes: draft.classes.clone(),
                actor_type: None,
                icon: None,
                business: None,
            };
            self.element_metadata(&mut state, draft.metadata.as_deref(), false)?;
            self.elements.insert(draft.named.id.clone(), state);
            self.register(&draft.named, draft.kind.symbol());
            return Ok(());
        }
        let id = draft.named.id.clone();
        self.with_element(&id, |this, existing| {
            if existing.named.label.text != draft.named.label.text
                || existing.named.label.kind != draft.named.label.kind
            {
                return Err(this.conflict(
                    &format!("ID '{id}' has conflicting labels"),
                    draft.named.span,
                    existing.named.span,
                    None,
                    None,
                ));
            }
            if let (Some(shape), Some(previous)) = (draft.shape, existing.shape)
                && shape != previous
            {
                return Err(this.conflict(
                    &format!("Use case '{id}' has conflicting shapes"),
                    draft.named.span,
                    existing.named.span,
                    None,
                    None,
                ));
            }
            // A boundary whose title makes an empty id is no parent, but it
            // still takes the place of one.
            if let Some(parent) = &parent
                && let Some(previous) = existing
                    .parent
                    .as_ref()
                    .filter(|previous| !previous.named.id.is_empty())
                && parent.named.id != previous.named.id
            {
                return Err(this.conflict(
                    &format!("Element '{id}' belongs to more than one system boundary"),
                    parent.named.span,
                    previous.named.span,
                    None,
                    None,
                ));
            }
            if let (Some(stereotype), Some(previous)) = (
                draft.stereotype.as_deref().filter(|s| !s.is_empty()),
                existing.stereotype.as_deref().filter(|s| !s.is_empty()),
            ) && stereotype != previous
            {
                return Err(this.conflict(
                    &format!("Element '{id}' has conflicting stereotypes"),
                    draft.named.span,
                    existing.stereotype_span.unwrap_or(existing.named.span),
                    None,
                    None,
                ));
            }
            if existing.shape.is_none() {
                existing.shape = draft.shape;
            }
            if existing.parent.is_none() {
                existing.parent = draft.parent.clone();
            }
            if existing.stereotype.is_none() {
                existing.stereotype = draft.stereotype.clone();
            }
            if existing.stereotype_span.is_none() {
                existing.stereotype_span = draft
                    .stereotype
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .map(|_| draft.named.span);
            }
            push_unique(&mut existing.classes, &draft.classes);
            this.element_metadata(existing, draft.metadata.as_deref(), false)
        })
    }

    fn declare_boundary(&mut self, draft: &DraftBoundary) -> Result<(), Error> {
        self.claim(&draft.named, SymbolKind::Boundary)?;
        if let Some(existing) = self.boundaries.get(&draft.named.id) {
            if existing.draft.named.label.text != draft.named.label.text
                || existing.draft.named.label.kind != draft.named.label.kind
            {
                return Err(self.conflict(
                    &format!("Boundary '{}' has conflicting titles", draft.named.id),
                    draft.named.span,
                    existing.draft.named.span,
                    None,
                    None,
                ));
            }
            let existing = self.boundaries.get_mut(&draft.named.id).expect("present");
            push_unique(&mut existing.draft.classes, &draft.classes);
            return Ok(());
        }
        self.boundaries.insert(
            draft.named.id.clone(),
            BoundaryState {
                draft: draft.clone(),
                kind: None,
                members: Vec::new(),
            },
        );
        self.register(&draft.named, SymbolKind::Boundary);
        Ok(())
    }

    /// An end of a relationship that nothing declares becomes a use case.
    fn endpoint(&mut self, end: &DraftEndpoint) -> Result<SymbolKind, Error> {
        if end.classed && !end.declares {
            return Err(self.fail(
                &format!(
                    "Relationship endpoint '{}' uses ::: without declaring the node",
                    end.named.id
                ),
                end.named.span,
            ));
        }
        if let Some(origin) = self.symbols.get(&end.named.id) {
            return Ok(origin.kind);
        }
        self.elements.insert(
            end.named.id.clone(),
            ElementState {
                named: end.named.clone(),
                kind: ElementKind::Usecase,
                parent: None,
                shape: Some(UseCaseShape::Ellipse),
                stereotype: None,
                stereotype_span: None,
                classes: Vec::new(),
                actor_type: None,
                icon: None,
                business: None,
            },
        );
        self.register(&end.named, SymbolKind::Usecase);
        self.mention(&end.named.id, end.named.span[0]);
        Ok(SymbolKind::Usecase)
    }

    fn stylable(&self, id: &str, span: Span) -> Result<Stylable, Error> {
        let model = &self.model;
        let found = match self.symbols.get(id).map(|origin| origin.kind) {
            Some(SymbolKind::Actor) => model.actors.contains_key(id).then_some(Stylable::Actor),
            Some(SymbolKind::Usecase) => model
                .use_cases
                .contains_key(id)
                .then_some(Stylable::Usecase),
            Some(SymbolKind::Boundary) => model
                .system_boundaries
                .contains_key(id)
                .then_some(Stylable::Boundary),
            Some(SymbolKind::Json) => model.json_nodes.contains_key(id).then_some(Stylable::Json),
            _ => self.edges.get(id).map(|&at| Stylable::Edge(at)),
        };
        found.ok_or_else(|| {
            self.fail(
                &format!("Class/style target '{id}' is unresolved or anonymous"),
                span,
            )
        })
    }

    fn style_lists(&mut self, id: &str, target: Stylable) -> (&mut Vec<String>, &mut Vec<String>) {
        let model = &mut self.model;
        match target {
            Stylable::Actor => {
                let actor = model.actors.get_mut(id).expect("present");
                (&mut actor.classes, &mut actor.styles)
            }
            Stylable::Usecase => {
                let use_case = model.use_cases.get_mut(id).expect("present");
                (&mut use_case.classes, &mut use_case.styles)
            }
            Stylable::Boundary => {
                let boundary = model.system_boundaries.get_mut(id).expect("present");
                (&mut boundary.classes, &mut boundary.styles)
            }
            Stylable::Json => {
                let json = model.json_nodes.get_mut(id).expect("present");
                (&mut json.classes, &mut json.styles)
            }
            Stylable::Edge(at) => {
                let edge = &mut model.relationships[at];
                (&mut edge.classes, &mut edge.styles)
            }
        }
    }
}

impl Drafts {
    /// The model of everything read, or the first disagreement among it.
    pub fn resolve(
        self,
        src: &Source,
        header_span: Span,
        mut statements: Vec<GraphStatement>,
        title: Option<&str>,
    ) -> Result<UsecaseModel, Error> {
        let mut r = Resolver {
            src,
            model: UsecaseModel::empty(),
            symbols: IndexMap::new(),
            elements: IndexMap::new(),
            boundaries: IndexMap::new(),
            edges: IndexMap::new(),
            first: IndexMap::new(),
        };

        enum Declaration<'d> {
            Element(&'d DraftElement),
            Boundary(&'d DraftBoundary),
            Json(&'d DraftJson),
            Edge(&'d str, Span),
        }
        let mut declarations: Vec<(usize, Declaration)> = Vec::new();
        for relationship in &self.relationships {
            r.mention(
                &relationship.source.named.id,
                relationship.source.named.span[0],
            );
            r.mention(
                &relationship.target.named.id,
                relationship.target.named.span[0],
            );
        }
        for draft in &self.elements {
            r.mention(&draft.named.id, draft.named.span[0]);
            declarations.push((draft.named.span[0], Declaration::Element(draft)));
        }
        for draft in &self.boundaries {
            r.mention(&draft.named.id, draft.named.span[0]);
            declarations.push((draft.named.span[0], Declaration::Boundary(draft)));
        }
        for draft in &self.jsons {
            r.mention(&draft.id, draft.span[0]);
            declarations.push((draft.span[0], Declaration::Json(draft)));
        }
        for relationship in &self.relationships {
            if let (Some(id), Some(id_span)) = (
                relationship.id.as_deref().filter(|id| !id.is_empty()),
                relationship.id_span,
            ) {
                declarations.push((id_span[0], Declaration::Edge(id, id_span)));
            }
        }
        declarations.sort_by_key(|(offset, _)| *offset);
        for (_, declaration) in &declarations {
            match declaration {
                Declaration::Element(draft) => r.declare_element(draft)?,
                Declaration::Boundary(draft) => r.declare_boundary(draft)?,
                Declaration::Json(draft) => r.unique(&draft.id, SymbolKind::Json, draft.span)?,
                Declaration::Edge(id, span) => r.unique(id, SymbolKind::Edge, *span)?,
            }
        }

        for id in r.in_order(r.elements.keys()) {
            r.publish(&id);
        }
        for id in r.in_order(r.boundaries.keys()) {
            let state = &r.boundaries[&id];
            r.model.system_boundaries.insert(
                id.clone(),
                SystemBoundary {
                    id: id.clone(),
                    label: state.draft.named.label.text.clone(),
                    label_type: state.draft.named.label.kind,
                    kind: BoundaryType::Rect,
                    members: Vec::new(),
                    classes: Vec::new(),
                    styles: Vec::new(),
                },
            );
        }
        let mut jsons: Vec<&DraftJson> = self.jsons.iter().collect();
        jsons.sort_by_key(|draft| r.first.get(&draft.id).copied().unwrap_or(0));
        for draft in jsons {
            r.model.json_nodes.insert(
                draft.id.clone(),
                UsecaseJsonNode {
                    id: draft.id.clone(),
                    value: draft.value.clone(),
                    property_order: draft.property_order.clone(),
                    classes: draft.classes.clone(),
                    styles: Vec::new(),
                },
            );
        }

        let mut anonymous = 0;
        for draft in &self.relationships {
            let (source, target, kind) = (&draft.source, &draft.target, draft.kind);
            let source_kind = r.endpoint(source)?;
            let target_kind = r.endpoint(target)?;
            let related = |kind: SymbolKind| {
                matches!(
                    kind,
                    SymbolKind::Actor | SymbolKind::Usecase | SymbolKind::Json
                )
            };
            if !related(source_kind) {
                return Err(r.conflict(
                    &format!(
                        "Relationship source '{}' cannot be {}",
                        source.named.id,
                        source_kind.name()
                    ),
                    source.named.span,
                    r.symbols[&source.named.id].span,
                    None,
                    None,
                ));
            }
            if !related(target_kind) {
                return Err(r.conflict(
                    &format!(
                        "Relationship target '{}' cannot be {}",
                        target.named.id,
                        target_kind.name()
                    ),
                    target.named.span,
                    r.symbols[&target.named.id].span,
                    None,
                    None,
                ));
            }
            if matches!(kind, RelationshipType::Include | RelationshipType::Extend)
                && (source_kind != SymbolKind::Usecase || target_kind != SymbolKind::Usecase)
            {
                return Err(r.fail(
                    &format!("{} relationship requires use-case endpoints", kind.name()),
                    draft.span,
                ));
            }
            if kind == RelationshipType::Generalization
                && (source_kind == SymbolKind::Json || source_kind != target_kind)
            {
                return Err(r.fail(
                    "Generalization requires actor-to-actor or use-case-to-use-case endpoints",
                    draft.span,
                ));
            }
            if kind == RelationshipType::Association
                && (source_kind == SymbolKind::Json || target_kind == SymbolKind::Json)
                && draft.arrow_type.is_decorated()
            {
                return Err(r.fail(
                    &format!(
                        "JSON relationship '{}' to '{}' permits only point, reversed-point, or markerless solid association",
                        source.named.id, target.named.id
                    ),
                    draft.span,
                ));
            }
            let id = match draft.id.as_deref() {
                Some(id) => id.to_string(),
                None => {
                    anonymous += 1;
                    format!("edge-{}", anonymous - 1)
                }
            };
            let mut relationship = Relationship {
                id: id.clone(),
                explicit_id: draft.id.as_deref().is_some_and(|id| !id.is_empty()),
                source: source.named.id.clone(),
                target: target.named.id.clone(),
                kind,
                arrow_type: draft.arrow_type,
                label: None,
                label_type: None,
                minlen: draft.minlen,
                classes: Vec::new(),
                styles: Vec::new(),
                animate: false,
                animation: None,
            };
            if let Some(label) = &draft.label {
                relationship.label = Some(label.text.clone());
                relationship.label_type = Some(label.kind);
            }
            r.model.relationships.push(relationship);
            r.edges.insert(id, r.model.relationships.len() - 1);
        }

        // The relationships may have added use cases; put all elements back in
        // the order of first mention.
        let declared_actors = std::mem::take(&mut r.model.actors);
        let declared_use_cases = std::mem::take(&mut r.model.use_cases);
        for id in r.in_order(r.elements.keys()) {
            if let Some(actor) = declared_actors.get(&id) {
                r.model.actors.insert(id, actor.clone());
            } else if let Some(use_case) = declared_use_cases.get(&id) {
                r.model.use_cases.insert(id, use_case.clone());
            } else {
                r.publish(&id);
            }
        }

        for metadata in &self.metadata {
            let target = metadata.target.as_str();
            let Some(origin) = r.symbols.get(target) else {
                let inferred = metadata.implied_kind().map_or(String::new(), |kind| {
                    format!(" (metadata implies {})", kind.name())
                });
                return Err(r.fail(
                    &format!("Metadata target '{target}' is unresolved{inferred}"),
                    metadata.span,
                ));
            };
            let kind = origin.kind;
            if kind == SymbolKind::Actor || kind == SymbolKind::Usecase {
                r.with_element(target, |this, state| {
                    this.element_metadata(state, Some(&metadata.properties), true)
                })?;
            } else if kind == SymbolKind::Edge
                && let Some(at) = metadata.statement
            {
                let statement = &mut statements[at];
                statement.kind = StatementKind::EdgeMetadata;
                statement.edges = Some(vec![EdgeOccurrence {
                    id: target.to_string(),
                    span: statement.span,
                    label_span: None,
                    id_span: Some(metadata.span),
                    metadata: statement.metadata.clone(),
                }]);
                statement.nodes = None;
            }
            if kind == SymbolKind::Actor || kind == SymbolKind::Usecase {
                continue;
            }
            let has_boundary = r.boundaries.contains_key(target);
            let edge = r.edges.get(target).copied();
            for property in &metadata.properties {
                let key = property.key.as_str();
                let value = &property.value;
                if kind == SymbolKind::Boundary
                    && has_boundary
                    && key == "type"
                    && let Some(boundary_type) = value.boundary_type()
                {
                    r.boundaries.get_mut(target).expect("present").kind = Some(boundary_type);
                } else if kind == SymbolKind::Edge
                    && let Some(at) = edge
                    && key == "animate"
                    && let Some(animate) = value.bool()
                {
                    r.model.relationships[at].animate = animate;
                } else if kind == SymbolKind::Edge
                    && let Some(at) = edge
                    && key == "animation"
                    && let Some(animation) = value.animation()
                {
                    let relationship = &mut r.model.relationships[at];
                    relationship.animation = Some(animation);
                    relationship.animate = true;
                } else {
                    return Err(r.invalid(target, kind.name(), property));
                }
            }
        }
        for relationship in &mut r.model.relationships {
            if relationship.animation.is_some() {
                relationship.animate = true;
            }
        }

        let ids: Vec<String> = r.elements.keys().cloned().collect();
        for id in ids {
            let state = &r.elements[&id];
            if state.kind == ElementKind::Actor {
                if state.icon.as_deref().is_some_and(|icon| !icon.is_empty())
                    && let Some(kind) = state.actor_type.filter(|&kind| kind != ActorType::Normal)
                {
                    return Err(r.fail(
                        &format!(
                            "Actor '{id}' cannot combine icon with type '{}'",
                            kind.name()
                        ),
                        state.named.span,
                    ));
                }
                if state.business == Some(true)
                    && (state.icon.as_deref().is_some_and(|icon| !icon.is_empty())
                        || state.actor_type == Some(ActorType::Awesome))
                {
                    return Err(r.fail(
                        &format!("Business actor '{id}' must use normal or hollow geometry"),
                        state.named.span,
                    ));
                }
            } else if state.shape == Some(UseCaseShape::Rect) && state.business == Some(true) {
                return Err(r.fail(
                    &format!("Rectangular use case '{id}' cannot be a business use case"),
                    state.named.span,
                ));
            }
            r.publish(&id);
        }

        let mut elements: Vec<&DraftElement> = self.elements.iter().collect();
        elements.sort_by_key(|draft| draft.named.span[0]);
        for draft in elements {
            let Some(parent) = draft
                .parent
                .as_ref()
                .filter(|parent| !parent.named.id.is_empty())
            else {
                continue;
            };
            let members = &mut r
                .boundaries
                .get_mut(&parent.named.id)
                .expect("a declared boundary")
                .members;
            if !members.contains(&draft.named.id) {
                members.push(draft.named.id.clone());
            }
        }
        for (id, state) in &r.boundaries {
            let boundary = r
                .model
                .system_boundaries
                .get_mut(id)
                .expect("a published boundary");
            boundary.kind = state.kind.unwrap_or(BoundaryType::Rect);
            boundary.members = state.members.clone();
            boundary.classes = state.draft.classes.clone();
        }

        for (n, draft) in self.notes.iter().enumerate() {
            let Some(origin) = r.symbols.get(&draft.target) else {
                return Err(r.fail(
                    &format!("Note target '{}' is unresolved", draft.target),
                    draft.span,
                ));
            };
            if origin.kind != SymbolKind::Actor && origin.kind != SymbolKind::Usecase {
                return Err(r.conflict(
                    &format!(
                        "Note target '{}' must be an actor or use case, not {}",
                        draft.target,
                        origin.kind.name()
                    ),
                    draft.span,
                    origin.span,
                    None,
                    None,
                ));
            }
            let id = format!("note-{n}");
            r.model.notes.insert(
                id.clone(),
                UsecaseNote {
                    id,
                    target: draft.target.clone(),
                    label: draft.label.text.clone(),
                    label_type: draft.label.kind,
                },
            );
        }

        for definition in &self.class_defs {
            for id in &definition.ids {
                r.model.class_defs.insert(
                    id.clone(),
                    ClassDef {
                        id: id.clone(),
                        styles: definition.styles.clone(),
                    },
                );
            }
        }
        for assignment in &self.classes {
            for (id, span) in &assignment.targets {
                let target = r.stylable(id, *span)?;
                let (classes, _) = r.style_lists(id, target);
                push_unique(classes, &assignment.classes);
            }
        }
        for assignment in &self.styles {
            let target = r.stylable(&assignment.target, assignment.span)?;
            let (_, styles) = r.style_lists(&assignment.target, target);
            styles.extend(assignment.styles.iter().cloned());
        }

        let mut model = r.model;
        model.direction = self.direction.unwrap_or(model.direction);
        model.title = title.map(str::to_string);
        model.acc_title = self.acc_title;
        model.acc_descr = acc_descr(&self.acc_description);
        model.acc_description = self.acc_description;
        model.source = src.text.clone();
        model.header_span = header_span;
        model.statements = statements;
        Ok(model)
    }
}

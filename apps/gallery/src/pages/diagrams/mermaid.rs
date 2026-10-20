//! One page per diagram type `bezel-mermaid` draws, and an overview.
//!
//! Each page is a one-fence document handed to [`markdown::render`], the
//! diagram first and its source under it as plain text.

use gpui::{AnyElement, Context, ScrollHandle, Window, div, prelude::*, px};
use markdown::Doc;
use motion::Painter;
use theme::Theme;
use ui::scroll::{self, Axes, TransientState};

use crate::{Gallery, hint, stack};

/// One diagram page: its rail key, the line above it, and its source.
pub struct Sample {
    pub key: &'static str,
    pub hint: &'static str,
    pub source: &'static str,
}

const fn sample(key: &'static str, hint: &'static str, source: &'static str) -> Sample {
    Sample { key, hint, source }
}

/// The overview page: prose with a fence in it, as a document carries one.
pub const OVERVIEW: &str = r#"A fence tagged `mermaid` is drawn as its diagram, in the theme's colours and fonts.

```mermaid
flowchart LR
  source[Markdown] --> parse --> doc[Document]
  doc --> fence{mermaid fence?}
  fence -- yes --> draw[bezel-mermaid] --> svg[SVG]
  fence -- no --> code[Code block]
```

Every page under it is one diagram type and the source that draws it."#;

/// One row per `mermaid-` page in [`crate::DIAGRAMS`], in rail order.
pub const SAMPLES: &[Sample] = &[
    sample(
        "mermaid-flowchart",
        "Nodes and edges, laid out in layers. Subgraphs group nodes; the header names the direction.",
        r#"flowchart LR
  subgraph Client
    UI[Web app]
    Cache[(Local cache)]
  end
  subgraph Services
    API[API gateway]
    Auth[Auth service]
    Orders[Order service]
  end
  subgraph Storage
    DB[(Orders DB)]
  end
  UI --> API
  UI --> Cache
  API --> Auth
  API --> Orders
  Orders --> DB
  Auth -. token .-> UI"#,
    ),
    sample(
        "mermaid-sequence",
        "Messages between participants over time, with activations, notes and numbering.",
        r#"sequenceDiagram
  autonumber
  actor Customer
  participant Web as Web app
  participant API as API gateway
  participant Bank
  Customer->>Web: Place order
  Web->>API: POST /orders
  activate API
  API->>Bank: Authorise payment
  Bank-->>API: Approved
  API-->>Web: 201 Created
  deactivate API
  Web-->>Customer: Order confirmed
  Note over Customer,Bank: One order, one transaction"#,
    ),
    sample(
        "mermaid-class",
        "Classes with their members, and the relations between them.",
        r#"classDiagram
    Animal <|-- Duck
    Animal <|-- Fish
    Animal <|-- Zebra
    Animal : +int age
    Animal : +String gender
    Animal: +isMammal()
    Animal: +mate()
    class Duck{
        +String beakColor
        +swim()
        +quack()
    }
    class Fish{
        -int sizeInFeet
        -canEat()
    }
    class Zebra{
        +bool is_wild
        +run()
    }"#,
    ),
    sample(
        "mermaid-state",
        "States and the transitions between them, with composite states and concurrent regions.",
        r#"stateDiagram-v2
    [*] --> Idle
    Idle --> Loading : fetch
    Loading --> Ready : resolved
    Loading --> Failed : rejected
    Failed --> Loading : retry
    state Ready {
        [*] --> Fresh
        Fresh --> Stale : timeout
        Stale --> Fresh : refresh
    }
    Ready --> [*]"#,
    ),
    sample(
        "mermaid-er",
        "Entities, their attributes, and the cardinality of each relationship.",
        r#"erDiagram
    CUSTOMER ||--o{ ORDER : places
    ORDER ||--|{ LINE-ITEM : contains
    CUSTOMER }|..|{ DELIVERY-ADDRESS : uses
    CUSTOMER {
        string name
        string email
    }
    ORDER {
        int id
        date placed
    }"#,
    ),
    sample(
        "mermaid-requirement",
        "Requirements and the elements that satisfy, verify or trace them.",
        r#"requirementDiagram
    requirement test_req {
    id: 1
    text: the test text.
    risk: high
    verifymethod: test
    }

    element test_entity {
    type: simulation
    }

    test_entity - satisfies -> test_req"#,
    ),
    sample(
        "mermaid-architecture",
        "Services and groups with icons, joined at the side each edge leaves and enters.",
        r#"architecture-beta
    group api(cloud)[API]

    service db(database)[Database] in api
    service disk1(disk)[Storage] in api
    service disk2(disk)[Storage] in api
    service server(server)[Server] in api

    db:L -- R:server
    disk1:T -- B:server
    disk2:T -- B:db"#,
    ),
    sample(
        "mermaid-block",
        "Blocks placed on a grid of columns, rather than by a layout.",
        r#"block
  columns 3
  Frontend blockArrowId6<[" "]>(right) Backend
  space:2 down<[" "]>(down)
  Disk left<[" "]>(left) Database[("Database")]"#,
    ),
    sample(
        "mermaid-c4",
        "The C4 model's people, systems and boundaries, and the relations between them.",
        r#"C4Context
  title System context for internet banking
  Person(customer, "Banking customer", "A customer of the bank")
  Enterprise_Boundary(bank, "Bank") {
    System(banking, "Internet banking", "Lets customers view their accounts")
    System_Ext(mail, "E-mail system", "Microsoft Exchange")
  }
  Rel(customer, banking, "Uses")
  Rel(banking, mail, "Sends e-mail", "SMTP")"#,
    ),
    sample(
        "mermaid-usecase",
        "Actors and the use cases they take part in, inside a system boundary.",
        r#"usecase-beta
direction LR
actor Customer("Customer")
actor Clerk("Clerk")
systemBoundary "Order system"
  Browse("Browse catalogue")
  Checkout("Place order")
  Refund("Issue refund")
end
Customer --> Browse
Customer --> Checkout
Clerk --> Refund"#,
    ),
    sample(
        "mermaid-swimlane",
        "A flowchart whose subgraphs are lanes, one per party.",
        r#"swimlane-beta LR
  subgraph Customer
    Browse[Browse catalogue]
    Pay[Pay]
  end
  subgraph Warehouse
    Pick[Pick items]
    Ship[Ship order]
  end
  subgraph Finance
    Invoice[Raise invoice]
  end
  Browse --> Pay
  Pay --> Pick
  Pick --> Ship
  Pay --> Invoice"#,
    ),
    sample(
        "mermaid-agentflow",
        "An agent's steps, tools and decisions, grouped into flows.",
        r#"agentflow-beta TB
  flow reviewer["Review Agent"]
    changes["Gather changes"]@{ shape: input }
    analyse["Analyse diff"]@{ shape: task }
    lint["run_linter"]@{ shape: tool }
    ok["Clean?"]@{ shape: decision }

    changes --> analyse --> lint --> ok
  end"#,
    ),
    sample(
        "mermaid-eventmodeling",
        "An event model: screens, commands and events along a timeline.",
        r#"eventmodeling

timeframe 01 ui CartUI
timeframe 02 command AddItem
timeframe 03 event ItemAdded

resetframe 04 event External.InventoryChanged
timeframe 05 processor InventoryProcessor
timeframe 06 command ChangeInventory
timeframe 07 event Cart.InventoryChanged"#,
    ),
    sample(
        "mermaid-journey",
        "A user's journey through tasks in sections, each scored and attributed to actors.",
        r#"journey
    title My working day
    section Go to work
      Make tea: 5: Me
      Go upstairs: 3: Me
      Do work: 1: Me, Cat
    section Go home
      Go downstairs: 5: Me
      Sit down: 5: Me"#,
    ),
    sample(
        "mermaid-gitgraph",
        "Commits, branches and merges, drawn as a history.",
        r#"gitGraph
   commit
   commit
   branch develop
   checkout develop
   commit
   commit
   checkout main
   merge develop
   commit
   commit"#,
    ),
    sample(
        "mermaid-pie",
        "Slices of a whole, labelled with their share.",
        r#"pie title Pets adopted by volunteers
    "Dogs" : 386
    "Cats" : 85
    "Rats" : 15"#,
    ),
    sample(
        "mermaid-xychart",
        "Bars and lines over a category or numeric x axis.",
        r#"xychart
    title "Sales Revenue"
    x-axis [jan, feb, mar, apr, may, jun, jul, aug, sep, oct, nov, dec]
    y-axis "Revenue (in $)" 4000 --> 11000
    bar [5000, 6000, 7500, 8200, 9500, 10500, 11000, 10200, 9200, 8500, 7000, 6000]
    line [5000, 6000, 7500, 8200, 9500, 10500, 11000, 10200, 9200, 8500, 7000, 6000]"#,
    ),
    sample(
        "mermaid-quadrant",
        "Points placed on two axes split into four named quadrants.",
        r#"quadrantChart
    title Reach and engagement of campaigns
    x-axis Low Reach --> High Reach
    y-axis Low Engagement --> High Engagement
    quadrant-1 We should expand
    quadrant-2 Need to promote
    quadrant-3 Re-evaluate
    quadrant-4 May be improved
    Campaign A: [0.3, 0.6]
    Campaign B: [0.45, 0.23]
    Campaign C: [0.57, 0.69]
    Campaign D: [0.78, 0.34]
    Campaign E: [0.40, 0.34]
    Campaign F: [0.35, 0.78]"#,
    ),
    sample(
        "mermaid-radar",
        "Curves over radial axes, one value per axis.",
        r#"radar-beta
  axis m["Math"], s["Science"], e["English"]
  axis h["History"], g["Geography"], a["Art"]
  curve a["Alice"]{85, 90, 80, 70, 75, 90}
  curve b["Bob"]{70, 75, 85, 80, 90, 85}

  max 100
  min 0"#,
    ),
    sample(
        "mermaid-sankey",
        "Flows between nodes, each as wide as its value. The source is CSV rows.",
        r#"sankey

Solar,Electricity,60
Wind,Electricity,45
Gas,Electricity,30
Gas,Heating,50
Electricity,Homes,70
Electricity,Industry,65
Heating,Homes,50"#,
    ),
    sample(
        "mermaid-treemap",
        "Nested rectangles, each sized by its value.",
        r#"treemap-beta
"Frontend"
    "Components": 40
    "Styles": 15
    "Tests": 20
"Backend"
    "API": 35
    "Database": 25
    "Jobs": 10"#,
    ),
    sample(
        "mermaid-venn",
        "Sets and the unions where they overlap.",
        r#"venn-beta
  title What makes a good feature
  set Desirable
  set Feasible
  set Viable
  union Desirable,Feasible["Buildable"]
  union Feasible,Viable["Sustainable"]
  union Desirable,Viable["Marketable"]
  union Desirable,Feasible,Viable["Ship it"]"#,
    ),
    sample(
        "mermaid-packet",
        "A packet's fields laid out by bit range.",
        r#"packet
0-15: "Source Port"
16-31: "Destination Port"
32-63: "Sequence Number"
64-95: "Acknowledgment Number"
96-99: "Data Offset"
100-105: "Reserved"
106: "URG"
107: "ACK"
108: "PSH"
109: "RST"
110: "SYN"
111: "FIN"
112-127: "Window""#,
    ),
    sample(
        "mermaid-gantt",
        "Tasks on a calendar, in sections, each after another or from a date.",
        r#"gantt
    title A Gantt Diagram
    dateFormat YYYY-MM-DD
    section Section
        A task          :a1, 2014-01-01, 30d
        Another task    :after a1, 20d
    section Another
        Task in Another :2014-01-12, 12d
        another task    :24d"#,
    ),
    sample(
        "mermaid-timeline",
        "Events grouped by period, in order.",
        r#"timeline
    title History of Social Media Platform
    2002 : LinkedIn
    2004 : Facebook
         : Google
    2005 : YouTube
    2006 : Twitter"#,
    ),
    sample(
        "mermaid-kanban",
        "Columns of task cards.",
        r#"kanban
  todo[To do]
    spec[Write the spec]
    tests[Add tests]
  doing[In progress]
    parser[Port the parser]
  done[Done]
    layout[Layered layout]"#,
    ),
    sample(
        "mermaid-mindmap",
        "A tree of ideas around a root, its shape set by indentation.",
        r#"mindmap
  root((mindmap))
    Origins
      Long history
      Popularisation
        British popular psychology author Tony Buzan
    Research
      On effectiveness<br/>and features
      On Automatic creation
        Uses
            Creative techniques
            Strategic planning
            Argument mapping
    Tools
      Pen and paper
      Mermaid"#,
    ),
    sample(
        "mermaid-treeview",
        "A file tree, written as the box-drawing output of `tree`.",
        r#"treeView-beta
├── src/
│   ├── index.ts
│   └── utils.ts
├── package.json
└── README.md"#,
    ),
    sample(
        "mermaid-ishikawa",
        "A fishbone: an effect, and its causes by category.",
        r#"ishikawa-beta
    Blurry Photo
    Process
        Out of focus
        Shutter speed too slow
    User
        Shaky hands
    Equipment
        Lens
            Damaged lens
            Dirty lens
        Sensor
            Dirty sensor
    Environment
        Subject moved
        Low light"#,
    ),
    sample(
        "mermaid-wardley",
        "A value chain plotted against evolution, with the moves planned for it.",
        r#"wardley-beta
title Tea Shop Value Chain

anchor Business [0.95, 0.63]
component Cup of Tea [0.79, 0.61]
component Tea [0.63, 0.81]
component Hot Water [0.52, 0.80]
component Kettle [0.43, 0.35]
component Power [0.10, 0.70]

Business -> Cup of Tea
Cup of Tea -> Tea
Cup of Tea -> Hot Water
Hot Water -> Kettle
Kettle -> Power

evolve Kettle 0.62
evolve Power 0.89"#,
    ),
    sample(
        "mermaid-cynefin",
        "Items sorted into the Cynefin framework's domains.",
        r#"cynefin-beta
  title Incident Response

  complex
    "Investigate root cause"
    "Run chaos experiment"

  complicated
    "Analyze performance data"
    "Expert review needed"

  clear
    "Restart service"
    "Apply known fix"

  chaotic
    "Page on-call immediately"

  confusion
    "Unknown failure mode""#,
    ),
    sample(
        "mermaid-railroad",
        "A grammar drawn as railroad tracks, from EBNF, PEG or combinator rules.",
        r#"railroad-ebnf-beta
title "Arithmetic Expression Grammar"

expression = term ( ( "+" | "-" ) term )* ;
term = factor ( ( "*" | "/" ) factor )* ;
factor = number | "(" expression ")" ;
number = digit+ ;
digit = "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" ;"#,
    ),
];

/// One page's parsed documents and its scroll, built once.
struct Page {
    key: &'static str,
    hint: Option<&'static str>,
    diagram: Doc,
    source: Option<Doc>,
    scroll: ScrollHandle,
    bar: TransientState,
}

/// What the Mermaid pages hold between frames.
pub(crate) struct State {
    pages: Vec<Page>,
}

impl State {
    pub(crate) fn new(cx: &mut Context<Gallery>) -> Self {
        let overview = Page {
            key: "mermaid",
            hint: None,
            diagram: markdown::parse(OVERVIEW),
            source: None,
            scroll: ScrollHandle::new(),
            bar: TransientState::new(Painter::of(cx)),
        };
        let samples = SAMPLES.iter().map(|sample| Page {
            key: sample.key,
            hint: Some(sample.hint),
            diagram: markdown::parse(&format!("```mermaid\n{}\n```", sample.source)),
            source: Some(markdown::parse(&format!("```text\n{}\n```", sample.source))),
            scroll: ScrollHandle::new(),
            bar: TransientState::new(Painter::of(cx)),
        });
        Self {
            pages: std::iter::once(overview).chain(samples).collect(),
        }
    }
}

impl Gallery {
    pub(crate) fn mermaid(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let theme = Theme::of(cx).clone();
        let page = self
            .diagrams
            .mermaid
            .pages
            .iter()
            .find(|page| page.key == key)?;
        let column = stack()
            .max_w(px(860.0))
            .when_some(page.hint, |column, copy| column.child(hint(&theme, copy)))
            .child(markdown::render(
                &page.diagram,
                markdown::Caption::Shown,
                window,
                cx,
            ))
            .when_some(page.source.as_ref(), |column, source| {
                column.child(markdown::render(
                    source,
                    markdown::Caption::Shown,
                    window,
                    cx,
                ))
            });
        Some(
            div()
                .relative()
                .size_full()
                .child(
                    scroll::pane("mermaid-page", Axes::Vertical)
                        .size_full()
                        .track_scroll(&page.scroll)
                        .child(column),
                )
                .child(scroll::transient(
                    "mermaid-bar",
                    &page.scroll,
                    &page.bar,
                    cx.reduce_motion(),
                ))
                .into_any_element(),
        )
    }
}

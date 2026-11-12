//! The shape of the parameter vector Λ as data: groups, fields, ranges and help.
//! Pure data (no UI), shared by the desktop app and the website's parameter reference, so a label is written once.

/// A text of the schema: always English.
pub type Text = String;

/// `a` + separator + `b`.
fn join(a: &str, separator: &str, b: &str) -> Text {
    format!("{a}{separator}{b}")
}

fn tx(en: &str) -> Text {
    en.to_string()
}

#[derive(Clone, Debug)]
pub enum FieldKind {
    Number { min: f64, max: f64, integer: bool },
    Boolean,
    Choice(Vec<(String, Text)>),
}

#[derive(Clone, Debug)]
pub struct ParamField {
    pub path: String,
    pub label: Text,
    pub help: Text,
    pub kind: FieldKind,
}

#[derive(Clone, Debug)]
pub struct ParamGroup {
    pub id: String,
    pub title: Text,
    pub intro: Text,
    pub fields: Vec<ParamField>,
    pub open_by_default: bool,
}

fn number(path: &str, label: Text, min: f64, max: f64, integer: bool) -> ParamField {
    ParamField {
        path: path.into(),
        help: help(path, &label),
        label,
        kind: FieldKind::Number { min, max, integer },
    }
}

fn boolean(path: &str, label: Text) -> ParamField {
    ParamField {
        path: path.into(),
        help: help(path, &label),
        label,
        kind: FieldKind::Boolean,
    }
}

fn choice(path: &str, label: Text, options: &[(&str, Text)]) -> ParamField {
    ParamField {
        path: path.into(),
        help: help(path, &label),
        label,
        kind: FieldKind::Choice(
            options
                .iter()
                .map(|(value, name)| ((*value).into(), name.clone()))
                .collect(),
        ),
    }
}

fn group(id: &str, title: Text, intro: Text, fields: Vec<ParamField>) -> ParamGroup {
    ParamGroup {
        id: id.into(),
        title,
        intro,
        fields,
        open_by_default: matches!(id, "shapes" | "graph"),
    }
}

fn class_fields(prefix: &str, title: Text, range: (f64, f64), integer: bool) -> Vec<ParamField> {
    [
        ("rect", tx("rectangular")),
        ("disc", tx("circular")),
        ("oct", tx("octagonal")),
        ("L", tx("L-shaped")),
        ("star", tx("truncated star")),
    ]
    .into_iter()
    .map(|(class, label)| {
        number(
            &format!("{prefix}.{class}"),
            join(&title, " · ", &label),
            range.0,
            range.1,
            integer,
        )
    })
    .collect()
}

pub fn groups() -> Vec<ParamGroup> {
    let mut shapes = class_fields("weights", tx("Weight"), (0.0, 1000.0), false);
    shapes.extend(class_fields("maxOf", tx("Max"), (0.0, 10000.0), true));
    shapes.extend([
        number(
            "starCells",
            tx("Minimum cells for a star"),
            1.0,
            1000.0,
            true,
        ),
        number(
            "homophily",
            tx("Preference for neighboring shapes"),
            -1.0,
            1.0,
            false,
        ),
    ]);

    let mut corridors = vec![
        number("width", tx("Corridor width (px)"), 0.1, 10000.0, false),
        number("clear", tx("Clearance σ (px)"), 0.0, 1000.0, false),
        number("exit", tx("Exit stretch (px)"), 0.1, 10000.0, false),
        number("seg", tx("Minimum segment (px)"), 0.1, 10000.0, false),
        number(
            "minLen",
            tx("Minimum distance between rooms (px)"),
            0.1,
            10000.0,
            false,
        ),
        choice(
            "maxBends",
            tx("Maximum bends"),
            &[
                ("0", tx("0")),
                ("1", tx("1")),
                ("2", tx("2")),
                ("4", tx("4")),
                ("5", tx("5")),
                ("6", tx("6")),
            ],
        ),
    ];
    corridors.extend(
        [
            ("pI", tx("Weight · straight I")),
            ("pL", tx("Weight · one L bend")),
            ("pZ", tx("Weight · two bends Z/U")),
            ("pW", tx("Weight · W detour")),
            ("pT", tx("Weight · T junction")),
            ("pX", tx("Weight · X crossing")),
            ("costBend", tx("Cost per bend")),
            ("costLen", tx("Cost per pixel")),
            ("cost45", tx("Cost per 45° bend")),
            ("costFacing", tx("Bonus for facing doors")),
        ]
        .into_iter()
        .map(|(path, label)| number(path, label, 0.0, 1000.0, false)),
    );

    let classes = [
        ("rect", tx("rectangular")),
        ("disc", tx("circular")),
        ("oct", tx("octagonal")),
        ("L", tx("L-shaped")),
        ("star", tx("truncated star")),
    ];
    let mut mix = Vec::new();
    for (i, (a, a_label)) in classes.iter().enumerate() {
        for (b, b_label) in classes.iter().skip(i) {
            mix.push(number(
                &format!("mix.{a}-{b}"),
                join(a_label, " × ", b_label),
                0.0,
                1.0,
                false,
            ));
        }
    }

    let mut leaves = vec![number(
        "leaves",
        tx("Number of chambers"),
        0.0,
        1000.0,
        true,
    )];
    leaves.extend(class_fields(
        "leafClasses",
        tx("Weight · room"),
        (0.0, 1000.0),
        false,
    ));
    leaves.extend([
        number(
            "leafRoles.trap",
            tx("Relative chance · trap"),
            0.0,
            1000.0,
            false,
        ),
        number(
            "leafRoles.prize",
            tx("Relative chance · prize"),
            0.0,
            1000.0,
            false,
        ),
        number(
            "leafRoles.empty",
            tx("Relative chance · empty"),
            0.0,
            1000.0,
            false,
        ),
    ]);

    vec![
        group(
            "scaffold",
            tx("Grid and size"),
            tx("Defines the available space and how many blocks make up the floor."),
            vec![
                choice(
                    "layout",
                    tx("Layout"),
                    &[("grid", tx("Grid")), ("growth", tx("Growth (prototype)"))],
                ),
                number("cols", tx("Columns"), 1.0, 64.0, true),
                number("rows", tx("Rows"), 1.0, 64.0, true),
                number("cellW", tx("Cell width (px)"), 1.0, 5000.0, false),
                number("cellH", tx("Cell height (px)"), 1.0, 5000.0, false),
                number("gap", tx("Room inset (px)"), 0.0, 1000.0, false),
                number("big", tx("Large blocks"), 0.0, 1000.0, true),
                number("drop", tx("Fraction of free cells"), 0.0, 1.0, false),
                number("layoutJitter", tx("Grid irregularity"), 0.0, 0.45, false),
                boolean("rotateRooms", tx("Rotate polygonal shapes")),
            ],
        ),
        group(
            "shapes",
            tx("Room shapes"),
            tx("Higher weights make a shape more likely; limits control how many of each type."),
            shapes,
        ),
        group(
            "directions",
            tx("Directions and doors"),
            tx("Controls the directions allowed for corridors and how doors are sampled."),
            vec![
                choice("m", tx("Direction alphabet"), &[("4", tx("4 directions")), ("8", tx("8 directions"))]),
                choice(
                    "discPorts",
                    tx("Doors per circular room"),
                    &[("4", tx("4")), ("8", tx("8")), ("16", tx("16"))],
                ),
                number("portStep", tx("Step between doors (px)"), 0.1, 10000.0, false),
            ],
        ),
        group(
            "corridors",
            tx("Corridors and junctions"),
            tx("Measurements in pixels. Weights favor kinds of path; costs change the choice between valid routes."),
            corridors,
        ),
        group(
            "mix",
            tx("Shape compatibility"),
            tx("Each value multiplies the chance of connecting two room types. Zero forbids the link; one keeps the normal chance."),
            mix,
        ),
        group(
            "graph",
            tx("Connections and density"),
            tx("Picks candidate room pairs and how many cycles the generator tries to create."),
            vec![
                choice(
                    "proximity",
                    tx("How to choose pairs"),
                    &[("rng", tx("RNG")), ("gabriel", tx("Gabriel")), ("near", tx("Near"))],
                ),
                number("near", tx("Radius between centers (px)"), 0.1, 10000.0, false),
                number("delta", tx("Density δ"), 0.0, 1.0, false),
            ],
        ),
        group(
            "topology",
            tx("Topology profile"),
            tx("Pick a connectivity pattern or tune its requirements; the generator tries to satisfy them without breaking the geometric rules."),
            vec![
                choice(
                    "topology.profile",
                    tx("Profile"),
                    &[
                        ("classic", tx("Classic")),
                        ("backbone", tx("Backbone")),
                        ("hub", tx("Hub")),
                        ("redundantExit", tx("Redundant exit")),
                        ("longLoop", tx("Long loop")),
                    ],
                ),
                number("topology.minBranches", tx("Minimum side branches"), 0.0, 1000.0, true),
                number("topology.minBranchDepth", tx("Minimum branch depth"), 0.0, 1000.0, true),
                number("topology.minExitRoutes", tx("Independent routes to the stairs"), 0.0, 1000.0, true),
                number("topology.minCycleLength", tx("Minimum cycle length"), 0.0, 1000.0, true),
                number("topology.minHubDegree", tx("Minimum hub degree"), 0.0, 1000.0, true),
            ],
        ),
        group(
            "growth",
            tx("Growth (prototype)"),
            tx("Used by the Growth layout: instead of one room per grid block, rooms grow one from another, in free positions. The grid only sets the size of the world."),
            vec![
                number("growth.rooms", tx("Rooms (0 = as the grid)"), 0.0, 2000.0, true),
                number("growth.branch", tx("Children per room (1 to N)"), 1.0, 16.0, true),
                number("growth.depth", tx("Maximum depth"), 1.0, 200.0, true),
                number("growth.islands", tx("Islands"), 1.0, 16.0, true),
                number("growth.cluster", tx("Clustering (λ)"), -10.0, 10.0, false),
                number("growth.reach", tx("Corridor reach (cells)"), 0.0, 20.0, false),
            ],
        ),
        group(
            "roles",
            tx("Entrance, stairs and roles"),
            tx("Controls the stairs doors, the distance from the entrance and the number of ambushes."),
            vec![
                number("stairsDoors", tx("Maximum doors on the stairs"), 1.0, 1000.0, true),
                number("minHops", tx("Minimum steps to the stairs"), 1.0, 1000.0, true),
                number("ambush", tx("Ambush rooms"), 0.0, 1000.0, true),
            ],
        ),
        group(
            "leaves",
            tx("Isolated chambers"),
            tx("Extra rooms linked by a single corridor, like dead ends with a prize or a trap."),
            leaves,
        ),
    ]
}

fn help(path: &str, label: &str) -> Text {
    match path {
        "cols" => tx("Number of columns. More columns increase the potential width of the floor."),
        "rows" => tx("Number of rows. More rows increase the potential height of the floor."),
        "cellW" => tx("Width of each grid cell. Large blocks can span several cells."),
        "cellH" => tx("Height of each grid cell. Large blocks can span several cells."),
        "gap" => tx("Inset applied to rooms inside their blocks to keep space between neighbors."),
        "big" => tx("Maximum number of large blocks the generator tries to place."),
        "drop" => tx("Fraction of free cells reserved for possible leaf chambers."),
        "layoutJitter" => tx("Shifts the inner grid lines. The generator lowers the value if there is not enough space for a minimum room."),
        "rotateRooms" => tx("Allows polygonal shapes to rotate in steps of 90° or 45° in octilinear mode."),
        "homophily" => tx("A positive value brings rooms of the same shape closer; negative favors different shapes; zero ignores neighbors."),
        "m" => tx("4 allows 90° angles. 8 also allows 45° diagonals."),
        "discPorts" => tx("Number of possible directions on the circular edge. Directions outside the active alphabet are discarded."),
        "portStep" => tx("Distance between attempted door positions. A smaller step tries more positions and raises the cost."),
        "width" => tx("Physical width of each corridor."),
        "clear" => tx("Minimum distance between corridors and rooms or corridors they do not connect to."),
        "exit" => tx("Minimum length the corridor runs after leaving the door before turning."),
        "seg" => tx("Minimum length of each inner corridor segment."),
        "minLen" => tx("Requires both the path and the direct distance between connected rooms to reach this length."),
        "maxBends" => tx("Maximum number of bends. Values from 4 to 6 allow more winding W detours."),
        "pI" => tx("Relative weight of a straight link, without bends."),
        "pL" => tx("Relative weight of a link with one bend."),
        "pZ" => tx("Relative weight of paths with two bends, in Z or U."),
        "pW" => tx("Weight of W detours; there are alternatives only when at least four bends are allowed."),
        "pT" => tx("Relative weight for linking a room to an existing corridor and forming a T junction."),
        "pX" => tx("Relative weight for crossing an existing corridor and forming an X junction."),
        "costBend" => tx("Penalty added for each bend. A higher value favors straight paths."),
        "costLen" => tx("Penalty multiplied by the path length in pixels."),
        "cost45" => tx("Additional penalty for each diagonal 45° bend."),
        "costFacing" => tx("Discount when doors face each other and allow a straight link."),
        "proximity" => tx("RNG and Gabriel filter pairs by geometric proximity. Near uses a maximum radius."),
        "near" => tx("Maximum radius between room centers in Near mode; ignored in the other modes."),
        "delta" => tx("Fraction of the cycle reference the generator tries to reach. Higher values favor shortcuts and alternative routes."),
        "stairsDoors" => tx("Maximum number of corridors that can enter the stairs room."),
        "minHops" => tx("Minimum number of links between the entrance and the stairs, counted in rooms."),
        "ambush" => tx("Number of larger plain rooms that get the ambush role."),
        "topology.profile" => tx("Classic keeps the base behavior; other profiles require branches, a hub, redundant routes or long cycles."),
        "topology.minBranches" => tx("Minimum number of side components linked to the main path by a single passage."),
        "topology.minBranchDepth" => tx("Minimum number of rooms in a branch from the main path."),
        "topology.minExitRoutes" => tx("Minimum number of independent paths between the entrance and the stairs."),
        "topology.minCycleLength" => tx("Minimum number of segments in the shortest cycle. Zero imposes no such requirement."),
        "topology.minHubDegree" => tx("Minimum number of arms at a central point. Zero does not require a hub."),
        "layout" => tx("Grid puts one room in each grid block. Growth grows rooms one from another off a parent's door, in free space; isolated chambers are not generated in that mode."),
        "growth.rooms" => tx("Number of rooms to grow. Zero uses the number of live blocks the grid would give."),
        "growth.branch" => tx("Each room spawns from 1 up to this many children before the draw picks another parent."),
        "growth.depth" => tx("Deepest generation a room can have; the first room is 0."),
        "growth.islands" => tx("Splits the rooms into islands linked by long bridges. With more than one, cycles stay inside each island."),
        "growth.cluster" => tx("Positive concentrates new rooms where there are already many; negative spreads them; zero is neutral."),
        "growth.reach" => tx("Average corridor distance beyond the minimum, in grid cells."),
        "leaves" => tx("Number of extra chambers the generator tries to place in free cells."),
        "leafRoles.trap" => tx("Relative weight for a leaf chamber to get a trap."),
        "leafRoles.prize" => tx("Relative weight for a leaf chamber to get a prize."),
        "leafRoles.empty" => tx("Relative weight for a leaf chamber to stay empty."),
        _ if path.starts_with("weights.") => tx("Draw weight for rooms of this shape. It is relative to the other weights and does not need to sum to 1."),
        _ if path.starts_with("maxOf.") => tx("Limit of rooms of this shape. If the limits sum too low, the generator may exceed them."),
        _ if path.starts_with("leafClasses.") => tx("Relative weight for choosing this shape for leaf chambers."),
        _ if path.starts_with("mix.") => tx("Compatibility between two shapes: zero forbids the link; one keeps the normal chance."),
        _ if path.starts_with("leafRoles.") => tx("Relative weight for the leaf chamber to get this role."),
        _ => format!("{label}. Adjust this value and watch how it changes the floor layout."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_field_path_exists_in_lambda() {
        let lambda = serde_json::to_value(tapiti_core::params::rich_of_size(
            tapiti_core::params::Size::Medium,
        ))
        .unwrap();
        for group in groups() {
            for field in &group.fields {
                let found = field.path.split('.').try_fold(&lambda, |v, k| v.get(k));
                assert!(found.is_some(), "{} is not a parameter of Λ", field.path);
            }
        }
    }

    #[test]
    fn every_text_is_filled_and_groups_are_unique() {
        let mut seen = HashSet::new();
        for group in groups() {
            assert!(
                seen.insert(group.id.clone()),
                "duplicate group {}",
                group.id
            );
            let mut texts = vec![&group.title, &group.intro];
            for field in &group.fields {
                texts.push(&field.label);
                texts.push(&field.help);
                if let FieldKind::Choice(options) = &field.kind {
                    texts.extend(options.iter().map(|(_, name)| name));
                }
            }
            for text in texts {
                assert!(!text.is_empty(), "empty text in {}", group.id);
            }
        }
    }

    #[test]
    fn paths_are_unique() {
        let mut seen = HashSet::new();
        for field in groups().into_iter().flat_map(|g| g.fields) {
            assert!(
                seen.insert(field.path.clone()),
                "duplicate path {}",
                field.path
            );
        }
    }
}

use std::f64::consts::{PI, TAU};
use std::time::Duration;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

pub(super) const WIDTH: u16 = 15;
pub(super) const HEIGHT: u16 = 6;
#[cfg(test)]
pub(super) const REVEAL_DURATION: Duration = Duration::from_millis(800);
pub(super) const PREVIEW_TIME: Duration = Duration::from_millis(1_125);

const VIEWBOX_WIDTH: f64 = 205.0;
const VIEWBOX_HEIGHT: f64 = 171.0;
const CYCLE_MS: f64 = 3_600.0;
#[cfg(test)]
const COMPLETE_TOPOLOGY_CYCLE_MS: f64 = CYCLE_MS * 6.0;
const CONVERGENCE_END_MS: f64 = 900.0;
const FUSION_END_MS: f64 = 1_350.0;
const SEPARATION_END_MS: f64 = 2_400.0;
const INNER_RING_RADIUS: f64 = 16.5;
const ARM_SEGMENTS: usize = 12;
const BRANCH_SEGMENTS: usize = 6;
const ARM_REVEAL_MS: f64 = 600.0;
const REVEAL_STAGGER_MS: f64 = 35.0;
const SIGNAL_HALF_WIDTH: f64 = 0.075;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    x: f64,
    y: f64,
}

impl Point {
    const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    fn subtract(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    fn scale(self, factor: f64) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }
}

const HUB: Point = Point::new(102.5105, 82.4145);

#[derive(Clone, Copy)]
struct Arm {
    anchor: Point,
    half_width: f64,
}

const ARMS: [Arm; 6] = [
    Arm {
        anchor: Point::new(46.4455, 10.6772),
        half_width: 9.36,
    },
    Arm {
        anchor: Point::new(153.7495, 0.0),
        half_width: 4.47,
    },
    Arm {
        anchor: Point::new(205.0, 54.98695),
        half_width: 5.18,
    },
    Arm {
        anchor: Point::new(158.555, 158.555),
        half_width: 9.36,
    },
    Arm {
        anchor: Point::new(52.0506, 170.299),
        half_width: 4.47,
    },
    Arm {
        anchor: Point::new(0.0, 106.237),
        half_width: 5.18,
    },
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Layer {
    Back,
    Middle,
    Front,
    Signal,
    Synapse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LogoCell {
    symbol: char,
    layer: Option<Layer>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MotionPhase {
    Converging,
    Fused,
    Separating,
    Breathing,
}

#[derive(Clone, Copy)]
struct MotionState {
    phase: MotionPhase,
    routing_index: u64,
    inner_radius: f64,
    fusion: f64,
    routing_angle: f64,
    signal_u: f64,
    fusion_pulse: f64,
}

impl MotionState {
    fn at(elapsed_ms: f64) -> Self {
        let cycle_index = (elapsed_ms / CYCLE_MS).floor() as u64;
        let cycle_ms = elapsed_ms % CYCLE_MS;
        let base_angle = (cycle_index % 6) as f64 * PI / 3.0;

        if cycle_ms < CONVERGENCE_END_MS {
            let progress = smootherstep(cycle_ms / CONVERGENCE_END_MS);
            return Self {
                phase: MotionPhase::Converging,
                routing_index: cycle_index,
                inner_radius: INNER_RING_RADIUS * (1.0 - progress),
                fusion: progress,
                routing_angle: base_angle,
                signal_u: progress,
                fusion_pulse: 0.0,
            };
        }

        if cycle_ms < FUSION_END_MS {
            let progress = smootherstep(
                (cycle_ms - CONVERGENCE_END_MS) / (FUSION_END_MS - CONVERGENCE_END_MS),
            );
            return Self {
                phase: MotionPhase::Fused,
                routing_index: cycle_index,
                inner_radius: 0.0,
                fusion: 1.0,
                routing_angle: base_angle + progress * PI / 3.0,
                signal_u: 1.0,
                fusion_pulse: (PI * progress).sin().powi(2),
            };
        }

        if cycle_ms < SEPARATION_END_MS {
            let progress =
                smootherstep((cycle_ms - FUSION_END_MS) / (SEPARATION_END_MS - FUSION_END_MS));
            return Self {
                phase: MotionPhase::Separating,
                routing_index: cycle_index + 1,
                inner_radius: INNER_RING_RADIUS * progress,
                fusion: 1.0 - progress,
                routing_angle: base_angle + PI / 3.0,
                signal_u: 1.0 - progress,
                fusion_pulse: 0.0,
            };
        }

        let progress = (cycle_ms - SEPARATION_END_MS) / (CYCLE_MS - SEPARATION_END_MS);
        let breath = 1.0 + 0.06 * (TAU * progress).sin();
        Self {
            phase: MotionPhase::Breathing,
            routing_index: cycle_index + 1,
            inner_radius: INNER_RING_RADIUS * breath,
            fusion: 0.0,
            routing_angle: base_angle + PI / 3.0,
            signal_u: (PI * progress).sin().powi(2),
            fusion_pulse: 0.0,
        }
    }
}

#[derive(Clone, Copy)]
struct ArmPath {
    anchor: Point,
    control_one: Point,
    control_two: Point,
    inner: Point,
    normal: Point,
    depth: f64,
    half_width: f64,
}

impl ArmPath {
    fn point(self, u: f64) -> Point {
        let inverse = 1.0 - u;
        let curve = self
            .anchor
            .scale(inverse.powi(3))
            .add(self.control_one.scale(3.0 * inverse.powi(2) * u))
            .add(self.control_two.scale(3.0 * inverse * u.powi(2)))
            .add(self.inner.scale(u.powi(3)));
        curve.add(self.normal.scale(3.0 * self.depth * (PI * u).sin()))
    }

    fn tangent(self, u: f64) -> Point {
        let inverse = 1.0 - u;
        self.control_one
            .subtract(self.anchor)
            .scale(3.0 * inverse.powi(2))
            .add(
                self.control_two
                    .subtract(self.control_one)
                    .scale(6.0 * inverse * u),
            )
            .add(self.inner.subtract(self.control_two).scale(3.0 * u.powi(2)))
    }
}

#[derive(Clone, Copy)]
struct Segment {
    start: Point,
    end: Point,
    radius: f64,
    layer: Layer,
    reveal_ms: f64,
}

#[derive(Clone, Copy)]
struct Node {
    center: Point,
    radius: f64,
    layer: Layer,
    reveal_ms: f64,
}

struct Geometry {
    segments: Vec<Segment>,
    nodes: Vec<Node>,
}

pub(super) fn lines(elapsed: Duration, colors: bool, reveal_intro: bool) -> Vec<Line<'static>> {
    cell_rows(elapsed, reveal_intro)
        .into_iter()
        .map(|cells| {
            Line::from(
                cells
                    .into_iter()
                    .map(|cell| {
                        Span::styled(cell.symbol.to_string(), layer_style(cell.layer, colors))
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn layer_style(layer: Option<Layer>, colors: bool) -> Style {
    match layer {
        None => Style::default(),
        Some(Layer::Back) => Style::default().add_modifier(Modifier::DIM),
        Some(Layer::Middle) => Style::default(),
        Some(Layer::Front) if colors => Style::default().fg(Color::Cyan),
        Some(Layer::Front) => Style::default().add_modifier(Modifier::BOLD),
        Some(Layer::Signal) if colors => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        Some(Layer::Signal) => Style::default().add_modifier(Modifier::BOLD),
        Some(Layer::Synapse) if colors => Style::default()
            .fg(Color::LightCyan)
            .add_modifier(Modifier::BOLD),
        Some(Layer::Synapse) => Style::default()
            .add_modifier(Modifier::BOLD)
            .add_modifier(Modifier::REVERSED),
    }
}

#[cfg(test)]
fn symbol_rows(elapsed: Duration, reveal_intro: bool) -> Vec<String> {
    cell_rows(elapsed, reveal_intro)
        .into_iter()
        .map(|cells| cells.into_iter().map(|cell| cell.symbol).collect())
        .collect()
}

fn cell_rows(elapsed: Duration, reveal_intro: bool) -> Vec<Vec<LogoCell>> {
    let elapsed_ms = elapsed.as_secs_f64() * 1_000.0;
    let geometry = build_geometry(elapsed_ms);
    (0..usize::from(HEIGHT))
        .map(|cell_y| {
            (0..usize::from(WIDTH))
                .map(|cell_x| braille_cell(cell_x, cell_y, elapsed_ms, reveal_intro, &geometry))
                .collect()
        })
        .collect()
}

fn build_geometry(elapsed_ms: f64) -> Geometry {
    let state = MotionState::at(elapsed_ms);
    let mut segments = Vec::with_capacity(ARMS.len() * (ARM_SEGMENTS + BRANCH_SEGMENTS));
    let mut nodes = Vec::with_capacity(ARMS.len() * 3 + 1);

    for (index, arm) in ARMS.iter().copied().enumerate() {
        let path = arm_path(arm, state, elapsed_ms);
        let base_layer = depth_layer(path.depth);
        let delay = index as f64 * REVEAL_STAGGER_MS;

        let mut previous = path.point(0.0);
        for segment_index in 0..ARM_SEGMENTS {
            let u_end = (segment_index + 1) as f64 / ARM_SEGMENTS as f64;
            let u_middle = (segment_index as f64 + 0.5) / ARM_SEGMENTS as f64;
            let next = path.point(u_end);
            let layer = if (u_middle - state.signal_u).abs() <= SIGNAL_HALF_WIDTH {
                Layer::Signal
            } else if state.phase == MotionPhase::Fused {
                Layer::Synapse
            } else {
                base_layer
            };
            segments.push(Segment {
                start: previous,
                end: next,
                radius: path.half_width,
                layer,
                reveal_ms: delay + ARM_REVEAL_MS * u_end,
            });
            previous = next;
        }

        let signal_center = path.point(state.signal_u);
        nodes.push(Node {
            center: signal_center,
            radius: (path.half_width * 0.8).max(4.2),
            layer: if state.phase == MotionPhase::Fused {
                Layer::Synapse
            } else {
                Layer::Signal
            },
            reveal_ms: delay + ARM_REVEAL_MS * state.signal_u + 50.0,
        });

        nodes.push(Node {
            center: path.inner,
            radius: 3.6 + 2.5 * state.fusion,
            layer: if state.fusion > 0.72 {
                Layer::Synapse
            } else {
                Layer::Front
            },
            reveal_ms: delay + ARM_REVEAL_MS,
        });

        add_branch(&mut segments, &mut nodes, path, state, index, delay);
    }

    nodes.push(Node {
        center: HUB,
        radius: 3.6 + 7.5 * state.fusion + 2.0 * state.fusion_pulse,
        layer: if state.fusion > 0.35 {
            Layer::Synapse
        } else {
            Layer::Middle
        },
        reveal_ms: 650.0,
    });

    Geometry { segments, nodes }
}

fn arm_path(arm: Arm, state: MotionState, elapsed_ms: f64) -> ArmPath {
    let outward = normalize(arm.anchor.subtract(HUB));
    let inner = HUB.add(rotate(outward, state.routing_angle).scale(state.inner_radius));
    let direction = normalize(inner.subtract(arm.anchor));
    let normal = Point::new(-direction.y, direction.x);
    let distance = inner.subtract(arm.anchor);
    let depth = (TAU * elapsed_ms / 7_200.0 + outward.y.atan2(outward.x)).sin();
    ArmPath {
        anchor: arm.anchor,
        control_one: arm.anchor.add(distance.scale(0.34)),
        control_two: inner
            .subtract(distance.scale(0.23))
            .add(normal.scale(9.0 * state.fusion)),
        inner,
        normal,
        depth,
        half_width: arm.half_width * (1.0 + 0.12 * depth),
    }
}

fn add_branch(
    segments: &mut Vec<Segment>,
    nodes: &mut Vec<Node>,
    path: ArmPath,
    state: MotionState,
    index: usize,
    delay: f64,
) {
    let growth = (state.inner_radius / INNER_RING_RADIUS).clamp(0.0, 1.0);
    if growth <= 0.01 {
        return;
    }

    let root_u = 0.32 + 0.07 * (index % 3) as f64;
    let root = path.point(root_u);
    let tangent = normalize(path.tangent(root_u));
    let normal = Point::new(-tangent.y, tangent.x);
    let side = if (index as u64 + state.routing_index).is_multiple_of(2) {
        1.0
    } else {
        -1.0
    };
    let length = growth * (19.0 + 3.0 * (index % 3) as f64);
    let branch_direction = normalize(tangent.scale(0.35).add(normal.scale(0.94 * side)));
    let tip = root.add(branch_direction.scale(length));
    let control = root
        .add(tangent.scale(length * 0.22))
        .add(normal.scale(side * length * 0.38));
    let radius = (path.half_width * 0.42).max(2.8);
    let layer = depth_layer(path.depth);
    let reveal_ms = delay + ARM_REVEAL_MS * root_u;

    let mut previous = root;
    for segment_index in 0..BRANCH_SEGMENTS {
        let u = (segment_index + 1) as f64 / BRANCH_SEGMENTS as f64;
        let inverse = 1.0 - u;
        let next = root
            .scale(inverse.powi(2))
            .add(control.scale(2.0 * inverse * u))
            .add(tip.scale(u.powi(2)));
        segments.push(Segment {
            start: previous,
            end: next,
            radius,
            layer,
            reveal_ms,
        });
        previous = next;
    }
    nodes.push(Node {
        center: tip,
        radius: 3.8 * growth,
        layer: Layer::Front,
        reveal_ms,
    });
}

fn depth_layer(depth: f64) -> Layer {
    if depth < -0.28 {
        Layer::Back
    } else if depth > 0.28 {
        Layer::Front
    } else {
        Layer::Middle
    }
}

fn braille_cell(
    cell_x: usize,
    cell_y: usize,
    elapsed_ms: f64,
    reveal_intro: bool,
    geometry: &Geometry,
) -> LogoCell {
    const DOT_BITS: [[u8; 2]; 4] = [[0, 3], [1, 4], [2, 5], [6, 7]];
    let mut mask = 0_u8;
    let mut layer = None;

    for (dot_y, row) in DOT_BITS.iter().enumerate() {
        for (dot_x, bit) in row.iter().enumerate() {
            let pixel_x = cell_x * 2 + dot_x;
            let pixel_y = cell_y * 4 + dot_y;
            let point = Point::new(
                (pixel_x as f64 + 0.5) * VIEWBOX_WIDTH / f64::from(WIDTH * 2),
                (pixel_y as f64 + 0.5) * VIEWBOX_HEIGHT / f64::from(HEIGHT * 4),
            );
            let dot_layer = layer_at(point, elapsed_ms, reveal_intro, geometry);
            if let Some(dot_layer) = dot_layer {
                mask |= 1 << bit;
                layer = Some(layer.map_or(dot_layer, |value: Layer| value.max(dot_layer)));
            }
        }
    }

    LogoCell {
        symbol: if mask == 0 {
            ' '
        } else {
            char::from_u32(0x2800 + u32::from(mask)).unwrap_or(' ')
        },
        layer,
    }
}

fn layer_at(
    point: Point,
    elapsed_ms: f64,
    reveal_intro: bool,
    geometry: &Geometry,
) -> Option<Layer> {
    let mut layer = None;
    for segment in &geometry.segments {
        if (!reveal_intro || elapsed_ms >= segment.reveal_ms)
            && distance_to_segment(point, segment.start, segment.end) <= segment.radius
        {
            layer = Some(layer.map_or(segment.layer, |value: Layer| value.max(segment.layer)));
        }
    }
    for node in &geometry.nodes {
        if (!reveal_intro || elapsed_ms >= node.reveal_ms)
            && distance(point, node.center) <= node.radius
        {
            layer = Some(layer.map_or(node.layer, |value: Layer| value.max(node.layer)));
        }
    }
    layer
}

fn normalize(point: Point) -> Point {
    let length = (point.x * point.x + point.y * point.y).sqrt();
    if length <= f64::EPSILON {
        Point::new(0.0, 0.0)
    } else {
        point.scale(1.0 / length)
    }
}

fn rotate(point: Point, angle: f64) -> Point {
    let (sin, cos) = angle.sin_cos();
    Point::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos)
}

fn distance(a: Point, b: Point) -> f64 {
    let delta = a.subtract(b);
    (delta.x * delta.x + delta.y * delta.y).sqrt()
}

fn distance_to_segment(point: Point, start: Point, end: Point) -> f64 {
    let segment = end.subtract(start);
    let length_squared = segment.x * segment.x + segment.y * segment.y;
    if length_squared <= f64::EPSILON {
        return distance(point, start);
    }
    let relative = point.subtract(start);
    let projection =
        ((relative.x * segment.x + relative.y * segment.y) / length_squared).clamp(0.0, 1.0);
    distance(point, start.add(segment.scale(projection)))
}

fn smootherstep(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    value.powi(3) * (value * (value * 6.0 - 15.0) + 10.0)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        ARM_SEGMENTS, ARMS, COMPLETE_TOPOLOGY_CYCLE_MS, Geometry, HEIGHT, Layer, MotionState,
        PREVIEW_TIME, REVEAL_DURATION, WIDTH, arm_path, build_geometry, distance, symbol_rows,
    };

    #[test]
    fn reveal_grows_six_arms_into_a_complete_logo() {
        let empty = symbol_rows(Duration::ZERO, true);
        let midpoint = symbol_rows(Duration::from_millis(400), true);
        let complete = symbol_rows(REVEAL_DURATION, true);

        assert!(empty.iter().all(|line| line.trim().is_empty()));
        assert_ne!(midpoint, empty);
        assert_ne!(midpoint, complete);
        assert_eq!(complete.len(), usize::from(HEIGHT));
        assert!(complete.iter().any(|line| !line.trim().is_empty()));
        assert!(
            complete
                .iter()
                .all(|line| line.chars().count() == usize::from(WIDTH))
        );
    }

    #[test]
    fn network_converges_rewires_and_separates() {
        let separated = build_geometry(0.0);
        let fused = build_geometry(1_125.0);
        let rewired = build_geometry(2_400.0);

        assert!(separated.segments.len() > ARMS.len() * ARM_SEGMENTS);
        assert_eq!(fused.segments.len(), ARMS.len() * ARM_SEGMENTS);
        assert!(rewired.segments.len() > fused.segments.len());
        assert!(fused.nodes.iter().any(|node| node.layer == Layer::Synapse));
        assert_ne!(
            symbol_rows(Duration::ZERO, false),
            symbol_rows(Duration::from_millis(1_125), false)
        );
        assert_ne!(
            symbol_rows(Duration::ZERO, false),
            symbol_rows(Duration::from_millis(2_400), false)
        );
    }

    #[test]
    fn rewiring_preserves_every_external_anchor() {
        for elapsed_ms in [0.0, 1_125.0, 2_400.0] {
            let state = MotionState::at(elapsed_ms);
            for arm in ARMS {
                let path = arm_path(arm, state, elapsed_ms);
                assert!(distance(path.point(0.0), arm.anchor) < 0.000_001);
            }
        }
    }

    #[test]
    fn complete_topology_is_periodic_and_preview_contains_a_synapse() {
        let first = symbol_rows(PREVIEW_TIME, false);
        let repeated = symbol_rows(
            PREVIEW_TIME + Duration::from_millis(COMPLETE_TOPOLOGY_CYCLE_MS as u64),
            false,
        );
        let preview_geometry: Geometry = build_geometry(PREVIEW_TIME.as_secs_f64() * 1_000.0);

        assert_eq!(first, repeated);
        assert!(first.iter().any(|line| line.contains('⣿')));
        assert!(
            preview_geometry
                .nodes
                .iter()
                .any(|node| node.layer == Layer::Synapse)
        );
    }
}

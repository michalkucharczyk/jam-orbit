// Validators Ring Shader
// Renders directed events as bezier trail lines between validators on a circle.
// Each particle is a growing/shrinking bezier curve segment tessellated into quads.
//
// We draw 96 vertices per instance (16 segments × 6 verts) for ALL particles — even
// radial ones — because wgpu's draw(0..N, ...) applies the same vertex count to every
// instance in a single draw call. Splitting radial vs directed into separate draw calls
// would require separate buffers and complicate the incremental upload system.
//
// Radial events (source == target): Only the first 6 vertices form a circle quad.
//   Vertices 6–95 are moved off-screen (early discard in VS, cheap).
//   quad_uv carries [-1,1] coords so the FS can discard outside the unit circle.
//
// Directed events (source != target): All 96 vertices used. Each group of 6 forms a
//   thin quad along the bezier curve; 16 quads stitched together make the trail line.
//   quad_uv is set to (0,0) so the FS skips the circle check and outputs solid color.

struct Uniforms {
    current_time: f32,
    num_validators: f32,
    aspect_ratio: f32,
    point_size: f32,       // line half-width in NDC
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Event type color lookup table (16 categories)
@group(0) @binding(1)
var<uniform> color_lut: array<vec4<f32>, 16>;

// Event type filter bitfield (256 bits = 8 x u32)
@group(0) @binding(2)
var<uniform> event_filter: array<vec4<u32>, 2>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) quad_uv: vec2<f32>,    // unused for trails, kept for compatibility
}

// Number of line segments per trail instance (directed events only)
const NUM_SEGMENTS: u32 = 16u;
const VERTS_PER_INSTANCE: u32 = NUM_SEGMENTS * 6u;  // 96 vertices

// Directed events travel 4x faster than their travel_duration
const DIRECTED_SPEED: f32 = 8.0;

// Line half-width in NDC for directed trail lines
const LINE_HALF_WIDTH: f32 = 0.0015;

// Quad vertex offsets for 2 triangles (6 vertices per quad segment)
// Triangle 1: 0,1,2  Triangle 2: 2,1,3
const QUAD_POS = array<vec2<f32>, 6>(
    vec2(-1.0, -1.0), vec2( 1.0, -1.0), vec2(-1.0,  1.0),
    vec2(-1.0,  1.0), vec2( 1.0, -1.0), vec2( 1.0,  1.0),
);

const PI: f32 = 3.14159265359;
// IMPORTANT: CPU overlay (validator dots, ring outline) in app.rs must match this value.
// NDC-to-pixel conversion: pixel_radius = RING_RADIUS * rect.height() * 0.5
// Angle formula: (index / num_validators) * 2π - π/2  (top = index 0)
const RING_RADIUS: f32 = 0.75;

// Get position on validator ring (circle)
fn validator_position(index: f32) -> vec2<f32> {
    let angle = (index / uniforms.num_validators) * 2.0 * PI - PI * 0.5;
    // Negate Y: wgpu NDC is Y-up, but egui screen coords (CPU overlay) are Y-down.
    return vec2(cos(angle), -sin(angle)) * RING_RADIUS;
}

// Quadratic bezier interpolation
fn bezier_quadratic(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, t: f32) -> vec2<f32> {
    let one_minus_t = 1.0 - t;
    return one_minus_t * one_minus_t * p0 + 2.0 * one_minus_t * t * p1 + t * t * p2;
}

// Quadratic bezier tangent (derivative)
fn bezier_tangent(p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, t: f32) -> vec2<f32> {
    return 2.0 * (1.0 - t) * (p1 - p0) + 2.0 * t * (p2 - p1);
}

// Get color for event type category
fn get_event_color(event_type: f32) -> vec4<f32> {
    // Map event type ranges to color indices
    var color_idx: u32 = 0u;
    let et = u32(event_type);

    if et == 0u {
        color_idx = 0u;  // Meta - gray
    } else if et >= 10u && et <= 13u {
        color_idx = 1u;  // Status - green
    } else if et >= 20u && et <= 28u {
        color_idx = 2u;  // Connection - blue
    } else if et >= 40u && et <= 47u {
        color_idx = 3u;  // Block auth - orange
    } else if et >= 60u && et <= 68u {
        color_idx = 4u;  // Block dist - purple
    } else if et >= 80u && et <= 84u {
        color_idx = 5u;  // Tickets - red
    } else if et >= 90u && et <= 104u {
        color_idx = 6u;  // Work Package - cyan
    } else if et >= 105u && et <= 113u {
        color_idx = 7u;  // Guaranteeing - teal
    } else if et >= 120u && et <= 131u {
        color_idx = 8u;  // Availability - yellow
    } else if et >= 140u && et <= 153u {
        color_idx = 9u;  // Bundle - pink
    } else if et >= 160u && et <= 178u {
        color_idx = 10u; // Segment - light blue
    } else if et >= 190u && et <= 199u {
        color_idx = 11u; // Preimage - light gray
    } else {
        color_idx = 15u; // Unknown - white
    }

    return color_lut[color_idx];
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) source_index: f32,
    @location(1) target_index: f32,
    @location(2) birth_time: f32,
    @location(3) travel_duration: f32,
    @location(4) event_type: f32,
    @location(5) curve_seed: f32,
) -> VertexOutput {
    var out: VertexOutput;
    out.quad_uv = vec2(0.0);

    let age = uniforms.current_time - birth_time;
    let is_directed = source_index != target_index;

    // Check event type filter bitfield (early out)
    let et = u32(event_type);
    let vec_idx = et / 128u;
    let comp_idx = (et % 128u) / 32u;
    let bit_idx = et % 32u;
    if (event_filter[vec_idx][comp_idx] & (1u << bit_idx)) == 0u {
        out.clip_position = vec4(2.0, 2.0, 0.0, 1.0);
        out.color = vec4(0.0);
        return out;
    }

    // Decode segment and corner from vertex_index
    let segment_idx = vertex_index / 6u;
    let corner_idx = vertex_index % 6u;
    let quad_offset = QUAD_POS[corner_idx];

    // ── Radial events: rendered as circles (only first segment = 6 verts) ──
    if !is_directed {
        // Discard extra segments — radial only uses segment 0
        if segment_idx > 0u {
            out.clip_position = vec4(2.0, 2.0, 0.0, 1.0);
            out.color = vec4(0.0);
            return out;
        }

        let t = clamp(age / travel_duration, 0.0, 1.0);
        if age > travel_duration * 1.5 || age < 0.0 {
            out.clip_position = vec4(2.0, 2.0, 0.0, 1.0);
            out.color = vec4(0.0);
            return out;
        }

        let angle = (source_index / uniforms.num_validators) * 2.0 * PI - PI * 0.5;
        let dir = vec2(cos(angle), -sin(angle));
        let r = mix(RING_RADIUS, RING_RADIUS * 1.2, t);
        let pos = dir * r;

        let corrected_pos = vec2(
            (pos.x + quad_offset.x * uniforms.point_size) / uniforms.aspect_ratio,
            pos.y + quad_offset.y * uniforms.point_size
        );
        out.clip_position = vec4(corrected_pos, 0.0, 1.0);
        out.quad_uv = quad_offset;

        var color = get_event_color(event_type);
        let fade_in = smoothstep(0.0, 0.1, t);
        let fade_out = 1.0 - smoothstep(0.9, 1.0, t);
        color.a *= fade_in * fade_out;
        out.color = color;
        return out;
    }

    // ── Directed events: rendered as bezier trail lines (all 16 segments) ──

    // Effective duration with 4x speed
    let eff_dur = travel_duration / DIRECTED_SPEED;

    // Animation: head 0→1 in eff_dur, then tail 0→1 in eff_dur
    let t_head = clamp(age / eff_dur, 0.0, 1.0);
    let t_tail = clamp((age - eff_dur) / eff_dur, 0.0, 1.0);

    if age > eff_dur * 2.5 || age < 0.0 || t_head <= t_tail {
        out.clip_position = vec4(2.0, 2.0, 0.0, 1.0);
        out.color = vec4(0.0);
        return out;
    }

    // along: 0=segment start, 1=segment end; side: ±1 perpendicular
    let along = (quad_offset.x + 1.0) * 0.5;
    let side = quad_offset.y;

    // Parametric t within visible [t_tail, t_head] range
    let seg_frac = (f32(segment_idx) + along) / f32(NUM_SEGMENTS);
    let curve_t = mix(t_tail, t_head, seg_frac);

    // Bezier curve between source and target validators
    let source_pos = validator_position(source_index);
    let target_pos = validator_position(target_index);
    let mid = (source_pos + target_pos) * 0.5;
    let diff = target_pos - source_pos;
    let perp = normalize(vec2(-diff.y, diff.x));
    let dist = length(diff);
    let curve_amount = curve_seed * dist * 0.3;
    let control = mid + perp * curve_amount;
    let pos = bezier_quadratic(source_pos, control, target_pos, curve_t);
    let tangent = bezier_tangent(source_pos, control, target_pos, curve_t);

    // Normal perpendicular to tangent for line width offset
    let tang_len = length(tangent);
    var normal: vec2<f32>;
    if tang_len > 0.001 {
        normal = normalize(vec2(-tangent.y, tangent.x));
    } else {
        normal = vec2(0.0, 1.0);
    }

    let offset_pos = pos + normal * side * LINE_HALF_WIDTH;

    let corrected_pos = vec2(
        offset_pos.x / uniforms.aspect_ratio,
        offset_pos.y
    );
    out.clip_position = vec4(corrected_pos, 0.0, 1.0);

    // Color with alpha gradient along trail (head bright, tail dim)
    var color = get_event_color(event_type);
    let trail_alpha = mix(0.3, 1.0, seg_frac);
    let overall_progress = age / (eff_dur * 2.0);
    let fade_in = smoothstep(0.0, 0.05, overall_progress);
    let fade_out = 1.0 - smoothstep(0.95, 1.0, overall_progress);
    color.a *= trail_alpha * fade_in * fade_out;

    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Radial particles use quad_uv for circle shape; directed trails have quad_uv = (0,0)
    let is_circle = (in.quad_uv.x != 0.0 || in.quad_uv.y != 0.0);
    if is_circle {
        let dist_sq = dot(in.quad_uv, in.quad_uv);
        if dist_sq > 1.0 {
            discard;
        }
    }
    if in.color.a <= 0.01 {
        discard;
    }
    return in.color;
}

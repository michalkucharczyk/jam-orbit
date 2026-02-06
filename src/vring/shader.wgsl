// Validators Ring Shader
// Renders particles traveling between validators on a circle using quadratic bezier curves

struct Uniforms {
    current_time: f32,
    num_validators: f32,
    aspect_ratio: f32,
    point_size: f32,
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
    @location(1) quad_uv: vec2<f32>,
}

// Quad vertex offsets for 2 triangles (6 vertices per instance)
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

    let quad_offset = QUAD_POS[vertex_index % 6u];
    out.quad_uv = quad_offset;

    let age = uniforms.current_time - birth_time;

    // Compute progress along path [0, 1]
    let t = clamp(age / travel_duration, 0.0, 1.0);

    // Discard completed particles
    if age > travel_duration * 1.5 || age < 0.0 {
        out.clip_position = vec4(2.0, 2.0, 0.0, 1.0);  // Off-screen
        out.color = vec4(0.0);
        return out;
    }

    // Check event type filter bitfield
    let et = u32(event_type);
    let vec_idx = et / 128u;
    let comp_idx = (et % 128u) / 32u;
    let bit_idx = et % 32u;
    if (event_filter[vec_idx][comp_idx] & (1u << bit_idx)) == 0u {
        out.clip_position = vec4(2.0, 2.0, 0.0, 1.0);
        out.color = vec4(0.0);
        return out;
    }

    // Get source and target positions on the ring
    let source_pos = validator_position(source_index);
    let target_pos = validator_position(target_index);

    // Compute control point for bezier curve
    let mid = (source_pos + target_pos) * 0.5;
    let diff = target_pos - source_pos;
    let perp = normalize(vec2(-diff.y, diff.x));

    // Curve deviation based on seed and distance
    // Push outward more for longer paths (crossing center)
    let dist = length(diff);
    let curve_amount = curve_seed * dist * 0.3;
    let control = mid + perp * curve_amount;

    // Get position along bezier curve
    let pos = bezier_quadratic(source_pos, control, target_pos, t);

    // Apply aspect ratio correction + quad offset scaled by point_size
    // point_size is in NDC units (e.g. 0.005 = ~3px on a 600px viewport)
    // Quad X offset also divided by aspect_ratio so particles stay circular
    let corrected_pos = vec2(
        (pos.x + quad_offset.x * uniforms.point_size) / uniforms.aspect_ratio,
        pos.y + quad_offset.y * uniforms.point_size
    );

    out.clip_position = vec4(corrected_pos, 0.0, 1.0);

    // Get color based on event type with alpha fade
    var color = get_event_color(event_type);

    // Fade in at start, fade out at end
    let fade_in = smoothstep(0.0, 0.1, t);
    let fade_out = 1.0 - smoothstep(0.9, 1.0, t);
    color.a *= fade_in * fade_out;

    out.color = color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Discard outside unit circle for round particles
    let dist_sq = dot(in.quad_uv, in.quad_uv);
    if dist_sq > 1.0 || in.color.a <= 0.01 {
        discard;
    }
    return in.color;
}

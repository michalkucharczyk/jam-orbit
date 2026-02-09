// Scatter Plot Shader for Event Particles
// Renders particles as colored round dots: X = node index, Y = age
// Uses quad-based rendering (6 vertices per instance) for reliable sizing
// across GPU drivers — PointList point_size is capped at 1px on many GPUs.
// Color from event type via color LUT, filtered by event_filter bitfield.

struct Uniforms {
    x_range: vec2<f32>,
    y_range: vec2<f32>,
    point_size: f32,
    current_time: f32,
    max_age: f32,
    aspect_ratio: f32,
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
const QUAD_POS = array<vec2<f32>, 6>(
    vec2(-1.0, -1.0), vec2( 1.0, -1.0), vec2(-1.0,  1.0),
    vec2(-1.0,  1.0), vec2( 1.0, -1.0), vec2( 1.0,  1.0),
);

// Get color for event type category (same mapping as ring shader)
fn get_event_color(event_type: f32) -> vec4<f32> {
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
    @location(0) node_index: f32,
    @location(1) birth_time: f32,
    @location(2) event_type: f32,
) -> VertexOutput {
    var out: VertexOutput;

    let quad_offset = QUAD_POS[vertex_index % 6u];
    out.quad_uv = quad_offset;

    let age = uniforms.current_time - birth_time;

    // Discard old or future particles
    if age > uniforms.max_age || age < 0.0 {
        out.clip_position = vec4(2.0, 2.0, 0.0, 1.0);
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

    // Transform to normalized device coordinates [-1, 1]
    let x = mix(-1.0, 1.0, (node_index - uniforms.x_range[0]) / (uniforms.x_range[1] - uniforms.x_range[0]));
    let y = mix(-1.0, 1.0, (age - uniforms.y_range[0]) / (uniforms.y_range[1] - uniforms.y_range[0]));

    // Apply quad offset scaled by point_size (in NDC)
    // Divide X offset by aspect_ratio so particles stay circular on non-square textures
    out.clip_position = vec4(x + quad_offset.x * uniforms.point_size / uniforms.aspect_ratio, y + quad_offset.y * uniforms.point_size, 0.0, 1.0);

    // Color from event type with age-based alpha fade
    var color = get_event_color(event_type);
    let fade_start = 0.5;
    let normalized_age = age / uniforms.max_age;
    let alpha = 1.0 - smoothstep(fade_start, 1.0, normalized_age);
    color.a *= alpha;

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

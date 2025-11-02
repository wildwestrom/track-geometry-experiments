#import bevy_pbr::forward_io::VertexOutput

struct ContourSettings {
    metallic: f32,
    perceptual_roughness: f32,
    interval: f32,
    line_color: vec3<f32>,
    line_thickness: f32,
    enabled: u32,
}

// Material bindings - AsBindGroup with Material trait uses bind group 3
// Matching shadplay pattern: material bindings go in group 3
@group(3) @binding(100)
var placeholder_texture: texture_2d<f32>;
@group(3) @binding(101)
var placeholder_sampler: sampler;

@group(3) @binding(102)
var<uniform> contour_settings: ContourSettings;

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    // Get world position Y (height) for contour calculation
    let world_height = in.world_position.y;
    
    // Compute contour using modulo/fract pattern
    // Map height to [0, 1] range based on interval
    let normalized_height = fract(world_height / contour_settings.interval);
    
    // Create smooth transition around the contour line
    // Contour lines appear when normalized_height is close to 0.0 or 1.0
    // We use smoothstep to create antialiased edges
    let distance_from_line = min(normalized_height, 1.0 - normalized_height);
    
    // Convert distance to contour factor using smoothstep for antialiasing
    // Lines appear where distance is small
    let contour_factor = 1.0 - smoothstep(
        0.0,
        contour_settings.line_thickness,
        distance_from_line
    );
    
    // Output only the contour lines with alpha channel
    // Alpha is 0 where there are no lines, and contour_factor where there are lines
    var alpha: f32 = 0.0;
    if !(contour_settings.enabled == 0) {
        alpha = contour_factor;
    }
    
    // Return contour line color with alpha for overlay blending
    return vec4<f32>(contour_settings.line_color, alpha);
}


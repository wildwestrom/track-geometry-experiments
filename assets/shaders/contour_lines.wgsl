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
    // Early return if contour lines are disabled
    if contour_settings.enabled == 0u {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Get world position height (Y component)
    let world_height = in.world_position.y;
    
    // Compute normalized position within the contour interval (0-1 range)
    // Lines occur when this value is near 0.0 or 1.0
    let contour_pos = fract(world_height / contour_settings.interval);
    
    // Calculate screen-space derivative of the normalized contour position
    // fwidth computes abs(dFdx(x)) + abs(dFdy(x)), giving us how much the value
    // changes across a 2x2 pixel quad in screen space
    let contour_fwidth = fwidth(world_height / contour_settings.interval);
    
    // Compute distance from the nearest contour line boundary (at 0.0 or 1.0)
    // This gives us the distance in the normalized [0,1] interval
    let dist_from_boundary = min(contour_pos, 1.0 - contour_pos);
    
    // Normalize to screen space by dividing by the screen-space derivative
    // This converts world-space distance to screen-space (pixel) distance
    let screen_space_dist = dist_from_boundary / contour_fwidth;
    
    // Use smoothstep to create an anti-aliased line with constant screen-space thickness
    // line_thickness controls how many pixels wide the line appears
    // Smaller values = thinner lines, larger values = thicker lines
    let line_factor = 1.0 - smoothstep(0.0, contour_settings.line_thickness, screen_space_dist);
    
    // Blend between transparent background and line color
    let color = mix(
        vec3<f32>(0.0, 0.0, 0.0),  // Background (transparent when alpha is 0)
        contour_settings.line_color,
        line_factor
    );
    
    // Return color with alpha based on line factor for blending
    return vec4<f32>(color, line_factor);
}


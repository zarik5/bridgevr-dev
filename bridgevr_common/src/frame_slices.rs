use crate::rendering::TextureBounds;

#[derive(Clone, Copy)]
pub struct SlicesDesc {
    pub single_resolution: (u32, u32),
    pub horizontal_count: usize,
    pub vertical_count: usize,
}

// Find the best arrangement of horizontal and vertical cuts so that the slices are as close as
// possible to squares. Maximizing the area/perimeter ratio, I minimize the probability that objects
// in the scene enter or exit the slice, so it uses less bandwidth.
pub fn slices_desc_from_count(count: usize, (frame_width, frame_height): (u32, u32)) -> SlicesDesc {
    let mut min_ratio_score = std::f32::MAX; // distance from 1
    let mut best_slices_desc = SlicesDesc {
        single_resolution: (frame_width, frame_height),
        horizontal_count: 1,
        vertical_count: 1,
    };
    for i in 1..=count {
        if count % i == 0 {
            let width = frame_width as f32 / (count / i) as f32;
            let height = frame_height as f32 / i as f32;
            let ratio = width / height;
            let score = (1f32 - ratio).abs();
            if score < min_ratio_score {
                min_ratio_score = score;
                best_slices_desc = SlicesDesc {
                    // 1 pixel of padding for overlap
                    single_resolution: (width.ceil() as u32 + 1, height.ceil() as u32 + 1),
                    horizontal_count: count / i,
                    vertical_count: i,
                }
            } else {
                break;
            }
        }
    }

    best_slices_desc
}

// slices are indexed left to right first, then top to bottom
pub fn get_slice_start(slice_index: usize, slice_desc: &SlicesDesc) -> (u32, u32) {
    let (single_width, single_height) = slice_desc.single_resolution;
    let horizontal_count = slice_index % slice_desc.horizontal_count;
    let vertical_count = slice_index / slice_desc.horizontal_count;

    (
        horizontal_count as u32 * (single_width - 1), // 1 pixel padding
        vertical_count as u32 * (single_height - 1),
    )
}

pub fn slice_bounds_to_texture_bounds(
    (frame_width, frame_height): (u32, u32),
    (slice_start_x, slice_start_y): (u32, u32),
    (aligned_slice_width, aligned_slice_height): (u32, u32),
) -> TextureBounds {
    let u_min = slice_start_x as f32 / frame_width as f32;
    let v_min = slice_start_y as f32 / frame_height as f32;
    let u_max = (slice_start_x + aligned_slice_width) as f32 / frame_width as f32;
    let v_max = (slice_start_y + aligned_slice_height) as f32 / frame_height as f32;

    TextureBounds {
        u_min,
        v_min,
        u_max,
        v_max,
    }
}

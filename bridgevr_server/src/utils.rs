struct SlicesDesc {
    width: u32,
    height: u32,
    horizontal_count: usize,
    vertical_count: usize,
}

// Find the best arrangement of horizontal and vertical cuts so that the slices are as close as
// possible to squares. Maximizing the area/perimeter ratio, I minimize the probability that objects
// in the scene enter or exit the slice, so it uses less bandwidth.
pub fn compute_slices_with_count(count: usize, frame_width: u32, frame_height: u32) -> SlicesDesc {
    let mut min_ratio_score = std::f32::MAX; // closeness to 1
    let mut best_slice_desc = SlicesDesc {
        width: frame_width,
        height: frame_height,
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
                best_slice_desc = SlicesDesc {
                    width: width.ceil() as u32,
                    height: height.ceil() as u32,
                    horizontal_count: count / i,
                    vertical_count: i,
                }
            } else {
                break;
            }
        }
    }

    best_slice_desc
}

// Actually max_pixels is only a hint. The encoder can deliberately add vertical and horizontal
// padding to optimize encoding.
pub fn compute_slices_with_max_pixels(
    max_pixels: u32,
    frame_width: u32,
    frame_height: u32,
) -> SlicesDesc {
    let count = ((frame_width * frame_height) as f32 / max_pixels as f32).ceil() as usize;
    compute_slices_with_count(count, frame_width, frame_height)
}

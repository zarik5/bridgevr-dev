
pub struct SlicesDesc {
    pub single_width: u32,
    pub single_height: u32,
    pub horizontal_count: usize,
    pub vertical_count: usize,
}

// Find the best arrangement of horizontal and vertical cuts so that the slices are as close as
// possible to squares. Maximizing the area/perimeter ratio, I minimize the probability that objects
// in the scene enter or exit the slice, so it uses less bandwidth.
pub fn slices_desc_from_count(count: usize, frame_width: u32, frame_height: u32) -> SlicesDesc {
    let mut min_ratio_score = std::f32::MAX; // distance from 1
    let mut best_slices_desc = SlicesDesc {
        single_width: frame_width,
        single_height: frame_height,
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
                    single_width: width.ceil() as u32,
                    single_height: height.ceil() as u32,
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
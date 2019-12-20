use crate::rendering::TextureBounds;

#[derive(Clone, Copy)]
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
                    single_width: width.ceil() as u32 + 1, // 1 pixel of padding for overlap
                    single_height: height.ceil() as u32 + 1,
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
    let horizontal_count = slice_index % slice_desc.horizontal_count;
    let vertical_count = slice_index / slice_desc.horizontal_count;

    (
        horizontal_count as u32 * (slice_desc.single_width - 1), // 1 pixel padding
        vertical_count as u32 * (slice_desc.single_height - 1),
    )
}

pub fn slice_bounds_to_texture_bounds(
    frame_width: u32,
    frame_height: u32,
    slice_start_x: u32,
    slice_start_y: u32,
    aligned_slice_width: u32,
    aligned_slice_height: u32,
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

// pub struct SliceStartIterator {
//     slices_desc: SlicesDesc,
//     next_slice_start: (u32, u32),
// }

// impl SliceStartIterator {
//     pub fn new(slices_desc: SlicesDesc) -> Self {
//         Self {
//             slices_desc,
//             next_slice_start: (0, 0),
//         }
//     }
// }

// impl Iterator for SliceStartIterator {
//     type Item = (u32, u32);

//     fn next(&mut self) -> Option<(u32, u32)> {
//         let (next_start_x, next_start_y) = &mut self.next_slice_start;
//         if *next_start_y < self.slices_desc.vertical_count as u32 {
//             let res = (
//                 *next_start_x * (self.slices_desc.single_width - 1), // 1 pixel padding
//                 *next_start_y * (self.slices_desc.single_height - 1),
//             );
//             *next_start_x += 1;
//             if *next_start_x == self.slices_desc.horizontal_count as u32 {
//                 *next_start_x = 0;
//                 *next_start_y += 1;
//             }

//             Some(res)
//         } else {
//             None
//         }
//     }
// }

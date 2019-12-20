use gst::prelude::*;
use gstreamer as gst;
use log::warn;

pub struct Gstreamer {
    pipeline: gst::Element,
}

// impl Gstreamer {
//     pub fn new<O>(pipeline_description: &str, data_callback: impl FnMut(&O) + 'static) -> Self {
//         let pipeline = ok_or_panic!(
//             gst::parse_launch(pipeline_description),
//             "GStreamer pipeline"
//         );

//         pipeline
//         Self { pipeline }
//     }

//     pub fn submit_data<I>(data: &I) {}
// }

impl Drop for Gstreamer {
    fn drop(&mut self) {
        if let Err(err) = self.pipeline.set_state(gst::State::Null) {
            debug!("GStreamer shutdown: {}", err);
        }
    }
}

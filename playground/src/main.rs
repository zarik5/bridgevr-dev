mod logging_backend;

// use gst::prelude::*;
// use gstreamer as gst;
use serde::*;
use std::marker::PhantomData;
use std::sync::*;

// #[derive(Serialize, Deserialize, Debug, Default)]
#[repr(C)]
#[derive(zerocopy::FromBytes)]
struct MyData<'a> {
    b: &'a [u8],
    a: u32,
}

fn main() {

    let bt = backtrace::Backtrace::new();


    println!("{:?}", bt);


    // logging_backend::init_logging();
    // pkg_config::probe_library("gstreamer-1.0").unwrap();

    // gst::init().unwrap();

    // let pipeline_desc = "playbin uri=https://www.freedesktop.org/software/gstreamer-sdk/data/media/sintel_trailer-480p.webm";
    // let pipeline_desc = r"videotestsrc pattern=0 ! glupload ! video/x-raw(memory:GLMemory) !  ! autovideosink";
    // // 0 8 10 11 12 13 18 21 24

    // let pipeline = gst::parse_launch(pipeline_desc).unwrap();

    // pipeline
    //     .set_state(gst::State::Playing)
    //     .expect("Unable to set the pipeline to the `Playing` state");

    // // Wait until error or EOS
    // let bus = pipeline.get_bus().unwrap();
    // for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
    //     use gst::MessageView;

    //     match msg.view() {
    //         MessageView::Eos(..) => break,
    //         MessageView::Error(err) => {
    //             println!(
    //                 "Error from {:?}: {} ({:?})",
    //                 err.get_src().map(|s| s.get_path_string()),
    //                 err.get_error(),
    //                 err.get_debug()
    //             );
    //             break;
    //         }
    //         _ => (),
    //     }
    // }

    // // Shutdown pipeline
    // pipeline
    //     .set_state(gst::State::Null)
    //     .expect("Unable to set the pipeline to the `Null` state");
}

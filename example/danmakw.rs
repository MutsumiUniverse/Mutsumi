use adw::prelude::*;
use mutsumi::Danmakw;

mod parse;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = adw::Application::builder()
        .application_id("io.github.mutsumi.example.player")
        .build();

    app.connect_activate(move |app| {
        mutsumi::init();

        let danmakw = Danmakw::new();

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Mutsumi Player")
            .default_width(1280)
            .default_height(720)
            .content(&danmakw)
            .build();

        window.present();

        let xml = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example/test.xml"),
        )
        .expect("failed to read test.xml");

        match parse::parse_bilibili_xml(&xml) {
            Ok(danmaku) => {
                danmakw.load_danmaku(danmaku);
                danmakw.start_rendering();
            }
            Err(e) => eprintln!("parse error: {e}"),
        }
    });

    app.run_with_args::<&str>(&[]);
}

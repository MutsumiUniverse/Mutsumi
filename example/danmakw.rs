use adw::{glib, prelude::*};
use mutsumi::Danmakw;

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

        danmakw.add_danmaku("abc");
        danmakw.add_danmaku("abcsdasdsadasdasdsafsadfasfasfasfasfasf");
    });

    app.run_with_args::<&str>(&[]);
}

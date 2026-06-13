use adw::{glib, prelude::*};
use mutsumi::{PlayParams, PlaySource, PlayerPage};

const DEFAULT_URL: &str = "https://www.youtube.com/watch?v=agmPj7XV-Xw";

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let url = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_URL.to_string());

    let app = adw::Application::builder()
        .application_id("io.github.mutsumi.example.player")
        .build();

    app.connect_activate(move |app| {
        mutsumi::control_init();

        let player = PlayerPage::new();

        player.mpv().set_property(
            "ytdl-raw-options",
            "cookies-from-browser=firefox".to_string(),
        );
        player
            .mpv()
            .command("script-binding", &["stats/display-stats-toggle"]);

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Mutsumi Player")
            .default_width(1280)
            .default_height(720)
            .content(&player)
            .build();

        window.present();

        let param = PlayParams::builder(PlaySource::Url(url.to_owned()))
            .title("wl-proxy embed mpv demo")
            .subtitle(url.to_owned())
            .build();

        player.play(&param);
    });

    app.run_with_args::<&str>(&[]);
}

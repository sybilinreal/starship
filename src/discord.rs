pub use discord_sdk as ds;
use tracing;

pub const APP_ID: ds::AppId = 1098754579383976047;

pub struct Client {
    pub discord: ds::Discord,
    pub user: ds::user::User,
    pub wheel: ds::wheel::Wheel
}

pub async fn make_client(subs: ds::Subscriptions) -> Client {
    let (wheel, handler) = ds::wheel::Wheel::new(Box::new(|err| {
        tracing::error!(error = ?err, "encountered an error");
    }));

    let mut user = wheel.user();

    let discord = ds::Discord::new(ds::DiscordApp::PlainId(APP_ID), subs, Box::new(handler))
        .expect("unable to create discord client");

    println!("connecting to discord...");
    // tracing::info!("waiting for handshake");
    user.0.changed().await.unwrap();

    let user = match &*user.0.borrow() {
        ds::wheel::UserState::Connected(user) => user.clone(),
        ds::wheel::UserState::Disconnected(err) => panic!("failed to connect to Discord: {}", err)
    };

    println!("done"); // connected
    tracing::info!("discord user is {}#{:0>4}", user.username, user.discriminator.unwrap_or(0));

    Client {
        discord,
        user,
        wheel
    }
}
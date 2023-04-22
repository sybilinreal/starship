use memory_rs::external::process::*;
use std::{
	thread::sleep,
	time::{Duration, Instant, SystemTime},
	//string
};
mod discord;
use discord::ds;

const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");

static CHARS: [&str; 24] = [
	"unknown",
	"Sol", // "Sol Badguy",
	"Ky", // "Ky Kiske",
	"May",
	"Axl", // "Axl Low",
	"Chipp", // "Chipp Zanuff",
	"Potemkin",
	"Faust",
	"Millia", // "Millia Rage",
	"Zato-ONE",
	"Ramlethal", // "Ramlethal Valentine",
	"Leo", // "Leo Whitefang",
	"Nagoriyuki",
	"Giovanna",
	"Anji", // "Anji Mito",
	"I-No",
	"Goldlewis", // "Goldlewis Dickison",
	"Jack-O", // "Jack-O' Valentine",
	"Happy Chaos",
	"Baiken",
	"Testament",
	"Bridget",
	"Sin", // "Sin Kiske",
	"Bedman?"
];
fn char_from_u8(char: u8) -> &'static str {
	if char == 33 { CHARS[0] }
	else { CHARS[(char+1) as usize] }
}

static CHARS_SHORT: [&str; 24] = [
	"??",
	"SO",
	"KY",
	"MA",
	"AX",
	"CH",
	"PO",
	"FA",
	"MI",
	"ZA",
	"RA",
	"LE",
	"NA",
	"GI",
	"AN",
	"IN",
	"GO",
	"JC",
	"HA",
	"BA",
	"TE",
	"BR",
	"SI",
	"BE",
];
fn char_short_u8(char: u8) -> &'static str {
	if char == 33 { CHARS_SHORT[0] }
	else { CHARS_SHORT[(char+1) as usize] }
}

fn vs_string(p1: u8, p2: u8) -> String {
	format!("{} vs. {}", char_from_u8(p1), char_from_u8(p2))
}
fn vs_string_long(p1: u8, p1_name: String, p2: u8, p2_name: String) -> String{
	// includes username, for online state
	format!(
		"{} ({}) vs. {} ({})",
		p1_name, char_short_u8(p1),
		p2_name, char_short_u8(p2)
	)
}

fn is_running() -> bool {
	let init: bool = match Process::new("GGST-Win64-Shipping.exe") {
		Ok(_) => true,
		Err(e) => {
			if e.to_string().contains("no more files") {
				// probably not running. this is the only error i see for that
				tracing::debug!("NMF");
				return false;
			}
			tracing::error!("{}", e);
			return true;
		}
	};

	return init;
}

fn wait_for_launch() {
	let interval = Duration::from_secs(15);
	let mut next_time = Instant::now() + interval;

	let mut already_complained = false;

	while !is_running() {
		if !already_complained {
			println!("unable to find strive; is it running?");
			already_complained = true;
		}

		// wait around so it doesn't poll really fast. it doesn't need to
		sleep(next_time - Instant::now());
		next_time += interval;
	}

	println!("found strive");
}

fn read_value(ggst: &Process, addr: usize) -> u8 {
	ggst.read_value::<u8>(addr, false)
}

fn read_value_str(ggst: &Process, addr: usize) -> String {
	let mut bytes:Vec<u16> = Vec::new();

	let mut a: u16;

	let mut offset: usize = 0;
	loop {
		a = ggst.read_value::<u16>(addr+offset, false);
		
		if a == 0 { break };

		bytes.push(a);
		offset += 2;
	}

	return String::from_utf16_lossy(&bytes);
}

fn gen_presence_from_memory(ggst: &Process, prev_gamemode: u8) -> Option<(ds::activity::ActivityBuilder, u8)> {
	let mut gamemode = read_value(&ggst, 0x45427f0);
	let      p1_char = read_value(&ggst, 0x48ab7f0);
	let      p2_char = read_value(&ggst, 0x48ab898);
	let  is_training = read_value(&ggst, 0x48ac024);

	let       p_side = read_value(&ggst, 0x48ced90); // player side when playing online

	// experimental values
	let is_replay = read_value(&ggst, 0x44d1f20); // or 0x44d1ef8; 1 in (offline) matches and 2 in replays
	let is_online = read_value(&ggst, 0x45d10bd) == 1; // or 0x45d10b9
	
	// let alt_p1 = read_value(&ggst, 0x48adea4); // p1 selected char; might be online?
	// let alt_p2 = read_value(&ggst, 0x43f7b48); // p2 selected char; might be for non-players, like arcade
	// let offline = read_value(&ggst, 0x48cedd0); // maybe online match flag? inverted. 0 is online
	// let  replay = read_value(&ggst, 0x4fc41d0); //  some sort of online match flag? also hits replays but not offline matches

	let     name_self = read_value_str(&ggst, 0x4Be1dc6);
	let name_opponent = read_value_str(&ggst, 0x48cb226); // prev 0x48d0be2
	let name_other = read_value_str(&ggst, 0x48cb710); // maybe parks and maybe spectating?

	tracing::debug!("{} {} {}({})", p1_char, p2_char, gamemode, is_training);
	tracing::debug!("\"{}\"({}) \"{}\"({}) {} {}", name_self, name_self.len(), name_opponent, name_opponent.len(), p_side, is_online);
	tracing::debug!("\"{}\"({})", name_other, name_other.len());

	// 6 is paused; the difference messes with the Elapsed timer
	if gamemode == 6 { gamemode = 5 };

	// if the gamemode hasn't changed then the presence shouldn't be updated
	if gamemode == prev_gamemode { return None };
	
	let desired_details: &str;
	let   desired_state: String;
	let    set_start_ts: bool;

	tracing::info!(gamemode);
	(desired_details, desired_state, set_start_ts) = match gamemode {
		// loading, title screen
		3 => { ("Loading...", String::from(""), false) },
		
		// match, replays, training mode
		5 => {
			if is_training == 1 { ("In training mode", String::from(""), true) }
			else if is_replay == 2 { ("Watching a replay", vs_string(p1_char, p2_char), true) }

			// normal match - check for online/offline here
			else {
				// spectator
				if p_side == 2 { ("Watching a match", vs_string(p1_char, p2_char), true) }
				// actually playing
				else if is_online {
					let (p1_name, p2_name) = if p_side == 0 { (name_self, name_opponent) }
					else { (name_opponent, name_self) };
					("In a match", vs_string_long(p1_char, p1_name, p2_char, p2_name), true)
				}
				else { ("In an offline match", vs_string(p1_char, p2_char), true) }
			}
		},

		// fishing, avatar; lobby?
		// 9 => { },

		// lobby
		12 => { ("In a lobby", String::from(""), true) },

		// something about rooms?
		// 18 => {}

		// win screen, main menu
		// 29 => { },

		// rematch prompt
		69 => { ("In a match", String::from("Waiting for rematch..."), true) },

		// unknown - assume some menu because there's a lot
		_ => { ("In the menus...", String::from(""), false) }
	};

	let assets = ds::activity::Assets::default()
	.large("bridget-623p", Some(format!("Starship v{}", VERSION.unwrap_or("unknown"))))
	.small("ggst", Some(String::from("for Guilty Gear Strive v1.26")));

	let presence = ds::activity::ActivityBuilder::new()
		.assets(assets)
		.details(desired_details) 
		.state(desired_state);

	let presence = if set_start_ts {presence.start_timestamp(SystemTime::now())} else {presence};

	return Some((presence, gamemode));
}

async fn polling_loop(ggst: &Process, client: &discord::Client) {
	let interval = Duration::from_secs(5);
	let mut next_time = Instant::now() + interval;

	// init value so it doesn't hit any gamemodes
	let mut prev_gamemode: u8 = 0;

	while is_running() {
		// wait around so it doesn't poll really fast
		// this is up here so debug doesn't scream
		sleep(next_time - Instant::now());
		next_time += interval;

		match gen_presence_from_memory(&ggst, prev_gamemode) {
			Some((presence, gamemode)) => {
				prev_gamemode = gamemode;

				client.discord.update_activity(presence).await.unwrap();
				tracing::debug!("updated activity");
			},
			None => {

			}
		};
	}
	tracing::info!("strive closed?");
	client.discord.clear_activity().await.unwrap();
	tracing::debug!("cleared activity");
}

#[tokio::main]
async fn main() {
	tracing_subscriber::fmt()
        .compact()
        .with_max_level(tracing::Level::TRACE)
        .init();

	loop {
		wait_for_launch();
		
		let ggst = Process::new("GGST-Win64-Shipping.exe").unwrap();

		let mut subs = ds::Subscriptions::empty();
		subs.toggle(ds::Subscriptions::ACTIVITY);
		let client = discord::make_client(subs).await;

		polling_loop(&ggst, &client).await;
	}
}

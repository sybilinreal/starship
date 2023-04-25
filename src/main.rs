use memory_rs::external::process::*;
use std::{
	thread::sleep,
	time::{Duration, Instant, SystemTime},
	//string
};

mod discord;
use discord::ds;

mod config;

const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");

static CHARS: [&str; 24] = [
	"<unknown>",
	"Sol Badguy",
	"Ky Kiske",
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
	"Anji Mito",
	"I-No",
	"Goldlewis", // "Goldlewis Dickison",
	"Jack-O", // "Jack-O' Valentine",
	"Happy Chaos",
	"Baiken",
	"Testament",
	"Bridget",
	"Sin Kiske",
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

struct RefreshState {
	gamemode: u8,
	is_in_match: bool,
	was_online: bool
}

fn gen_presence_from_memory(ggst: &Process, refresh_state: &mut RefreshState) -> Option<ds::activity::ActivityBuilder> {
	let mut gamemode = read_value(&ggst, 0x45427f0);
	let      p1_char = read_value(&ggst, 0x48ab7f0);
	let      p2_char = read_value(&ggst, 0x48ab898);

	let     is_replay = read_value(&ggst, 0x44d1f20) == 2;
	let   is_training = read_value(&ggst, 0x48ac024) == 1;

	let        p_side = read_value(&ggst, 0x48ced90); // player side when playing online (2 is spec)
	let     name_self = read_value_str(&ggst, 0x4be1dc6);
	let name_opponent = read_value_str(&ggst, 0x48cb226);
	let    name_other = read_value_str(&ggst, 0x48cb710); // for spectating

	let p1_name: String;
	let p2_name: String;

	if p_side == 0 {
		p1_name = name_self.clone();
		p2_name = name_opponent.clone();
	}
	/* else if p_side == 2 { // spectator
		// currently impossible; need a way to identify player side other than comparing with own name
		// when this is possible it will probably involve name_other, which is the the player other than name_opponent
	} */
	else {
		p1_name = name_self.clone();
		p2_name = name_opponent.clone();
	}

	let is_in_match = read_value(&ggst, 0x45d10b9) == 1; // for detecting rematch

	// cursed experimental online check
	let online_flag = read_value(&ggst, 0x48cedd0);// or 0x45d10bd ?
	
	let is_online: bool;


	// tracing::debug!("{} {} {}({}) {}", p1_char, p2_char, gamemode, is_training, is_in_match);
	// tracing::debug!("\"{}\"({}) \"{}\"({}) {} {}", name_self, name_self.len(), name_opponent, name_opponent.len(), p_side, is_online);
	// tracing::debug!("\"{}\"({})", name_other, name_other.len());

	refresh_state.gamemode = gamemode;
	refresh_state.is_in_match = is_in_match;

	#[derive(Debug, PartialEq, Eq)]
	enum GameState {
		Unknown,
		Menu,
		Loading, // and title screen
		Lobby, // room/park/tower distinction possible
		TrainingMode,
		Replay,
		Match, // no online flag
		// the following never occur for now
		OfflineMatch,
		OnlineMatch,
		Paused,
		Rematch,
	}

	let mut gamestate = match gamemode {
		3|45 => GameState::Loading,
		5 => {
			if is_training { GameState::TrainingMode }
			else if is_replay { GameState::Replay }
			else {
				// online flag is invalid - generic for now
				// actually playing
				// if is_online {
				// 	// spectator
				// 	if p_side == 2 { ("Watching a match", vs_string(p1_char, p2_char), true) }
				// add config check for show_names here
				// 	// determine which player is p1 and p2
				// 		else { let (p1_name, p2_name) =
				// 			if p_side == 0 { (name_self, name_opponent) }
				// 			else { (name_opponent, name_self) };

				// 		("In a match", vs_string_long(p1_char, p1_name, p2_char, p2_name), true)
				// 	}
				// }
				// else { ("In an offline match", vs_string(p1_char, p2_char), true) }
				GameState::Match
			}
		},
		6 => GameState::Paused,
		69 => GameState::Rematch,
		// probably also lobby: 9, 18
		10|12 => GameState::Lobby,
		29|32|35|38|40|43 => GameState::Menu,
		_ => GameState::Unknown
	};
	tracing::debug!("{} GameState::{:?}", gamemode, gamestate);

	// pretends paused and rematch states are just more Match so that it does not disrupt the "time elapsed" display in sets
	if gamestate == GameState::Rematch || gamestate == GameState::Paused { gamestate = GameState::Match; };

	// if the gamemode hasn't changed then the presence shouldn't be updated
	if refresh_state.gamemode == gamemode && refresh_state.is_in_match == is_in_match { return None };
	// tracing::debug!("refresh_state: {} {}", refresh_state.gamemode, refresh_state.is_in_match);
	// tracing::debug!("mem: {} {}", gamemode, is_in_match);
	
	// Activity values
	let desired_details: &str;
	let   desired_state: String;
	let    set_start_ts: bool;

	(desired_details, desired_state, set_start_ts) = match gamestate {
		GameState::Unknown      => ("unknown game state", gamemode.to_string(), false),
		// GameState::Unknown => ("In the menus...", String::from(""), false),

		GameState::Menu         => ("In the menus...", String::from(""), false),
		GameState::Loading      => ("Loading", String::from(""), false),
		GameState::Lobby        => ("In a lobby", String::from(""), true), // maybe include lobby info in state here - probably a config setting
		GameState::TrainingMode => ("In training mode", String::from(""), true),
		GameState::OfflineMatch => ("In an offline match", vs_string(p1_char, p2_char), true),
		GameState::OnlineMatch  => ("In a match", vs_string_long(p1_char, p1_name, p2_char, p2_name), true),
		GameState::Match        => ("In a match", vs_string(p1_char, p2_char), true),
		GameState::Replay       => ("Watching a replay", vs_string(p1_char, p2_char), true),
		GameState::Rematch      => ("Waiting to rematch...", String::from(""), true),
		GameState::Paused       => ("Paused", String::from(""), true)
	};

	// (desired_details, desired_state, set_start_ts) = match gamemode {
	// 	// loading, title screen
	// 	3 => ("Loading...", String::from(""), false),
		
	// 	// match, replays, training mode
	// 	5 => {
	// 		if is_training { ("In training mode", String::from(""), true) }
	// 		else if is_replay { ("Watching a replay", vs_string(p1_char, p2_char), true) }

	// 		// normal match - check for online/offline here
	// 		else {

	// 			("In a match", vs_string(p1_char, p2_char), true)
	// 		}
	// 	},

	// 	// fishing, avatar; lobby?
	// 	// 9 => { },

	// 	// lobby
	// 	12 => ("In a lobby", String::from(""), true),

	// 	// something about rooms? saw while spectating; investigate
	// 	// 18 => { },

	// 	// win screen, main menu
	// 	29 => {
	// 		if is_in_match { ("In a match", String::from("Waiting to rematch..."), false) }
	// 		else { ("In the menus...", String::from(""), false) }
	// 	},

	// 	// rematch prompt
	// 	69 => ("In a match", String::from("Waiting to rematch..."), false),

	// 	// unknown - assume some menu because there's a lot
	// 	_ => ("In the menus...", String::from(""), false)
	// };

	let assets = ds::activity::Assets::default()
	.large("bridget-623p", Some(format!("Starship v{}", VERSION.unwrap_or("?.?"))))
	.small("ggst", Some(String::from("for Guilty Gear Strive v1.26")));

	let presence = ds::activity::ActivityBuilder::new()
		.assets(assets)
		.details(desired_details) 
		.state(desired_state);

	let presence = if set_start_ts {presence.start_timestamp(SystemTime::now())} else {presence};

	return Some(presence);
}

async fn polling_loop(ggst: &Process, client: &discord::Client) {
	let interval = Duration::from_secs(5);
	let mut next_time = Instant::now() + interval;

	// init value so it doesn't hit any gamemodes
	let mut refresh_state = RefreshState { gamemode: 0u8, is_in_match: false, was_online: false };

	while is_running() {
		// wait around so it doesn't poll really fast
		// this is up here so debug doesn't scream
		sleep(next_time - Instant::now());
		next_time += interval;

		match gen_presence_from_memory(&ggst, &mut refresh_state) {
			Some(presence) => {
				client.discord.update_activity(presence).await.unwrap();
				tracing::debug!("updated activity");
			},
			None => { }
		};
	}
	tracing::info!("strive closed?");
	client.discord.clear_activity().await.unwrap();
	tracing::debug!("cleared activity");
}

#[tokio::main]
async fn main() {
	println!("Starship v{}", VERSION.unwrap_or("?.?"));

	let config = &config::init()["config"];

	let trace_level = if config["debug"] { tracing::Level::TRACE } else { tracing::Level::ERROR };
	// let args: Vec<String> = env::args().collect();
	// let trace_level = if args.iter().any(|i| i=="debug") {tracing::Level::TRACE} else {tracing::Level::ERROR};
	tracing_subscriber::fmt()
        .compact()
        .with_max_level(trace_level)
        .init();

	loop {
		wait_for_launch();
		
		let ggst = Process::new("GGST-Win64-Shipping.exe").unwrap();

		let mut subs = ds::Subscriptions::empty();
		subs.toggle(ds::Subscriptions::ACTIVITY);
		let client = discord::make_client(subs).await;

		polling_loop(&ggst, &client).await;
	};
}

use ini::ini;

use std::{
    collections::HashMap,
    fs::{File, write}
};

fn parse_ini(ini: HashMap<String, HashMap<String, Option<String>>>) -> HashMap<String, HashMap<String, bool>> {
    let mut out: HashMap<String, HashMap<String, bool>> = HashMap::new();

    for (section, entry) in ini {
        let mut new_entry: HashMap<String, bool> = HashMap::new();
        for (key, value) in entry {
            new_entry.insert(key, value.unwrap() == "true");
        }
        out.insert(section, new_entry);
    }

    return out;
}

pub fn init() -> HashMap<String, HashMap<String, bool>> {
    println!("loading config");
	match File::open("config.ini") {
		Ok(_) => {}
		Err(_) => {
			println!("config not found; creating ./config.ini");

			match write("config.ini", "[Config]\ndebug = false\nshow_names = true") {
                Err(e) => {
                    panic!("could not write config.ini. insufficient permissions?\n{}", e)
                },
                _ => {}
            };
		}
	};

	parse_ini(ini!("config.ini"))
}

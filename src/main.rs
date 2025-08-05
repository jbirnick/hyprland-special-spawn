use std::env;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;

const EXCLUDED_CLASSES: [&str; 1] = ["org.kde.dolphin"];

fn main() {
    // get addresses of hyprland sockets
    let xdg_runtime_dir = env::var("XDG_RUNTIME_DIR").expect("couldn't get $XDG_RUNTIME_DIR");
    let hyprland_instance_signature =
        env::var("HYPRLAND_INSTANCE_SIGNATURE").expect("couldn't get $HYPRLAND_INSTANCE_SIGNATURE");
    let socket_address_control =
        format!("{xdg_runtime_dir}/hypr/{hyprland_instance_signature}/.socket.sock");
    let socket_address_events =
        format!("{xdg_runtime_dir}/hypr/{hyprland_instance_signature}/.socket2.sock");

    // connect to the event socket
    let stream_events = UnixStream::connect(socket_address_events)
        .expect("couldn't connect to second hyprland socket");
    stream_events
        .shutdown(std::net::Shutdown::Write)
        .expect("couldn't shutdown writing for second hyprland socket");

    // store which workspace we were on before the window got opened on the special workspace
    let mut last_workspace: String = "1".into();

    // read the events stream line by line and react to the relevant events
    let bufreader_events = BufReader::new(stream_events);
    for line in bufreader_events.lines() {
        let line = line.expect("couldn't read line from hyprland events socket");
        match parse_event(&line) {
            Event::Irrelevant => {}
            Event::FocusedWorkspace { name } => {
                last_workspace = name.into();
            }
            Event::SpawnedWindowOnSpecial { address } => {
                // connect to control socket and dispatch command
                let mut stream_control = UnixStream::connect(&socket_address_control)
                    .expect("couldn't connect to first hyprland socket");
                write!(
                    &mut stream_control,
                    "/dispatch movetoworkspacesilent name:{last_workspace},address:0x{address}"
                )
                .expect("couldn't write to first hyprland socket");
                stream_control
                    .shutdown(std::net::Shutdown::Both)
                    .expect("couldn't shutdown stream to first hyprland socket")
            }
        }
    }
}

enum Event<'a> {
    Irrelevant,
    // TODO: replace with workspace ID (instead of name)
    // but hyprland doesn't provide the workspace ID with the `focusedmon` event
    FocusedWorkspace { name: &'a str },
    SpawnedWindowOnSpecial { address: &'a str },
}

fn parse_event(line: &str) -> Event {
    let (event, data) = line
        .split_once(">>")
        .expect("line of event didn't contain \">>\"");
    let mut data = data.split(',');

    match event {
        "workspace" => {
            let name = data.next().unwrap();
            Event::FocusedWorkspace { name }
        }
        "focusedmon" => {
            let _monitor_name = data.next().unwrap();
            let name = data.next().unwrap();
            Event::FocusedWorkspace { name }
        }
        "openwindow" => {
            let address = data.next().unwrap();
            let workspace = data.next().unwrap();
            if workspace.starts_with("special:") {
                let class = data.next().unwrap();
                match EXCLUDED_CLASSES.contains(&class) {
                    true => Event::Irrelevant,
                    false => Event::SpawnedWindowOnSpecial { address },
                }
            } else {
                Event::Irrelevant
            }
        }
        _ => Event::Irrelevant,
    }
}

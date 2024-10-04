extern crate ncurses;
extern crate mpvipc;

use ncurses::*;
use mpvipc::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;

#[macro_use]
mod macros;

const OBS_ID_PLAYLIST: usize = 1;
const OBS_ID_PAUSE: usize = 2;
const OBS_ID_TIME_POS: usize = 3;
const OBS_ID_DURATION: usize = 4;
const OBS_ID_METADATA: usize = 5;
const OBS_ID_VOLUME: usize = 6;
const OBS_ID_MUTE: usize = 7;
const OBS_ID_SPEED: usize = 8;

const KEY_ENTER: i32 = 10;
const KEY_ESC: i32 = 27;
const KEY_BACKSPACE: i32 = 127;
const KEY_GT: i32 = '>' as i32;
const KEY_LT: i32 = '<' as i32;
const KEY_PLUS: i32 = '+' as i32;
const KEY_MINUS: i32 = '-' as i32;
const KEY_LSBR: i32 = '[' as i32;
const KEY_RSBR: i32 = ']' as i32;
const KEY_M: i32 = 'm' as i32;
const KEY_N: i32 = 'n' as i32;
const KEY_O: i32 = 'o' as i32;
const KEY_P: i32 = 'p' as i32;
const KEY_Q: i32 = 'q' as i32;
const KEY_R: i32 = 'r' as i32;
const KEY_S: i32 = 's' as i32;
const KEY_U: i32 = 'u' as i32;
const KEY_Z: i32 = 'z' as i32;
const KEY_SLASH: i32 = '/' as i32;

enum Repaint {
    Playlist {
        clear_win: bool,
        scroll_to_beginning: bool,
    },
    StatusBar(UpdateStatusBar),
    TopBar(UpdateTopBar),
}

enum UpdateStatusBar {
    Clear,
    Time,
    Message(String, Formatting),
}

enum UpdateTopBar {
    Clear,
    Metadata,
    Speed,
    Volume,
}

enum Formatting {
    Normal,
    Blinking,
}

struct Player {
    duration: f64,
    is_muted: bool,
    is_paused: bool,
    metadata: Option<HashMap<String, MpvDataType>>,
    playlist: Playlist,
    search_results: (Vec<usize>, usize),
    speed: f64,
    time_pos: f64,
    volume: f64,
}

struct PlaylistCanvas {
    top_line: usize,
    bottom_line: usize,
    selected_line: usize,
}

trait Error {
    fn error(&self, &str);
}

impl Error for std::sync::mpsc::Sender<Repaint> {
    fn error(&self, msg: &str) {
        self.send(Repaint::StatusBar(UpdateStatusBar::Message(
            String::from(msg),
            Formatting::Normal,
        ))).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(2500));

        self.send(Repaint::StatusBar(UpdateStatusBar::Clear))
            .unwrap();
    }
}

fn main() {
    setlocale(LcCategory::all, "");
    initscr(); /* Start curses mode 		  */
    noecho();
    keypad(stdscr(), true);
    curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);

    let mut max_x = 0;
    let mut max_y = 0;
    getmaxyx(stdscr(), &mut max_y, &mut max_x);

    let height_top_bar = 3;
    let height_status_bar = 3;
    let height_playlist_win = max_y - height_status_bar - height_top_bar;

    let top_bar = newwin(height_top_bar, max_x, 0, 0);
    let playlist_win = newwin(height_playlist_win, max_x, height_status_bar, 0);
    let status_bar = newwin(
        height_status_bar,
        max_x,
        height_top_bar + height_playlist_win,
        0,
    );

    let (tx, rx) = mpsc::channel();

    match Mpv::connect("/tmp/mpvsocket") {
        Ok(mpv) => {
            let player = Arc::new(Mutex::new(Player {
                duration: 0f64,
                metadata: if let Ok(m) = mpv.get_metadata() {
                    Some(m)
                } else {
                    None
                },
                is_muted: mpv.get_property("mute").unwrap(),
                is_paused: mpv.get_property("pause").unwrap(),
                playlist: mpv.get_playlist().unwrap(),
                search_results: (vec![], 0),
                speed: mpv.get_property("speed").unwrap(),
                time_pos: 0f64,
                volume: mpv.get_property("volume").unwrap(),
            }));

            //Spawn the playlist observation thread
            {
                let (player, tx) = (player.clone(), tx.clone());
                thread::Builder::new()
                    .name("property_observer".into())
                    .spawn(move || {
                        //Start a new IPC client so there are no races for events between threads
                        let mut observer = Mpv::connect("/tmp/mpvsocket").unwrap();
                        observer
                            .observe_property(&OBS_ID_DURATION, "duration")
                            .unwrap();
                        observer
                            .observe_property(&OBS_ID_METADATA, "metadata")
                            .unwrap();
                        observer.observe_property(&OBS_ID_MUTE, "mute").unwrap();
                        observer
                            .observe_property(&OBS_ID_PLAYLIST, "playlist")
                            .unwrap();
                        observer.observe_property(&OBS_ID_PAUSE, "pause").unwrap();
                        observer.observe_property(&OBS_ID_SPEED, "speed").unwrap();
                        observer
                            .observe_property(&OBS_ID_TIME_POS, "time-pos")
                            .unwrap();
                        observer.observe_property(&OBS_ID_VOLUME, "volume").unwrap();
                        //observer.observe_property(&999, "mpv-version").unwrap();
                        // if !observer.get_property::<bool>("playback-abort").unwrap() {
                        //     observer.seek(0f64, SeekOptions::Relative).unwrap();
                        // }
                        loop {
                            let event = observer.event_listen().unwrap();
                            match event {
                                Event::PropertyChange { id, data, .. } => {
                                    match id { 
                                        OBS_ID_DURATION => {
                                            if let MpvDataType::Double(f) = data {
                                                player.lock().unwrap().duration = f;
                                                tx.send(Repaint::StatusBar(UpdateStatusBar::Time))
                                                    .unwrap();
                                            } else if let MpvDataType::Null = data {
                                                player.lock().unwrap().duration = 0f64;
                                                player.lock().unwrap().time_pos = 0f64;
                                                tx.send(Repaint::StatusBar(UpdateStatusBar::Clear))
                                                    .unwrap();
                                            }
                                        }

                                        OBS_ID_METADATA => {
                                            if let MpvDataType::HashMap(metadata) = data {
                                                player.lock().unwrap().metadata = Some(metadata);
                                                tx.send(Repaint::TopBar(UpdateTopBar::Metadata))
                                                    .unwrap();
                                            } else if let MpvDataType::Null = data {
                                                player.lock().unwrap().metadata = None;
                                                tx.send(Repaint::TopBar(UpdateTopBar::Clear))
                                                    .unwrap();
                                                tx.send(Repaint::TopBar(UpdateTopBar::Speed))
                                                    .unwrap();
                                                tx.send(Repaint::TopBar(UpdateTopBar::Volume))
                                                    .unwrap();
                                            }
                                        }

                                        OBS_ID_MUTE => {
                                            if let MpvDataType::Bool(muted) = data {
                                                player.lock().unwrap().is_muted = muted;
                                                if muted {
                                                    tx.send(
                                                        Repaint::StatusBar(UpdateStatusBar::Message(
                                                            String::from("Muted"),
                                                            Formatting::Blinking,
                                                        )),
                                                    ).unwrap();
                                                } else {
                                                    tx.send(
                                                        Repaint::StatusBar(UpdateStatusBar::Message(
                                                            String::from("     "),
                                                            Formatting::Blinking,
                                                        )),
                                                    ).unwrap();
                                                }
                                            }
                                        }

                                        OBS_ID_PAUSE => {
                                            if let MpvDataType::Bool(paused) = data {
                                                player.lock().unwrap().is_paused = paused;
                                                if paused {
                                                    tx.send(
                                                        Repaint::StatusBar(UpdateStatusBar::Message(
                                                            String::from("Paused"),
                                                            Formatting::Blinking,
                                                        )),
                                                    ).unwrap();
                                                } else {
                                                    tx.send(
                                                        Repaint::StatusBar(UpdateStatusBar::Clear),
                                                    ).unwrap();
                                                }
                                            }
                                        }

                                        OBS_ID_SPEED => {
                                            if let MpvDataType::Double(f) = data {
                                                player.lock().unwrap().speed = f;
                                                tx.send(Repaint::TopBar(UpdateTopBar::Speed))
                                                    .unwrap();
                                            }
                                        }

                                        OBS_ID_TIME_POS => {
                                            if let MpvDataType::Double(f) = data {
                                                player.lock().unwrap().time_pos = f;
                                                tx.send(Repaint::StatusBar(UpdateStatusBar::Time))
                                                    .unwrap();
                                            }
                                        }

                                        OBS_ID_PLAYLIST => {
                                            if let MpvDataType::Playlist(pl) = data {
                                                if player.lock().unwrap().playlist.0.len() !=
                                                    pl.0.len()
                                                {
                                                    player.lock().unwrap().playlist = pl;
                                                    tx.send(Repaint::Playlist {
                                                        clear_win: true,
                                                        scroll_to_beginning: true,
                                                    }).unwrap();
                                                } else {
                                                    player.lock().unwrap().playlist = pl;
                                                    tx.send(Repaint::Playlist {
                                                        clear_win: false,
                                                        scroll_to_beginning: false,
                                                    }).unwrap();
                                                }
                                            }
                                        }

                                        OBS_ID_VOLUME => {
                                            if let MpvDataType::Double(f) = data {
                                                player.lock().unwrap().volume = f;
                                                tx.send(Repaint::TopBar(UpdateTopBar::Volume))
                                                    .unwrap();
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }

                        }
                    })
                    .unwrap();
            }

            let playlist_canvas = PlaylistCanvas {
                top_line: 0,
                bottom_line: max_y as usize,
                selected_line: 0,
            };
            let playlist_canvas_mutex = Arc::new(Mutex::new(playlist_canvas));
            {
                let (mpv, player, playlist_canvas, tx) = (
                    mpv.clone(),
                    player.clone(),
                    playlist_canvas_mutex.clone(),
                    tx.clone(),
                );
                thread::Builder::new()
                    .name("input_listener".into())
                    .spawn(move || {
                        loop {
                            let ch = getch();
                            match ch {
                                KEY_UP => {
                                    let mut top_line = playlist_canvas.lock().unwrap().top_line;
                                    let mut selected_line =
                                        playlist_canvas.lock().unwrap().selected_line;

                                    if selected_line == top_line {
                                        if top_line > 0 {
                                            top_line -= 1;
                                        }
                                    }
                                    if selected_line > 0 {
                                        selected_line -= 1;
                                    }
                                    playlist_canvas.lock().unwrap().top_line = top_line;
                                    playlist_canvas.lock().unwrap().selected_line = selected_line;
                                    tx.send(Repaint::Playlist {
                                        clear_win: false,
                                        scroll_to_beginning: false,
                                    }).unwrap();
                                }

                                KEY_DOWN => {
                                    let ref playlist = player.lock().unwrap().playlist;
                                    let mut top_line = playlist_canvas.lock().unwrap().top_line;
                                    let bottom_line = playlist_canvas.lock().unwrap().bottom_line;
                                    let mut selected_line =
                                        playlist_canvas.lock().unwrap().selected_line;

                                    if selected_line < playlist.0.len() - 1 {
                                        selected_line += 1;
                                    }
                                    if selected_line == bottom_line {
                                        if top_line <
                                            playlist.0.len() - height_playlist_win as usize
                                        {
                                            top_line += 1;
                                        }
                                    }

                                    playlist_canvas.lock().unwrap().top_line = top_line;
                                    playlist_canvas.lock().unwrap().selected_line = selected_line;
                                    tx.send(Repaint::Playlist {
                                        clear_win: false,
                                        scroll_to_beginning: false,
                                    }).unwrap();
                                }

                                KEY_PPAGE => {
                                    let mut top_line = playlist_canvas.lock().unwrap().top_line;
                                    let mut selected_line =
                                        playlist_canvas.lock().unwrap().selected_line;

                                    if top_line >= height_playlist_win as usize {
                                        top_line -= height_playlist_win as usize;
                                        selected_line -= height_playlist_win as usize;
                                    } else {
                                        top_line = 0;
                                        selected_line = 0;
                                    }

                                    playlist_canvas.lock().unwrap().top_line = top_line;
                                    playlist_canvas.lock().unwrap().selected_line = selected_line;
                                    tx.send(Repaint::Playlist {
                                        clear_win: false,
                                        scroll_to_beginning: false,
                                    }).unwrap();
                                }

                                KEY_NPAGE => {
                                    let ref playlist = player.lock().unwrap().playlist;
                                    let mut top_line = playlist_canvas.lock().unwrap().top_line;
                                    let mut selected_line =
                                        playlist_canvas.lock().unwrap().selected_line;

                                    if playlist.0.len() > height_playlist_win as usize {
                                        if top_line as i32 <=
                                            playlist.0.len() as i32 - 2 * height_playlist_win
                                        {
                                            top_line += height_playlist_win as usize;
                                            selected_line += height_playlist_win as usize;
                                        } else {
                                            top_line = playlist.0.len() -
                                                height_playlist_win as usize;
                                            selected_line = playlist.0.len() - 1;
                                        }
                                    } else {
                                        selected_line = playlist.0.len() - 1;
                                    }

                                    playlist_canvas.lock().unwrap().top_line = top_line;
                                    playlist_canvas.lock().unwrap().selected_line = selected_line;
                                    tx.send(Repaint::Playlist {
                                        clear_win: false,
                                        scroll_to_beginning: false,
                                    }).unwrap();
                                }

                                KEY_LEFT => {
                                    if let Err(why) = mpv.seek(-5.0, SeekOptions::Relative) {
                                        tx.error(&format!("Error: {}", why));
                                    }
                                }

                                KEY_RIGHT => {
                                    if let Err(why) = mpv.seek(5.0, SeekOptions::Relative) {
                                        tx.error(&format!("Error: {}", why));
                                    }
                                }

                                KEY_ENTER => {
                                    let selected_line =
                                        playlist_canvas.lock().unwrap().selected_line;
                                    mpv.playlist_play_id(selected_line as usize).expect(
                                        "playlist_play_id",
                                    );
                                    //*playlist.lock().unwrap() = mpv.get_playlist().unwrap();
                                    tx.send(Repaint::Playlist {
                                        clear_win: false,
                                        scroll_to_beginning: false,
                                    }).unwrap();
                                }

                                KEY_BACKSPACE => {
                                    mpv.restart().expect("next");
                                }

                                KEY_GT => {
                                    mpv.next().expect("next");
                                }

                                KEY_LT => {
                                    mpv.prev().expect("prev");
                                }

                                KEY_PLUS => {
                                    mpv.set_volume(2f64, NumberChangeOptions::Increase).expect(
                                        "vol_up",
                                    );
                                }
                                
                                KEY_MINUS => {
                                    mpv.set_volume(2f64, NumberChangeOptions::Decrease).expect(
                                        "vol_down",
                                    );
                                }
                                
                                KEY_RSBR => {
                                    mpv.set_speed(0.05, NumberChangeOptions::Increase).expect(
                                        "speed_up",
                                    );
                                }
                                
                                KEY_LSBR => {
                                    mpv.set_speed(0.05, NumberChangeOptions::Decrease).expect(
                                        "speed_down",
                                    );
                                }
                                
                                KEY_M => {
                                    mpv.set_mute(Switch::Toggle).expect("next");
                                }
                                
                                KEY_N => {
                                    let ref mut player = player.lock().unwrap();

                                    if player.search_results.0.len() > 0 {
                                        //Calculate new index
                                        if player.search_results.1 < player.search_results.0.len() - 1 {
                                            player.search_results.1 += 1;
                                        } else { 
                                            player.search_results.1 = 0;
                                        }
                                        let (ref results, ref current_id) = player.search_results;

                                        let new_canvas;
                                        {
                                            let ref playlist = player.playlist;
                                            let ref canvas = playlist_canvas.lock().unwrap();
                                            new_canvas = try_center_id(playlist, canvas, results[*current_id]);
                                        }
                                        if let Some(new_canvas) = new_canvas {
                                            *playlist_canvas.lock().unwrap() = new_canvas;
                                            tx.send(Repaint::Playlist {
                                                clear_win: false,
                                                scroll_to_beginning: false,
                                            }).unwrap();
                                        }
                                    }
                                }
                                
                                KEY_O => {
                                    let ref playlist = player.lock().unwrap().playlist;
                                    let new_canvas;
                                    {
                                        let ref canvas = playlist_canvas.lock().unwrap();
                                        new_canvas = jump_to_current(playlist, canvas);
                                    }
                                    if let Some(new_canvas) = new_canvas {
                                        *playlist_canvas.lock().unwrap() = new_canvas;
                                        tx.send(Repaint::Playlist {
                                            clear_win: false,
                                            scroll_to_beginning: false,
                                        }).unwrap();
                                    }
                                }
                                
                                KEY_P => {
                                    mpv.toggle().expect("toggle");
                                }
                                
                                KEY_Q => {
                                    endwin();
                                    //mpv.disconnect();
                                    std::process::exit(0);
                                }
                                
                                KEY_R => {
                                    let selected_line =
                                        playlist_canvas.lock().unwrap().selected_line;
                                    mpv.playlist_remove_id(selected_line).unwrap();
                                    //*playlist.lock().unwrap() = mpv.get_playlist().unwrap();
                                    //tx.send(Repaint::Playlist(true)).unwrap();
                                }
                                
                                KEY_S => {
                                    mpv.stop().expect("mpv_stop");
                                    //*playlist.lock().unwrap() = mpv.get_playlist().unwrap();
                                    //tx.send(Repaint::Playlist(true)).unwrap();
                                }
                                
                                KEY_SLASH => {
                                    tx.send(
                                        Repaint::StatusBar(UpdateStatusBar::Message(
                                            format!("Search:"),
                                            Formatting::Normal,
                                        )),
                                    ).unwrap();
                                    let mut search_string = String::new();
                                    loop {
                                        let ch = getch();
                                        match ch {
                                            KEY_ESC => {
                                                tx.send(Repaint::StatusBar(UpdateStatusBar::Clear))
                                                    .unwrap();
                                                break;
                                            }

                                            KEY_ENTER => {
                                                let result;
                                                if &search_string == ""
                                                {
                                                    result = (vec![], 0);
                                                    tx.send(
                                                    Repaint::StatusBar(UpdateStatusBar::Clear)).unwrap();
                                                } else {
                                                    let ref playlist = player.lock().unwrap().playlist;
                                                    result =
                                                        (search_playlist(playlist, &search_string), 0);
                                                        let match_count = result.0.len();
                                                                                                
                                                    if match_count as i32 > 0 {
                                                        //Jump to first result
                                                        let new_canvas;
                                                        {
                                                            let ref canvas =
                                                                playlist_canvas.lock().unwrap();
                                                            new_canvas =
                                                                try_center_id(playlist,
                                                                canvas,
                                                                result.0[result.1]);
                                                        }
                                                        if let Some(new_canvas) = new_canvas {
                                                            *playlist_canvas.lock().unwrap() =
                                                                new_canvas;
                                                            tx.send(Repaint::Playlist {
                                                                clear_win: false,
                                                                scroll_to_beginning: false,
                                                            }).unwrap();

                                                            //Clear search info
                                                            tx.send(
                                                    Repaint::StatusBar(UpdateStatusBar::Clear)).unwrap();
                                                        }
                                                    } else {
                                                        tx.send(Repaint::StatusBar(UpdateStatusBar::Message(
                                                            String::from("Search pattern not found"),
                                                            Formatting::Normal))).unwrap();
                                                    }
                                                }
                                                player.lock().unwrap().search_results = result;

                                                break;
                                            }

                                            KEY_BACKSPACE => {
                                                search_string.pop();
                                                tx.send(
                                                    Repaint::StatusBar(UpdateStatusBar::Clear)).unwrap();
                                                tx.send(
                                                    Repaint::StatusBar(UpdateStatusBar::Message(
                                                        format!("Search: {}", search_string),
                                                        Formatting::Normal,
                                                    )),
                                                ).unwrap();
                                            }

                                            _ => {
                                                //panic!("{}", ch);
                                                search_string.push(
                                                    std::char::from_u32(ch as u32)
                                                        .expect("Invalid char"),
                                                );
                                                tx.send(
                                                    Repaint::StatusBar(UpdateStatusBar::Message(
                                                        format!("Search: {}", search_string),
                                                        Formatting::Normal,
                                                    )),
                                                ).unwrap();
                                            }
                                        }
                                    }
                                }
                                
                                KEY_U => {
                                    player.lock().unwrap().playlist = mpv.get_playlist().unwrap();
                                    tx.send(Repaint::Playlist {
                                        clear_win: true,
                                        scroll_to_beginning: true,
                                    }).unwrap();
                                }
                                
                                KEY_Z => {
                                    mpv.run_command("playlist-shuffle", &[]).unwrap();

                                    let ref playlist = mpv.get_playlist().unwrap();
                                    let new_canvas;
                                    {
                                        let ref canvas = playlist_canvas.lock().unwrap();
                                        new_canvas = jump_to_current(playlist, canvas);
                                    }
                                    if let Some(new_canvas) = new_canvas {
                                        *playlist_canvas.lock().unwrap() = new_canvas;
                                        tx.send(Repaint::Playlist {
                                            clear_win: false,
                                            scroll_to_beginning: false,
                                        }).unwrap();
                                    }
                                }
                                _ => {
                                    //panic!("{}", ch);
                                }
                            }
                        }
                    })
                    .unwrap();
            }

            print_status(status_bar, "This is a test");
            wrefresh(status_bar);

            //Trigger first update
            tx.send(Repaint::Playlist {
                clear_win: false,
                scroll_to_beginning: false,
            }).unwrap();

            //Main loop
            loop {
                wmove(top_bar, 2, 0);
                whline(top_bar, ACS_HLINE(), max_x);
                wrefresh(top_bar);
                wmove(status_bar, 0, 0);
                whline(status_bar, ACS_HLINE(), max_x);
                wrefresh(status_bar);
                //Wait for repaint trigger
                match rx.recv().unwrap() {
                    //Repaint playlist
                    Repaint::Playlist {
                        clear_win,
                        scroll_to_beginning,
                    } => {
                        let mut top_line = playlist_canvas_mutex.lock().unwrap().top_line;
                        let mut selected_line = playlist_canvas_mutex.lock().unwrap().selected_line;
                        let ref playlist = player.lock().unwrap().playlist;
                        //panic!("Playlist changed");
                        if clear_win {
                            wclear(playlist_win);
                        }

                        if scroll_to_beginning {
                            top_line = 0;
                            selected_line = 0;
                            playlist_canvas_mutex.lock().unwrap().top_line = top_line;
                            playlist_canvas_mutex.lock().unwrap().selected_line = selected_line;
                        }

                        let bottom_line = top_line + height_playlist_win as usize;
                        playlist_canvas_mutex.lock().unwrap().bottom_line = bottom_line;

                        wmove(playlist_win, 0, 0);
                        if selected_line as i32 > playlist.0.len() as i32 - 1 {
                            selected_line = 0;
                            playlist_canvas_mutex.lock().unwrap().selected_line = selected_line;
                        }

                        print_playlist(
                            &playlist_win,
                            &playlist,
                            &playlist_canvas_mutex.lock().unwrap(),
                        );
                    }

                    Repaint::StatusBar(what) => {
                        match what {
                            UpdateStatusBar::Clear => {
                                wclear(status_bar);
                            }
                            UpdateStatusBar::Message(msg, formatting) => {
                                match formatting {
                                    Formatting::Normal => {}
                                    Formatting::Blinking => {
                                        wattron(status_bar, A_BLINK());
                                    }
                                }
                                wmove(status_bar, 1, 0);
                                wprintw(status_bar, &msg);
                                match formatting {
                                    Formatting::Normal => {}
                                    Formatting::Blinking => {
                                        wattroff(status_bar, A_BLINK());
                                    }
                                }
                            }
                            UpdateStatusBar::Time => {
                                let player = player.lock().unwrap();
                                let percentage = 100f64 / player.duration * player.time_pos;
                                let time_text = &format!(
                                    "    {} / {} ({}%)",
                                    get_pretty_time(player.time_pos),
                                    get_pretty_time(player.duration),
                                    percentage as i32
                                );
                                //Aligned on the right
                                wmove(status_bar, 1, max_x - time_text.len() as i32);
                                //wmove(status_bar, 0, 0);
                                wprintw(status_bar, time_text);

                                wmove(status_bar, 2, 0);
                                for i in 0..max_x {
                                    if i < max_x / 100 * percentage as i32 {
                                        wprintw(status_bar, "=");
                                    //waddch(status_bar, '=' as u32);
                                    //wattron(status_bar, A_STANDOUT());
                                    } else if i == max_x / 100 * percentage as i32 {
                                        wprintw(status_bar, ">");
                                    //waddch(status_bar, 'ç' as u32);
                                    //wattron(status_bar, A_STANDOUT());
                                    } else {
                                        wprintw(status_bar, "-");
                                        //waddch(status_bar, ACS_HLINE());
                                        //wattroff(status_bar, A_STANDOUT());
                                    }
                                }
                            }
                        }
                        wmove(status_bar, 0, 0);
                        whline(status_bar, ACS_HLINE(), max_x);
                        wrefresh(status_bar);
                    }

                    Repaint::TopBar(what) => {
                        match what {
                            UpdateTopBar::Clear => {
                                wclear(top_bar);
                            }
                            UpdateTopBar::Metadata => {
                                let player = player.lock().unwrap();
                                let ref metadata = player.metadata.as_ref().unwrap();
                                wmove(top_bar, 0, 0);
                                wprintw(top_bar, "Title:  ");
                                if metadata.contains_key("title") {
                                    if let MpvDataType::String(ref title) = metadata["title"] {
                                        wprintw(top_bar, title);
                                    }
                                } else {
                                    wprintw(
                                        top_bar,
                                        if let Ok(ref title) = mpv.get_property::<String>(
                                            "media-title",
                                        )
                                        {
                                            title
                                        } else {
                                            "<empty>"
                                        },
                                    );
                                }

                                wmove(top_bar, 1, 0);
                                wprintw(top_bar, "Artist: ");
                                if metadata.contains_key("artist") {
                                    if let MpvDataType::String(ref artist) = metadata["artist"] {
                                        wprintw(top_bar, artist);
                                    }
                                } else {
                                    wprintw(top_bar, "<empty>");
                                }
                            }

                            UpdateTopBar::Speed => {
                                let player = player.lock().unwrap();
                                let speed_str = &format!("  Speed: {:.*} ", 2, player.speed);

                                //Aligned on the right
                                wmove(top_bar, 0, max_x - speed_str.len() as i32);
                                wprintw(top_bar, speed_str);
                            }

                            UpdateTopBar::Volume => {
                                let player = player.lock().unwrap();
                                let volume_str = &format!("  Volume: {}%%", player.volume as usize);

                                //Aligned on the right
                                wmove(top_bar, 1, max_x - volume_str.len() as i32);
                                wprintw(top_bar, volume_str);
                            }
                        }

                        wmove(top_bar, 2, 0);
                        whline(top_bar, ACS_HLINE(), max_x);
                        wrefresh(top_bar);
                    }
                }
            }
        }
        Err(code) => {
            endwin();
            error!("Error: Could not connect to mpv socket: {}", code);
        }
    }
}

fn print_playlist(win: &WINDOW, playlist: &Playlist, canvas: &PlaylistCanvas) {
    let from = canvas.top_line;
    let to = canvas.bottom_line;
    let selected = canvas.selected_line;
    let max_x = getmaxx(*win);
    if playlist.0.len() > 0 {
        let mut y = 0;
        for i in from..to {
            wmove(*win, y, 0);
            if i < playlist.0.len() {
                let ref entry = playlist.0[i as usize];
                if i == selected {
                    wattron(*win, A_REVERSE());
                }
                let mut output = format!(
                    "{}{}{}",
                    entry.id,
                    if entry.current {
                        if entry.id < 10 {
                            "   ▶ "
                        } else if entry.id < 100 {
                            "  ▶ "
                        } else {
                            " ▶ "
                        }
                    } else {
                        if entry.id < 10 {
                            "     "
                        } else if entry.id < 100 {
                            "    "
                        } else {
                            "   "
                        }
                    },
                    if &entry.title == "" {
                        &entry.filename
                    } else {
                        &entry.title
                    }
                );
                let len = output.chars().count();
                if len > max_x as usize {
                    output.truncate(max_x as usize - 3);
                    output.push_str("...");
                }
                if entry.current {
                    wattron(*win, A_BOLD());
                }
                if len < max_x as usize {
                    for _ in 0..max_x as usize - len {
                        output.push(' ');
                    }
                }
                wprintw(*win, &output);
                //printw("\n");
                wattroff(*win, A_BOLD());
                if i == selected {
                    wattroff(*win, A_REVERSE());
                }
            }
            y += 1;
        }
    } else {
        wmove(*win, 0, 6);
        wprintw(*win, "Playlist is empty");
    }

    wmove(*win, 0, 5);
    wvline(*win, ACS_VLINE(), (to - from) as i32);
    wrefresh(*win);
}

fn get_pretty_time(seconds: f64) -> String {
    let hours = seconds as i64 / 3600;
    let mins = (seconds as i64 - hours * 3600) / 60;
    let secs = seconds as i64 % 60;
    if seconds < 3600 as f64 {
        format!(
            "{}:{}",
            if mins < 10 {
                format!("0{}", mins)
            } else {
                format!("{}", mins)
            },

            if secs < 10 {
                format!("0{}", secs)
            } else {
                format!("{}", secs)
            }
        )
    } else {
        format!(
            "{}:{}:{}",
            if hours < 10 {
                format!("0{}", hours)
            } else {
                format!("{}", hours)
            },

            if mins < 10 {
                format!("0{}", mins)
            } else {
                format!("{}", mins)
            },

            if secs < 10 {
                format!("0{}", secs)
            } else {
                format!("{}", secs)
            }
        )
    }
}

fn print_status(win: WINDOW, msg: &str) {
    wmove(win, 1, 0);
    wprintw(win, msg);
    wrefresh(win);
}

fn jump_to_current(
    playlist: &Playlist,
    playlist_canvas: &PlaylistCanvas,
) -> Option<PlaylistCanvas> {
    let mut current_id: usize = 0;
    for (id, entry) in playlist.0.iter().enumerate() {
        if entry.current {
            current_id = id;
            break;
        }
    }

    try_center_id(playlist, playlist_canvas, current_id)
}

fn search_playlist(playlist: &Playlist, search_pattern: &str) -> Vec<usize> {
    let mut output: Vec<usize> = vec![];
    for ref entry in playlist.0.iter() {
        if entry.filename.contains(search_pattern) || entry.title.contains(search_pattern) {
            output.push(entry.id);
        }
    }

    output
}

fn try_center_id(
    playlist: &Playlist,
    canvas: &PlaylistCanvas,
    id: usize,
) -> Option<PlaylistCanvas> {
    let top_line;
    let bottom_line;
    let line_count = canvas.bottom_line - canvas.top_line;
    if id < playlist.0.len() {
        if id < line_count / 2 {
            top_line = 0;
            bottom_line = top_line + line_count;
        } else if id > playlist.0.len() - line_count / 2 {
            bottom_line = playlist.0.len();
            top_line = bottom_line - line_count;
        } else {
            top_line = id - line_count / 2;
            bottom_line = top_line + line_count;
        }

        return Some(PlaylistCanvas {
            top_line,
            bottom_line,
            selected_line: id,
        });
    }
    return None;
}
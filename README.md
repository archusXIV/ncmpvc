This is a forked project from https://gitlab.com/mpv-ipc/ncmpvc.

![ncmpvc screenshot](https://github.com/archusXIV/ncmpvc/blob/main/res/logo.png "logo")
# ncmpvc (ncurses mpv client)

A ncurses client for mpv which connects to existing mpv instances through sockets, written in Rust.

This tool is inspired by ncmpcpp, a curses based client for the Music Player Daemon.
It makes use of mpv's JSON IPC protocol to control any mpv instance over a given socket.

**WARNING**: 
This app is in early development stage and _will_ contain bugs.
If you are a bug-hunter, feel free to use the app and report bugs at the [Issue Tracker](https://gitlab.com/mpv-ipc/ncmpvc/issues).

![ncmpvc screenshot](https://github.com/archusXIV/ncmpvc/blob/main/ncmpvc.png "ncmpvc screenshot")

Make sure mpv is started with the following option:
`
$ mpv --input-ipc-server=/tmp/mpvsocket ...
`

## Dependencies

- `mpv`
- `cargo` (makedep)
- `ncurses`

## Install

- [Arch](https://aur.archlinux.org/packages/ncmpvc-git) - `yay -S ncmpvc-git`

If you have packaged mpvc for your distribution, let me know so I can add it here.

#### Manual Install

Use "cargo build --release" to build the program.
The output binary will be found in 'target/release/'

## Usage

Make sure mpv is started with the following option:
`
$ mpv --input-ipc-server=/tmp/mpvsocket --idle
`

At the moment ncmpvc does not launch mpv instances, so the instances have to be launched beforehand. Also, the path to the socket is hardcoded to `/tmp/mpvsocket`. It will be possible in the future to read this from a configuration file.
I'm not sure yet where to go with this project so this might change in the future.
To control mpv without a user interface I suggest the use of [mpvc](https://gitlab.com/mpv-ipc/mpvc-rs).

### Key bindings
Key | Feature | Comment
--- | --- | ---
Play | `ENTER` |
Scrolling | `UP`, `DOWN`, `PGUP`, `PGDOWN` |
Jump to current song | `o`
Shuffle playlist | `z` | mpv >= v0.26.0
Remove from playlist | `r` |
Stop playback | `s` |
Toggle playback | `p` |
Toggle mute | `m` |
Search mode | `/` |
Cancel search mode | `ESC` |
Play next/previous song | `>`, `<` |
Volume up/down 2% | `+`, `-` |
Speed up/down 5% | `]`, `[` |
Seek (+/- 5 seconds) | `LEFT`, `RIGHT` |
Force playlist update | `u` | should never be necessary
Quit ncmpvc | `q` |

## Roadmap
* [x] Implement basic control with keys (see key bindings)
* [x] Header bar with infos about current song
* [x] Status bar with time information
* [x] Ability to search playlist
* [x] Ability to jump to current song
* [ ] Ability to add files / playlist (integrated filebrowser)
* [ ] Add more player commands:
  * [x] Playlist shuffle
  * [ ] Fast seek
  * [x] Increase / decrease speed
  * [x] Restart playback
* [ ] Ability to change player options
  * [ ] loop-file
  * [ ] loop-playlist
  * [ ] consume mode
* [ ] Ability to resize the window
* [ ] Ability to configure _ncmpvc_ in a configuration file
* [ ] Proper error handling

## Bugs / Ideas

Check out the [Issue Tracker](https://gitlab.com/mpv-ipc/ncmpvc/issues)

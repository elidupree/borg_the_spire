# Borg the Spire (under construction)

An AI/cyborg gameplay helper for Slay the Spire. Runs alongside Slay the Spire and shows an external window with AI suggestions.

Fair warning: This is a toy project for me, and I don't particularly expect to complete it or make it convenient for anyone but me to use.

This crate compiles to an executable usable with [CommunicationMod](https://github.com/ForgottenArbiter/CommunicationMod).

There are two separate subcommands, `borg_the_spire communicate` and `borg_the_spire live-analyze`. `communicate` talks to CommunicationMod and saves the gamestate to a file. `live-analyze` watches the file for changes and serves a webpage that displays its analysis. This division exists for two reasons:
* First, it allows me to change and rebuild the `live-analyze` part without restarting Slay the Spire.
* Second, it allows me to run `live-analyze`, which is the CPU-heavy part, on a separate computer.

To run Borg the Spire:
* `cargo build`
* Set the CommunicationMod command to run `borg_the_spire communicate [state-file]`, giving a filepath where the gamestate will be saved (e.g. `command=C\:\\Path\\To\\borg_the_spire\\target\\debug\\borg_the_spire.exe communicate C\:\\Path\\To\\borg_the_spire\\data\\last_state.json`)
* Run Slay the Spire with mods, enabling CommunicationMod
* Run `borg_the_spire live-analyze --state-file=[state-file] --static-files=Path\\To\\borg_the_spire\\static --data-files=Path\\To\\borg_the_spire\\data --ip=[ip] --port=[port]`, which will watch the state file, analyze it whenever it changes, and serve the analyzed output. `ip` is an address to listen on, perhaps `localhost`.
* While `borg_the_spire live-analyze` is running, go to `http://[ip]:[port]/` in a browser for the interface.